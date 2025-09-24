use crate::dep_analysis::{
    ContainerDependency, DEPENDENCIES_FILE, ProcessDependencies, SHELL_EXCLUDES, analyze_depends,
};
use fxhash::FxHashSet;
use proc_macro::{
    Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree, token_stream,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rand::{Rng as _, distr::Uniform};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::Write as _,
    iter::Peekable,
    mem,
    path::Path,
    sync::{LazyLock, Mutex},
};
use syn::{
    Expr, Ident as SynIdent, LitBool, LitStr, bracketed, parenthesized,
    parse::{self, Parse},
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Eq},
};

#[cfg(feature = "check_scripts")]
mod checker;
mod dep_analysis;

struct ProcessDefinition {
    name: Option<Expr>,
    executor: LitStr,
    container: Option<Container>,
    inputs: Punctuated<SynIdent, Comma>,
    args: Punctuated<SynIdent, Comma>,
    outputs: Punctuated<SynIdent, Comma>,
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
        let mut name = None;
        let mut declared_executor = None;
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
            if input.peek(kw::name) {
                let _: kw::name = input.parse()?;
                let _: Eq = input.parse()?;
                name = Some(input.parse()?);
            } else if input.peek(kw::executor) {
                let _: kw::executor = input.parse()?;
                let _: Eq = input.parse()?;
                declared_executor = Some(input.parse()?);
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
                parse_list!(inputs, SynIdent::parse)
            } else if input.peek(kw::args) {
                parse_list!(args, SynIdent::parse)
            } else if input.peek(kw::outputs) {
                parse_list!(outputs, SynIdent::parse)
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
        let executor = match declared_executor {
            Some(v) => v,
            None => {
                return Err(syn::Error::new(
                    input.span(),
                    "Missing required `executor` field!",
                ));
            }
        };

        Ok(ProcessDefinition {
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
    let mut input_iter = input.clone().into_iter().peekable();
    let (doc_strings, rest) = {
        fn parse_doc(iter: &mut Peekable<token_stream::IntoIter>) -> Result<String, ()> {
            let first_item = match iter.next() {
                Some(TokenTree::Punct(p)) => p,
                _ => unreachable!(),
            };
            if first_item.as_char() == '#' {
                if let Some(TokenTree::Group(p)) = iter.next()
                    && p.delimiter() == Delimiter::Bracket
                {
                    let mut inner_stream = p.stream().into_iter();
                    if let Some(doc_ident_token) = inner_stream.next()
                        && let TokenTree::Ident(doc_ident) = doc_ident_token
                        && doc_ident.to_string() == "doc"
                        && let Some(equal_token) = inner_stream.next()
                        && let TokenTree::Punct(equal_punct) = equal_token
                        && equal_punct.as_char() == '='
                        && let Some(doc_token) = inner_stream.next()
                        && let TokenTree::Literal(doc) = doc_token
                    {
                        let doc_str = doc.to_string();
                        let trimmed_str = doc_str.trim();
                        Ok(trimmed_str[1..doc_str.len() - 1].trim().to_string())
                    } else {
                        Err(())
                    }
                } else {
                    Err(())
                }
            } else {
                Err(())
            }
        }
        let mut docs = Vec::new();
        while let Some(TokenTree::Punct(_)) = input_iter.peek() {
            match parse_doc(&mut input_iter) {
                Ok(lit) => docs.push(lit),
                Err(_) => {
                    return syn::Error::new(
                        Span::call_site().into(),
                        "process! macro must begin with either a docstring or an identifier",
                    )
                    .into_compile_error()
                    .into();
                }
            }
        }
        (docs, input_iter.collect())
    };
    let definition = parse_macro_input!(rest as ProcessDefinition);

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
        let mut inject = |arg: &SynIdent| {
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
    fn into_pairs(args: Punctuated<SynIdent, Comma>) -> impl IntoIterator<Item = TokenStream2> {
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
    let mut ignore_all_autodetected = false;
    for dependency_lit in definition.dependencies {
        let dependency = dependency_lit.value();
        if dependency == "!" {
            ignore_all_autodetected = true;
        }
        if let Some(excluded_dep) = dependency.strip_prefix('!') {
            excludes.insert(excluded_dep.to_string());
        } else {
            dependencies.insert(dependency);
        }
    }
    if !ignore_all_autodetected {
        analyze_depends(&process, &mut excludes, &mut dependencies);
    }

    let process_span = literal.span();
    let mut root = HashMap::new();
    let dependencies = ProcessDependencies {
        executor: definition.executor.value(),
        container: definition
            .container
            .as_ref()
            .map(|container| match container {
                Container::Docker(img) => ContainerDependency::Docker {
                    container_image: img.value(),
                },
                Container::Apptainer(img) => ContainerDependency::Apptainer {
                    container_image: img.value(),
                },
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
    let docstring = doc_strings
        .into_iter()
        .map(|s| format!("# {s}"))
        .reduce(|mut acc, s| {
            acc.push('\n');
            acc.push_str(&s);
            acc
        })
        .map(|mut str| {
            str.push('\n');
            str
        })
        .unwrap_or("".to_string());

    {
        let mut lock = DEPENDENCIES_FILE.lock().unwrap();
        if let Err(e) = writeln!(lock, "{docstring}{toml_str}") {
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
    let executor = definition.executor;
    let executor_tokens = quote! {
        maestro::submit_request! {
            maestro::RequestedExecutor(#executor, file!(), line!(), column!())
        };
        maestro::config::MAESTRO_CONFIG.executors[#executor].exe(process)
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
        #executor_tokens
    }}
    .into()
}

static GENERATED_HASHES: LazyLock<Mutex<FxHashSet<String>>> =
    LazyLock::new(|| Mutex::new(FxHashSet::default()));

#[proc_macro_attribute]
pub fn main(attrs: TokenStream, body: TokenStream) -> TokenStream {
    if !attrs.is_empty() {
        return construct_error_stream(
            "#[maestro::main] does not accept attributes!",
            attrs.into_iter().next().unwrap().span(),
        );
    }
    let mut token_iter = body.clone().into_iter().enumerate();
    if !token_iter.any(|(_, token)| {
        if let TokenTree::Ident(ident) = token {
            ident.to_string() == "fn"
        } else {
            false
        }
    }) {
        return construct_error_stream(
            "#[maestro::main] can only be used on functions",
            Span::call_site(),
        );
    }

    let function_body = token_iter.find_map(|(i, token)| {
        if let TokenTree::Group(group) = token
            && group.delimiter() == Delimiter::Brace
        {
            Some((i, group.stream()))
        } else {
            None
        }
    });
    let (function_body_idx, mut function_body_vec): (usize, VecDeque<_>) = match function_body {
        Some((idx, body)) => (idx, body.into_iter().collect()),
        None => return construct_error_stream("Expected function body", Span::call_site()),
    };

    let start_tokens = [
        TokenTree::Ident(Ident::new("maestro", Span::call_site())),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("initialize", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new(';', Spacing::Joint)),
    ];
    for token in start_tokens.into_iter().rev() {
        function_body_vec.push_front(token);
    }

    let end_tokens = [
        TokenTree::Ident(Ident::new("maestro", Span::call_site())),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("deinitialize", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new(';', Spacing::Joint)),
    ];
    for token in end_tokens.into_iter() {
        function_body_vec.push_back(token);
    }

    let mut final_stream: Vec<TokenTree> = body.into_iter().collect();
    final_stream[function_body_idx] = TokenTree::Group(Group::new(
        Delimiter::Brace,
        function_body_vec.into_iter().collect(),
    ));
    final_stream.into_iter().collect()
}

fn construct_error_stream(msg: &str, span: Span) -> TokenStream {
    [
        TokenTree::Ident(proc_macro::Ident::new("compile_error", span)),
        TokenTree::Punct(Punct::new('!', Spacing::Alone)),
        {
            let mut group = TokenTree::Group(Group::new(Delimiter::Parenthesis, {
                [TokenTree::Literal(Literal::string(msg))]
                    .into_iter()
                    .collect()
            }));
            group.set_span(span);
            group
        },
    ]
    .into_iter()
    .collect()
}
