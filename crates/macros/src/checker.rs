use regex::Regex;
use std::{
    io::Write,
    path::Path,
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

pub(crate) fn run_shellcheck(input: &str, origin: Option<&Path>) -> Result<(), (String, String)> {
    let mut child = Command::new("shellcheck")
        .arg("-")
        .args(["-f", "gcc"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
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
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Ignore errors associated with a line >= file_lines
        // These errors are due to injected text, not the file itself
        // Typically caused by Rust variables not lining up with script variables
        let file_lines = input.lines().count();
        let line_pattern =
            Regex::new(r"^-:(\d+):").expect("shellcheck output replacement regex should be valid!");
        let combined = format!("{stderr}{stdout}")
            .lines()
            .filter(|line| {
                if let Some(captures) = line_pattern.captures(line) {
                    // Index 1 is first sub-group, 0 is entire str
                    if let Some(capture) = captures.get(1) {
                        if let Ok(line_num) = capture.as_str().parse::<usize>() {
                            return line_num < file_lines;
                        }
                    }
                }
                true
            })
            .collect::<Vec<_>>()
            .join("\n");
        let combined = if let Some(p) = origin {
            combined.replace("-:", &format!("{}:", p.display()))
        } else {
            combined
        };
        Err(("The script has errors!".to_string(), combined))
    } else {
        Ok(())
    }
}
