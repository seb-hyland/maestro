use proc_macro::TokenStream;
use quote::quote;
use std::{fs, path::Path};
use syn::{Ident, LitStr, parse::Parse, parse_macro_input, punctuated::Punctuated, token::Comma};

// mod checker;
// mod container;

struct ScriptDefinition {
    path_lit: LitStr,
    env_vars: Punctuated<Ident, Comma>,
}

impl Parse for ScriptDefinition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path_lit: LitStr = input.parse()?;
        let _comma: Option<Comma> = input.parse().ok();

        let env_vars = Punctuated::parse_terminated(input)?;
        Ok(ScriptDefinition { path_lit, env_vars })
    }
}

#[proc_macro]
pub fn inline_process(input: TokenStream) -> TokenStream {
    let ScriptDefinition {
        path_lit: script_lit,
        env_vars,
    } = parse_macro_input!(input as ScriptDefinition);

    let pairs = env_vars.into_iter().map(|ident| {
        let lit = LitStr::new(&ident.to_string(), ident.span());
        quote! { (#lit, #ident.into()) }
    });
    quote! {
        maestro::Script {
            script: #script_lit,
            vars: &mut [
                #(#pairs),*
            ]
        }
    }
    .into()
}

#[proc_macro]
pub fn process(input: TokenStream) -> TokenStream {
    let ScriptDefinition { path_lit, env_vars } = parse_macro_input!(input as ScriptDefinition);

    let path_str = path_lit.value();
    let path: &Path = path_str.as_ref();
    if !path.exists() {
        return syn::Error::new(
            path_lit.span(),
            format!("The file `{path_str}` does not exist!"),
        )
        .into_compile_error()
        .into();
    }
    let file_contents = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) => {
            return syn::Error::new(
                path_lit.span(),
                format!("The file `{path_str}` could not be read:\n{e:?}"),
            )
            .into_compile_error()
            .into();
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

    let file_contents_lit = LitStr::new(&file_contents, path_lit.span());

    let pairs = env_vars.into_iter().map(|ident| {
        let lit = LitStr::new(&ident.to_string(), ident.span());
        quote! { (#lit, #ident.into()) }
    });
    quote! {
        maestro::Script {
            script: #file_contents_lit,
            vars: &mut [
                #(#pairs),*
            ]
        }
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
