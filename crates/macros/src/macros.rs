use crate::{
    checker::{ScriptDefinition, run_shellcheck},
    container::check_manifest,
};
use proc_macro::{Span, TokenStream};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use std::{fs, path::Path};
use syn::{LitStr, parse_macro_input};

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

    if let Err(e) = run_shellcheck(&presented_contents) {
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
        let env_vars: Vec<::workflow::EnvVar> = vec! [
            #(
                ::workflow::EnvVar(
                    #env_var_lits,
                    #env_vars.into()
                )
            ),*
        ];
        ::workflow::Script { contents: #file_contents_lit, env: env_vars, runtime: ::workflow::Runtime::Local }
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
    if let Err(e) = check_manifest(&name_lit.value()) {
        abort! {
            Span::call_site(),
            e
        }
    }
    quote! {
        #name_lit
    }
    .into()
}
