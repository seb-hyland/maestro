use crate::checker::ScriptDefinition;

use proc_macro::{Span, TokenStream};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use std::{fs, path::Path};
use syn::{ItemFn, LitStr, ReturnType, Type, parse_macro_input, spanned::Spanned};

mod checker;
mod container;

#[proc_macro]
#[proc_macro_error]
pub fn script(input: TokenStream) -> TokenStream {
    let ScriptDefinition { path_lit, env_vars } = parse_macro_input!(input as ScriptDefinition);

    let path_str = path_lit.value();
    let path_stub: &Path = path_str.as_ref();
    let path = path_stub.canonicalize().unwrap_or_else(|e| {
        abort! {
            path_lit.span(),
            format!("Unable to canonicalize script path!\n{e}")
        }
    });
    let file_contents = fs::read_to_string(&path).unwrap_or_else(|e| {
        abort! {
            path_lit.span(),
            format!(
                "The file `{path_str}` could not be opened!\n{e}",
            ),
        }
    });

    // Make a copy and append environment variables to stop shellcheck yapping abt undefined vars
    let mut presented_contents = file_contents.clone();
    env_vars
        .iter()
        .map(|ident| ident.to_string())
        .for_each(|name| {
            let script_injection = format!("{name}=\"\"\n");
            presented_contents.push_str(&script_injection);
        });

    if let Err((msg, e)) = checker::run_shellcheck(&presented_contents, Some(&path)) {
        abort! {
            path_lit.span(),
            "{}\n{}",
            msg,
            e
        }
    }

    let file_contents_lit = LitStr::new(&file_contents, path_lit.span());
    let env_var_lits: Vec<LitStr> = env_vars
        .iter()
        .map(|ident| LitStr::new(&ident.to_string(), ident.span()))
        .collect();
    quote! {{
        let env_vars: Vec<::finalflow::prelude::EnvVar> = vec! [
            #(
                ::finalflow::prelude::EnvVar(
                    #env_var_lits,
                    #env_vars.into()
                )
            ),*
        ];
        ::finalflow::prelude::Script { contents: #file_contents_lit, env: env_vars }
    }}
    .into()
}

/// Example usage:
/// ```rust
/// oci!("rust:alpine3.22")
/// ```
#[proc_macro]
#[proc_macro_error]
pub fn oci(input: TokenStream) -> TokenStream {
    let name_lit = parse_macro_input!(input as LitStr);
    if let Err(e) = container::check_manifest(&name_lit.value()) {
        abort! {
            Span::call_site(),
            e
        }
    }
    quote! {
        ::finalflow::prelude::Oci(#name_lit)
    }
    .into()
}

/// Example usage:
/// ```rust
/// sif!("rust:alpine3.22")
/// ```
#[proc_macro]
#[proc_macro_error]
pub fn sif(input: TokenStream) -> TokenStream {
    let name_lit = parse_macro_input!(input as LitStr);
    let name = name_lit.value();
    if let Err(e) = container::verify_sif(&name) {
        abort! {
            Span::call_site(),
            e
        }
    }
    quote! {
        ::finalflow::SIF(#name_lit)
    }
    .into()
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn workflow(_attributes: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);

    const RETURN_MSG: &str = "#[workflow] functions must return finalflow::Workflow";
    match &input.sig.output {
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Path(path) => {
                if path
                    .path
                    .segments
                    .last()
                    .unwrap_or_else(|| {
                        abort! {
                            ty.span(),
                            format!("Empty return type!\n{RETURN_MSG}")
                        }
                    })
                    .ident
                    != "Workflow"
                {
                    abort! {
                        ty.span(),
                        RETURN_MSG
                    }
                }
            }
            _ => abort! {
                ty.span(),
                RETURN_MSG
            },
        },
        _ => abort! {
            input.sig.output.span(),
            RETURN_MSG
        },
    };

    let name = input.sig.ident.to_string();
    if name.contains('_') || !name.chars().next().unwrap_or('a').is_ascii_uppercase() {
        abort! {
            input.sig.ident.span(),
            "Workflow names must be UpperCamelCase to distinguish from regular functions!"
        }
    }

    let vis = input.vis;
    let sig = input.sig;
    let block = input.block;

    quote! {
        #[allow(non_snake_case)]
        #vis #sig #block
    }
    .into()
}
