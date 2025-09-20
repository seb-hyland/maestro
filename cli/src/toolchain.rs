use std::process::Command;

use crate::{StringResult, TOOLCHAIN_VERSION, dynamic_err, mapper};

pub(crate) fn install_toolchain(toolchain: String) -> StringResult {
    let install = Command::new("rustup")
        .args(["target", "add"])
        .arg(toolchain)
        .args(["--toolchain", TOOLCHAIN_VERSION])
        .status()
        .map_err(|e| mapper(&e, "Failed to install toolchain"))?;
    if !install.success() {
        let err_msg = match install.code() {
            Some(code) => format!("Toolchain installation failed with error code {code}"),
            None => "Toolchain installation failed due to external signal".to_string(),
        };
        Err(dynamic_err(err_msg))
    } else {
        Ok(())
    }
}
