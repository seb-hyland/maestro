use fxhash::{FxBuildHasher, FxHashSet};
use proc_macro::{Span, TokenStream};
use proc_macro2::{Span as Span2, TokenStream as TokenStream2};
use quote::quote;
use rand::{Rng as _, distr::Uniform};
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write as _},
    path::Path,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use syn::{
    Expr, Ident, LitBool, LitStr, bracketed,
    parse::{self, Parse},
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Eq},
};

#[cfg(feature = "check_scripts")]
mod checker;
// mod container;

struct ProcessDefinition {
    name: Option<Expr>,
    inputs: Punctuated<Ident, Comma>,
    outputs: Punctuated<Ident, Comma>,
    args: Punctuated<Ident, Comma>,
    dependencies: Punctuated<LitStr, Comma>,
    inline: bool,
    literal: LitStr,
}

mod kw {
    use syn::custom_keyword;
    custom_keyword!(name);
    custom_keyword!(inputs);
    custom_keyword!(outputs);
    custom_keyword!(args);
    custom_keyword!(dependencies);
    custom_keyword!(inline);
    custom_keyword!(process);
}

impl Parse for ProcessDefinition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: Option<Expr> = if input.peek(kw::name) {
            let _: kw::name = input.parse()?;
            let _: Eq = input.parse()?;
            let name = input.parse()?;
            let _: Comma = input.parse()?;
            Some(name)
        } else {
            None
        };

        macro_rules! parse_list {
            ($token:path, $parse:expr) => {
                if input.peek($token) {
                    let _: $token = input.parse()?;
                    let _: Eq = input.parse()?;
                    let process_inputs;
                    bracketed!(process_inputs in input);
                    let parsed = process_inputs.parse_terminated($parse, Comma);
                    let _: Comma = input.parse()?;
                    parsed
                } else {
                    Ok(Punctuated::new())
                }
            };
        }
        let inputs = parse_list!(kw::inputs, Ident::parse)?;
        let args = parse_list!(kw::args, Ident::parse)?;
        let outputs = parse_list!(kw::outputs, Ident::parse)?;
        let dependencies = parse_list!(kw::dependencies, <LitStr as parse::Parse>::parse)?;

        let inline = if input.peek(kw::inline) {
            let _: kw::inline = input.parse()?;
            let _: Eq = input.parse()?;
            let bool: LitBool = input.parse()?;
            let _: Comma = input.parse()?;
            bool.value
        } else {
            true
        };

        let _: kw::process = input.parse()?;
        let _: Eq = input.parse()?;
        let literal: LitStr = input.parse()?;
        let _: Result<Comma, _> = input.parse();

        Ok(ProcessDefinition {
            name,
            inputs,
            args,
            outputs,
            dependencies,
            inline,
            literal,
        })
    }
}

