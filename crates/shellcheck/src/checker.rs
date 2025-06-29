use proc_macro::{Span, TokenStream};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use syn::{Ident, LitStr, parse::Parse, parse_macro_input};

struct ScriptDefinition {
    path_lit: LitStr,
    env_vars: Vec<Ident>,
}

impl Parse for ScriptDefinition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path_lit: LitStr = input.parse()?;

        let mut env_vars = Vec::new();
        while !input.is_empty() {
            let next_var: Ident = input.parse()?;
            env_vars.push(next_var);
        }

        Ok(ScriptDefinition { path_lit, env_vars })
    }
}

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

    let mut child = Command::new("shellcheck")
        .arg("-")
        .args(["-f", "gcc"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            abort! {
                Span::call_site(),
                format!("Failed to spawn script checker!\n{e}")
            }
        });

    // Drop stdin handle
    {
        let stdin = child.stdin.as_mut().unwrap_or_else(|| {
            abort! {
                Span::call_site(),
                "Failed to get input handle to script checker!"
            }
        });
        stdin
            .write_all(presented_contents.as_bytes())
            .unwrap_or_else(|e| {
                abort! {
                    Span::call_site(),
                    format!("Failed to pass script contents into script checker!\n{e}")
                }
            });
    }

    let output = child.wait_with_output().unwrap_or_else(|e| {
        abort! {
            Span::call_site(),
            format!("Failed to wait on script checker!\n{e}")
        }
    });

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        abort! {
            path_lit.span(),
            format!("The script has errors!\n{stderr}")
        }
    }

    let file_contents_lit = LitStr::new(&file_contents, path_lit.span());
    quote! {
        ::workflow::Script { contents: #file_contents_lit, runtime: ::workflow::Runtime::Local }
    }
    .into()
}
