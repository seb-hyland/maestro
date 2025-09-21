use crate::dep_analysis::{
    ContainerDependency, DEPENDENCIES_FILE, ProcessDependencies, SHELL_EXCLUDES, analyze_depends,
};
use fxhash::FxHashSet;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rand::{Rng as _, distr::Uniform};
use std::{
    collections::HashMap,
    fs,
    io::Write as _,
    path::Path,
    sync::{LazyLock, Mutex},
};
use syn::{
    Expr, Ident, LitBool, LitStr, bracketed, parenthesized,
    parse::{self, Parse},
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Eq},
};

#[cfg(feature = "check_scripts")]
mod checker;
mod dep_analysis;

struct ProcessDefinition {
    docstr: Option<LitStr>,
    name: Option<Expr>,
    executor: Option<LitStr>,
    container: Option<Container>,
    inputs: Punctuated<Ident, Comma>,
    args: Punctuated<Ident, Comma>,
    outputs: Punctuated<Ident, Comma>,
    dependencies: Punctuated<LitStr, Comma>,
    inline: bool,
    literal: LitStr,
}
enum Container {
    Docker(LitStr),
    Apptainer(LitStr),
}

mod kw {
    use syn::custom_keyword;
    custom_keyword!(doc);
    custom_keyword!(executor);
    custom_keyword!(name);
    custom_keyword!(inputs);
    custom_keyword!(outputs);
    custom_keyword!(args);
    custom_keyword!(container);
    custom_keyword!(Docker);
    custom_keyword!(Apptainer);
    custom_keyword!(dependencies);
    custom_keyword!(inline);
    custom_keyword!(process);
}

