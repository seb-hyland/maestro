use std::{
    io::Write,
    process::{Command, Stdio},
};
use syn::{Ident, LitStr, parse::Parse, punctuated::Punctuated, token::Comma};

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

pub(crate) fn run_shellcheck(input: &str) -> Result<(), (String, String)> {
    let mut child = Command::new("shellcheck")
        .arg("-")
        .args(["-f", "gcc"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ("Failed to spawn script checker!".to_string(), e.to_string()))?;

    // Drop stdin handle
    {
        let stdin = child.stdin.as_mut().ok_or((
            "Failed to get input handle to script checker!".to_string(),
            String::new(),
        ))?;
        stdin.write_all(input.as_bytes()).map_err(|e| {
            (
                "Failed to pass script contents into script checker!".to_string(),
                e.to_string(),
            )
        })?;
    }

    let output = child.wait_with_output().map_err(|e| {
        (
            "Failed to wait on script checker!".to_string(),
            e.to_string(),
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(("The script has errors!".to_string(), stderr))
    } else {
        Ok(())
    }
}
