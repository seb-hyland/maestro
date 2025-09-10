use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rand::{Rng as _, distr::Uniform};
use std::{fs, path::Path};
use syn::{
    Expr, Ident, LitBool, LitStr, bracketed,
    parse::Parse,
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Eq},
};

// mod checker;
// mod container;

struct ProcessDefinition {
    name: Option<Expr>,
    inputs: Punctuated<Ident, Comma>,
    outputs: Punctuated<Ident, Comma>,
    args: Punctuated<Ident, Comma>,
    inline: bool,
    literal: LitStr,
}

mod kw {
    use syn::custom_keyword;
    custom_keyword!(name);
    custom_keyword!(inputs);
    custom_keyword!(outputs);
    custom_keyword!(args);
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

        macro_rules! parse_args {
            ($input:expr, $token:path) => {
                if $input.peek($token) {
                    let _: $token = $input.parse()?;
                    let _: Eq = $input.parse()?;
                    let process_inputs;
                    bracketed!(process_inputs in $input);
                    let parsed = process_inputs.parse_terminated(Ident::parse, Comma);
                    let _: Comma = input.parse()?;
                    parsed
                } else {
                    Ok(Punctuated::new())
                }
            };
        }
        let inputs = parse_args!(input, kw::inputs)?;
        let args = parse_args!(input, kw::args)?;
        let outputs = parse_args!(input, kw::outputs)?;

        let inline = if input.peek(kw::inline) {
            let _: kw::inline = input.parse()?;
            let _: Eq = input.parse()?;
            let bool: LitBool = input.parse()?;
            let _: Comma = input.parse()?;
            bool.value
        } else {
            false
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
    let process = if definition.inline {
        let trimmed_lit = literal_value.trim();
        if !trimmed_lit.starts_with("#!") {
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

    // Make a copy and append environment variables to stop shellcheck yapping abt undefined vars
    // let mut presented_contents = file_contents.clone();
    // env_vars
    //     .iter()
    //     .map(|ident| ident.to_string())
    //     .for_each(|name| {
    //         let script_injection = format!("{name}=\"\"\n");
    //         presented_contents.push_str(&script_injection);
    //     });

    // if let Err((msg, e)) = checker::run_shellcheck(&presented_contents, Some(&path)) {
    //     abort! {
    //         path_lit.span(),
    //         "{}\n{}",
    //         msg,
    //         e
    //     }
    // }

    let process_lit = LitStr::new(&process, literal.span());
    fn into_pairs(args: Punctuated<Ident, Comma>) -> impl IntoIterator<Item = TokenStream2> {
        args.into_iter().map(|ident| {
            let lit = LitStr::new(&ident.to_string(), ident.span());
            quote! { (#lit, PathBuf::from(#ident))}
        })
    }
    let input_pairs = into_pairs(definition.inputs).into_iter();
    let output_pairs = into_pairs(definition.outputs).into_iter();
    let arg_pairs = definition.args.into_iter().map(|ident| {
        let lit = LitStr::new(&ident.to_string(), ident.span());
        quote! { (#lit, #ident.to_string())}
    });

    fn generate_hashed_name() -> String {
        let rng = rand::rng();
        let letter_sample =
            Uniform::new_inclusive('a', 'z').expect("Uniform character sampling should not fail!");
        rng.sample_iter(letter_sample).take(10).collect()
    }
    let name = match definition.name {
        Some(expr) => quote! {{ #expr }},
        None => {
            let name = generate_hashed_name();
            quote! { #name }
        }
    };

    quote! {
        maestro::Process::new(
            #name.to_string(),
            #process_lit,
            vec![#(#input_pairs),*],
            vec![#(#output_pairs),*],
            vec![#(#arg_pairs),*]
        )
    }
    .into()
}

// Example usage:
// ```rust
// oci!("rust:alpine3.22")
// ```
// #[proc_macro]
// #[proc_macro_error]
// pub fn oci(input: TokenStream) -> TokenStream {
//     let name_lit = parse_macro_input!(input as LitStr);
//     if let Err(e) = container::check_manifest(&name_lit.value()) {
//         abort! {
//             Span::call_site(),
//             e
//         }
//     }
//     quote! {
//         ::finalflow::prelude::Oci(#name_lit)
//     }
//     .into()
// }

// /// Example usage:
// /// ```rust
// /// sif!("rust:alpine3.22")
// /// ```
// #[proc_macro]
// #[proc_macro_error]
// pub fn sif(input: TokenStream) -> TokenStream {
//     let name_lit = parse_macro_input!(input as LitStr);
//     let name = name_lit.value();
//     if let Err(e) = container::verify_sif(&name) {
//         abort! {
//             Span::call_site(),
//             e
//         }
//     }
//     quote! {
//         ::finalflow::SIF(#name_lit)
//     }
//     .into()
// }
