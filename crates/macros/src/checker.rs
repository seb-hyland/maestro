use proc_macro::Span;
use std::{
    io::Write,
    process::{Command, Stdio},
};
use syn::{Error, Ident, LitStr, parse::Parse, punctuated::Punctuated, token::Comma};

pub(crate) struct ScriptDefinition {
    pub(crate) path_lit: LitStr,
    pub(crate) env_vars: Punctuated<Ident, Comma>,
}

impl Parse for ScriptDefinition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path_lit: LitStr = input.parse()?;
        let _comma: Option<Comma> = input.parse().ok();

        let env_vars = Punctuated::parse_terminated(input)?;
        Ok(ScriptDefinition { path_lit, env_vars })
    }
}

pub(crate) fn run_shellcheck(input: &str, span: Span) -> Result<(), Error> {
    let mut child = Command::new("shellcheck")
        .arg("-")
        .args(["-f", "gcc"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::new(span.into(), format!("Failed to spawn script checker!\n{e}")))?;

    // Drop stdin handle
    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            Error::new(span.into(), "Failed to get input handle to script checker!")
        })?;
        stdin.write_all(input.as_bytes()).map_err(|e| {
            Error::new(
                span.into(),
                format!("Failed to pass script contents into script checker!\n{e}"),
            )
        })?;
    }

    let output = child.wait_with_output().map_err(|e| {
        Error::new(
            span.into(),
            format!("Failed to wait on script checker!\n{e}"),
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::new(
            span.into(),
            format!("The script has errors!\n{stderr}"),
        ))
    } else {
        Ok(())
    }
}
