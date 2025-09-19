use std::{
    io::Write,
    process::{Command, Stdio},
};

pub(crate) fn run_shellcheck(
    input: &str,
    path: Option<&str>,
    has_shebang: bool,
    injection_count: usize,
) -> Result<(), (String, String)> {
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
        let file_lines = input.lines().count() - injection_count;
        let combined = format!("{stderr}{stdout}")
            .lines()
            .filter_map(|line| {
                if let Some(rest) = line.strip_prefix("-:")
                    // Extract line number
                    && let Some(colon_pos) = rest.find(':')
                {
                    let line_num_str = &line[2..2 + colon_pos];
                    if let Ok(mut line_num) = line_num_str.parse::<usize>() {
                        if !has_shebang {
                            line_num -= 1;
                        }
                        if line_num < file_lines {
                            return Some(format!(
                                "{}{}{}",
                                &line[0..2],
                                line_num,
                                &line[2 + colon_pos..]
                            ));
                        }
                    }
                }
                None
            })
            .collect::<Vec<_>>()
            .join("\n");
        let combined = if let Some(p) = path {
            combined.replace("-:", &format!("{}:", p))
        } else {
            combined.replace("-:", "")
        };
        if combined.is_empty() {
            Ok(())
        } else {
            Err(("The script has errors!".to_string(), combined))
        }
    } else {
        Ok(())
    }
}