///
/// ## Example
/// ```rust
/// process! {
///     ...something
/// }
/// ```
#[proc_macro]
pub fn process(input: TokenStream) -> TokenStream {
    let definition = parse_macro_input!(input as ProcessDefinition);

    let literal = definition.literal;
    let literal_value = literal.value();

    let mut has_shebang = true;
    let process = if definition.inline {
        let trimmed_lit = literal_value.trim();
        if !trimmed_lit.starts_with("#!") {
            has_shebang = false;
            String::from("#!/bin/bash\n") + trimmed_lit
        } else {
            trimmed_lit.to_string()
        }
    } else {
        let path = Path::new(&literal_value);
        let path_disp = path.display();
        if !path.exists() {
            return syn::Error::new(
                literal.span(),
                format!("The file {path_disp} does not exist"),
            )
            .into_compile_error()
            .into();
        }
        match fs::read_to_string(path) {
            Ok(v) => v,
            Err(e) => {
                return syn::Error::new(
                    literal.span(),
                    format!("The file {path_disp} could not be read: {e:?}"),
                )
                .into_compile_error()
                .into();
            }
        }
    };

    #[cfg(feature = "check_scripts")]
    {
        // Make a copy and append environment variables to stop shellcheck yapping abt undefined vars
        let mut presented_contents = process.clone();
        let mut inject = |arg: &Ident| presented_contents.push_str(&format!("\n{arg}=\"\""));
        for input in &definition.inputs {
            inject(input);
        }
        for output in &definition.outputs {
            inject(output);
        }
        for arg in &definition.args {
            inject(arg);
        }

        let path = if definition.inline {
            None
        } else {
            Some(literal_value.as_str())
        };

        if let Err((msg, e)) = checker::run_shellcheck(&presented_contents, path, has_shebang) {
            return syn::Error::new(literal.span(), format!("{msg}\n{e}"))
                .into_compile_error()
                .into();
        }
    }

    let process_lit = LitStr::new(&process, literal.span());
    fn into_pairs(args: Punctuated<Ident, Comma>) -> impl IntoIterator<Item = TokenStream2> {
        args.into_iter().map(|ident| {
            let lit = LitStr::new(&ident.to_string(), ident.span());
            quote! { (::std::borrow::Cow::Borrowed(#lit), PathBuf::from(#ident))}
        })
    }
    let input_pairs = into_pairs(definition.inputs).into_iter();
    let output_pairs = into_pairs(definition.outputs).into_iter();
    let arg_pairs = definition.args.into_iter().map(|ident| {
        let lit = LitStr::new(&ident.to_string(), ident.span());
        quote! { (#lit, #ident.to_string())}
    });

    fn generate_hashed_name() -> String {
        loop {
            let rng = rand::rng();
            let letter_sample = Uniform::new_inclusive('a', 'z')
                .expect("Uniform character sampling should not fail!");
            let hash: String = rng.sample_iter(letter_sample).take(10).collect();
            {
                let mut handle = GENERATED_HASHES.lock().unwrap();
                if !handle.contains(&hash) {
                    handle.insert(hash.clone());
                    return hash;
                }
            }
        }
    }
    let name = match definition.name {
        Some(expr) => quote! {{ #expr }},
        None => {
            let name = generate_hashed_name();
            quote! { #name }
        }
    };

    let mut dependencies = FxHashSet::default();
    let mut excludes = SHELL_EXCLUDES.clone();
    for dependency_lit in definition.dependencies {
        let dependency = dependency_lit.value();
        if let Some(excluded) = dependency.strip_prefix('!') {
            excludes.insert(excluded.to_string());
        } else {
            dependencies.insert(dependency);
        }
    }
    analyze_depends(&process, &mut excludes, &mut dependencies);
    let dependencies_filepath =
        Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("dependencies.txt");
    let first_invocation = FIRST_INVOCATION.load(Ordering::Acquire);
    let mut dependencies_file = match if first_invocation {
        FIRST_INVOCATION.store(false, Ordering::Release);
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(dependencies_filepath)
    } else {
        OpenOptions::new().append(true).open(dependencies_filepath)
    } {
        Ok(file) => file,
        Err(e) => {
            return syn::Error::new(
                Span2::call_site(),
                format!("Failed to open dependency file for writing: {e:#?}"),
            )
            .into_compile_error()
            .into();
        }
    };
    let process_span = literal.span();
    if let Err(e) = || -> Result<(), io::Error> {
        if !first_invocation {
            writeln!(dependencies_file)?;
        }
        writeln!(
            dependencies_file,
            "{}:{}:{}",
            process_span.file(),
            process_span.start().line,
            process_span.start().column
        )?;
        for dependency in dependencies {
            writeln!(dependencies_file, "- {}", dependency)?;
        }
        Ok(())
    }() {
        return syn::Error::new(
            process_span,
            format!("Failed to write into dependencies.txt: {e:#?}"),
        )
        .into_compile_error()
        .into();
    }

    quote! {
        maestro::Process::new(
            #name.to_string(),
            ::std::borrow::Cow::Borrowed(#process_lit),
            vec![#(#input_pairs),*],
            vec![#(#output_pairs),*],
            vec![#(#arg_pairs),*]
        )
    }
    .into()
}

static GENERATED_HASHES: LazyLock<Mutex<FxHashSet<String>>> =
    LazyLock::new(|| Mutex::new(FxHashSet::default()));

fn analyze_depends(
    script: &str,
    excluded: &mut FxHashSet<String>,
    all_dependencies: &mut FxHashSet<String>,
) {
    let mut push_if_not_excluded = |depend: String| {
        if !excluded.contains(&depend) {
            all_dependencies.insert(depend);
        }
    };
    let is_valid = |input: &str| -> bool {
        input
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
    };
    for line in script.lines() {
        let mut tokens = line
            .split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}')
            .filter(|t| !t.is_empty())
            .peekable();
        if let Some(first) = tokens.next()
            && is_valid(first)
        {
            push_if_not_excluded(first.to_string());
        }
        while let Some(token) = tokens.next()
            && let Some(next_token) = tokens.peek()
        {
            if ((token == "|" || token == "&&" || token == "||") || token.ends_with(';'))
                && is_valid(next_token)
            {
                push_if_not_excluded(next_token.to_string());
            }
        }
    }
}

static FIRST_INVOCATION: AtomicBool = AtomicBool::new(true);
static SHELL_EXCLUDES: LazyLock<FxHashSet<String>> = LazyLock::new(|| {
    let mut set = FxHashSet::with_capacity_and_hasher(
        SHELL_KEYWORDS.len() + SHELL_BUILTINS.len(),
        FxBuildHasher::default(),
    );
    for exclude in SHELL_KEYWORDS.into_iter().chain(SHELL_BUILTINS) {
        set.insert(exclude.to_string());
    }
    set
});
const SHELL_KEYWORDS: [&str; 17] = [
    "case", "coproc", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in",
    "select", "then", "until", "while", "time",
];
const SHELL_BUILTINS: [&str; 57] = [
    "alias",
    "bg",
    "bind",
    "break",
    "builtin",
    "caller",
    "cd",
    "command",
    "compgen",
    "complete",
    "compopt",
    "continue",
    "declare",
    "typeset",
    "dirs",
    "disown",
    "echo",
    "enable",
    "eval",
    "exec",
    "exit",
    "export",
    "false",
    "fc",
    "fg",
    "getopts",
    "hash",
    "help",
    "history",
    "jobs",
    "kill",
    "let",
    "local",
    "logout",
    "mapfile",
    "readarray",
    "popd",
    "printf",
    "pushd",
    "pwd",
    "read",
    "readonly",
    "return",
    "set",
    "shift",
    "shopt",
    "suspend",
    "test",
    "times",
    "trap",
    "true",
    "type",
    "ulimit",
    "umask",
    "unalias",
    "unset",
    "wait",
];
