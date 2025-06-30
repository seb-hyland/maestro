use crate::checker::ScriptDefinition;

use proc_macro::{Span, TokenStream};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use std::{fs, path::Path};
use syn::{
    FnArg, ItemFn, LitStr, Pat, ReturnType, Stmt, Type, parse_macro_input, parse_quote,
    spanned::Spanned,
};

mod checker;
mod container;

#[proc_macro]
#[proc_macro_error]
pub fn script(input: TokenStream) -> TokenStream {
    let ScriptDefinition { path_lit, env_vars } = parse_macro_input!(input as ScriptDefinition);

    let path_str = path_lit.value();
    let path: &Path = path_str.as_ref();
    let file_contents = fs::read_to_string(path).unwrap_or_else(|e| {
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

    if let Err(e) = checker::run_shellcheck(&presented_contents) {
        abort! {
            Span::call_site(),
            e
        }
    }

    let file_contents_lit = LitStr::new(&file_contents, path_lit.span());
    let env_var_lits: Vec<LitStr> = env_vars
        .iter()
        .map(|ident| LitStr::new(&ident.to_string(), ident.span()))
        .collect();
    quote! {
        let env_vars: Vec<::finalflow::EnvVar> = vec! [
            #(
                ::finalflow::EnvVar(
                    #env_var_lits,
                    #env_vars.into()
                )
            ),*
        ];
        ::finalflow::Script { contents: #file_contents_lit, env: env_vars, runtime: ::finalflow::Runtime::Local }
    }
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
        ::finalflow::OCI(#name_lit)
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
    let mut input = parse_macro_input!(input as ItemFn);
    let mut path_checks = input
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(v) = arg {
                Some(v)
            } else if let FnArg::Receiver(v) = arg {
                abort! {
                    v.span(),
                    "Associated methods cannot be annotated with #[workflow]!"
                }
            } else {
                None
            }
        })
        .map(|arg| (&*arg.ty, &*arg.pat))
        .filter_map(|(ty, pat)| match ty {
            Type::Path(type_path) => {
                let name = match pat {
                    Pat::Ident(v) => v.ident.clone(),
                    _ => abort! {
                        pat.span(),
                        "Non-ident argument pattern detected!"
                    },
                };
                let type_ident = type_path
                    .path
                    .segments
                    .last()
                    .unwrap_or_else(|| {
                        abort! {
                            ty.span(),
                            "Failed to parse input type!"
                        }
                    })
                    .ident
                    .clone();
                match type_ident.to_string().as_str() {
                    "PathBuf" => Some(name),
                    "String" => None,
                    _ => abort!(
                        ty.span(),
                        "#[workflow] annotated functions must only take String or PathBuf as input"
                    ),
                }
            }
            _ => abort! {
                ty.span(),
                "#[workflow] annotated functions must only take String or PathBuf as input"
            },
        })
        .map(|ident| -> Stmt {
            parse_quote! {
                if !#ident.exists() {
                    return ::finalflow::WorkflowResult::Err(#ident);
                }
            }
        })
        .collect::<Vec<_>>();
    path_checks.push(parse_quote! { (); });
    input.block.stmts.splice(0..0, path_checks);

    const RETURN_MSG: &str = "#[workflow] functions must return finalflow::WorkflowResult";
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
                    != "WorkflowResult"
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

    quote! {
        #input
    }
    .into()
}