impl Parse for ProcessDefinition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut docstr = None;
        let mut name = None;
        let mut executor = None;
        let mut container = None;
        let mut inputs = Punctuated::new();
        let mut args = Punctuated::new();
        let mut outputs = Punctuated::new();
        let mut dependencies = Punctuated::new();
        let mut inline = true;
        let mut process = None;

        macro_rules! parse_list {
            ($token:ident, $parse:expr) => {{
                let _: kw::$token = input.parse()?;
                let _: Eq = input.parse()?;
                let process_inputs;
                bracketed!(process_inputs in input);
                $token = process_inputs.parse_terminated($parse, Comma)?;
            }};
        }

        while !input.is_empty() {
            if input.peek(kw::doc) {
                let _: kw::doc = input.parse()?;
                let _: Eq = input.parse()?;
                docstr = Some(input.parse()?);
            } else if input.peek(kw::name) {
                let _: kw::name = input.parse()?;
                let _: Eq = input.parse()?;
                name = Some(input.parse()?);
            } else if input.peek(kw::executor) {
                let _: kw::executor = input.parse()?;
                let _: Eq = input.parse()?;
                executor = Some(input.parse()?);
            } else if input.peek(kw::container) {
                let _: kw::container = input.parse()?;
                let _: Eq = input.parse()?;
                container = Some(if input.peek(kw::Docker) {
                    let _: kw::Docker = input.parse()?;
                    let container_literal;
                    parenthesized!(container_literal in input);
                    let image: LitStr = container_literal.parse()?;
                    Container::Docker(image)
                } else {
                    let _: kw::Apptainer = input.parse().map_err(|_| {
                        syn::Error::new(input.span(), "Expected keyword Docker or Apptainer")
                    })?;
                    let container_literal;
                    parenthesized!(container_literal in input);
                    let image: LitStr = container_literal.parse()?;
                    Container::Apptainer(image)
                });
            } else if input.peek(kw::inputs) {
                parse_list!(inputs, Ident::parse)
            } else if input.peek(kw::args) {
                parse_list!(args, Ident::parse)
            } else if input.peek(kw::outputs) {
                parse_list!(outputs, Ident::parse)
            } else if input.peek(kw::dependencies) {
                parse_list!(dependencies, <LitStr as parse::Parse>::parse)
            } else if input.peek(kw::inline) {
                let _: kw::inline = input.parse()?;
                let _: Eq = input.parse()?;
                inline = input.parse::<LitBool>()?.value();
            } else if input.peek(kw::process) {
                let _: kw::process = input.parse()?;
                let _: Eq = input.parse()?;
                process = Some(input.parse()?);
            }
            if let Err(_) = input.parse::<Comma>()
                && !input.is_empty()
            {
                return Err(syn::Error::new(
                    input.span(),
                    "Fields must be separated by commas",
                ));
            }
        }

        let literal = match process {
            Some(v) => v,
            None => {
                return Err(syn::Error::new(
                    input.span(),
                    "Missing required `process` field!",
                ));
            }
        };

        Ok(ProcessDefinition {
            docstr,
            name,
            executor,
            container,
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
        let mut injection_count = 0;
        let mut inject = |arg: &Ident| {
            injection_count += 1;
            presented_contents.push_str(&format!("\n{arg}=\"\""));
        };
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

        if let Err((msg, e)) =
            checker::run_shellcheck(&presented_contents, path, has_shebang, injection_count)
        {
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
    let mut ignore_all = false;
    for dependency_lit in definition.dependencies {
        let dependency = dependency_lit.value();
        if dependency == "!" {
            ignore_all = true;
        }
        if let Some(excluded) = dependency.strip_prefix('!') {
            excludes.insert(excluded.to_string());
        } else {
            dependencies.insert(dependency);
        }
    }
    if !ignore_all {
        analyze_depends(&process, &mut excludes, &mut dependencies);
    }

    let process_span = literal.span();
    let mut root = HashMap::new();
    let dependencies = ProcessDependencies {
        doc: definition.docstr.map(|lit| lit.value()),
        executor: definition
            .executor
            .as_ref()
            .map(|lit| lit.value())
            .unwrap_or("default".to_string()),
        container: definition
            .container
            .as_ref()
            .map(|container| match container {
                Container::Docker(img) => ContainerDependency::Docker { image: img.value() },
                Container::Apptainer(img) => ContainerDependency::Apptainer { image: img.value() },
            }),
        deps: dependencies.into_iter().collect(),
    };
    root.insert(
        format!(
            "{}:{}:{}",
            process_span.file(),
            process_span.start().line,
            process_span.start().column
        ),
        dependencies,
    );

    let toml_str = match toml::to_string(&root) {
        Ok(toml_str) => toml_str,
        Err(e) => {
            return syn::Error::new(
                process_span,
                format!("Failed to serialize dependencies to toml: {e}"),
            )
            .into_compile_error()
            .into();
        }
    };

    {
        let mut lock = DEPENDENCIES_FILE.lock().unwrap();
        if let Err(e) = writeln!(lock, "{toml_str}") {
            return syn::Error::new(
                process_span,
                format!("Failed to write into dependencies.txt: {e:#?}"),
            )
            .into_compile_error()
            .into();
        }
    }

    let container = match definition.container {
        None => quote! { None },
        Some(container) => match container {
            Container::Docker(img) => {
                quote! { Some(maestro::Container::Docker(::std::borrow::Cow::Borrowed(#img))) }
            }
            Container::Apptainer(img) => {
                quote! { Some(maestro::Container::Apptainer(::std::borrow::Cow::Borrowed(#img))) }
            }
        },
    };
    let executor = match definition.executor {
        None => quote! { maestro::MAESTRO_CONFIG.executor.exe(process) },
        Some(executor) => quote! {
            maestro::submit_request! {
                maestro::RequestedExecutor(#executor, file!(), line!(), column!())
            };
            maestro::MAESTRO_CONFIG.custom_executors[#executor].exe(process)
        },
    };

    quote! {{
        let process = maestro::Process::new(
            #name.to_string(),
            #container,
            vec![#(#input_pairs),*],
            vec![#(#arg_pairs),*],
            vec![#(#output_pairs),*],
            ::std::borrow::Cow::Borrowed(#process_lit),
        );
        #executor
    }}
    .into()
}

static GENERATED_HASHES: LazyLock<Mutex<FxHashSet<String>>> =
    LazyLock::new(|| Mutex::new(FxHashSet::default()));
