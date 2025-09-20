use std::{ffi::OsStr, fs, os::unix::ffi::OsStrExt, process::Command};

use crate::{StringResult, TOOLCHAIN_VERSION, dynamic_err, find_crate_root, mapper};

pub(crate) fn build_and_bundle(passed_args: Vec<String>) -> StringResult {
    let crate_root = find_crate_root()?;
    let bundle_dir = crate_root.join("maestro_bundle");
    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir)
            .map_err(|e| mapper(&e, "Failed to clean maestro_bundle directory"))?;
    }
    fs::create_dir_all(&bundle_dir)
        .map_err(|e| mapper(&e, "Failed to create maestro_bundle directory"))?;

    let toolchain_prep = Command::new("rustup")
        .args(["toolchain", "install", TOOLCHAIN_VERSION])
        .status()
        .map_err(|e| mapper(&e, "Failed to install required Rust toolchain"))?;
    if !toolchain_prep.success() {
        let err_msg = match toolchain_prep.code() {
            Some(code) => format!("Build failed with exit code {code}"),
            None => "Build failed due to external signal".to_string(),
        };
        return Err(dynamic_err(err_msg));
    }

    let build = Command::new("rustup")
        .args([
            "run",
            TOOLCHAIN_VERSION,
            "cargo",
            "build",
            "--release",
            "--no-default-features",
        ])
        .args(passed_args)
        .current_dir(&crate_root)
        .status()
        .map_err(|e| mapper(&e, "Failed to build project"))?;
    if !build.success() {
        let err_msg = match build.code() {
            Some(code) => format!("Build failed with exit code {code}"),
            None => "Build failed due to external signal".to_string(),
        };
        return Err(dynamic_err(err_msg));
    }

    let target_dir = crate_root.join("target/release");
    for file in fs::read_dir(&target_dir)
        .map_err(|e| mapper(&e, "Failed to read contents of build directory"))?
        .flatten()
    {
        let path = file.path();
        if path.extension() == Some(OsStr::from_bytes(b"d")) {
            let binary_path = path.with_extension("");
            fs::copy(
                &binary_path,
                bundle_dir.join(binary_path.file_name().unwrap()),
            )
            .map_err(|e| mapper(&e, "Failed to copy build outputs to bundle directory"))?;
        }
    }

    fs::copy(
        crate_root.join("Maestro.toml"),
        bundle_dir.join("Maestro.toml"),
    )
    .map_err(|e| mapper(&e, "Failed to copy Maestro.toml to bundle directory"))?;
    fs::copy(
        crate_root.join("dependencies.txt"),
        bundle_dir.join("dependencies.txt"),
    )
    .map_err(|e| mapper(&e, "Failed to copy dependencies.txt to bundle directory"))?;

    Ok(())
}
