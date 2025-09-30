use crate::{
    StringResult,
    build::{BuildType, build_project},
    find_crate_root, mapper, report_process_failure,
};
use clap::ValueEnum;
use std::{
    ffi::OsStr,
    fs, io,
    os::unix::ffi::OsStrExt,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Compression {
    Zip,
    Gzip,
    Xz,
    Bzip2,
    Zstd,
    Lzma,
}

pub(crate) fn bundle_project(
    cargo_args: Vec<String>,
    compression: Option<Compression>,
) -> StringResult {
    let crate_root = find_crate_root()?;
    let bundle_dir = crate_root.join("maestro_bundle");

    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir)
            .map_err(|e| mapper(&e, "Failed to clean maestro_bundle directory"))?;
    }
    fs::create_dir_all(&bundle_dir)
        .map_err(|e| mapper(&e, "Failed to create maestro_bundle directory"))?;

    build_project(cargo_args, Vec::new(), BuildType::Build)?;

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
        crate_root.join("procinfo.toml"),
        bundle_dir.join("procinfo.toml"),
    )
    .map_err(|e| mapper(&e, "Failed to copy dependencies.toml to bundle directory"))?;

    fn copy_recursively(src: &Path, dst: &Path) -> io::Result<()> {
        fs::create_dir(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let entry_name = entry.file_name();
            if ty.is_dir() {
                copy_recursively(&entry.path(), &dst.join(entry_name))?;
            } else {
                fs::copy(entry.path(), dst.join(entry_name))?;
            }
        }
        Ok(())
    }
    copy_recursively(&crate_root.join("data/"), &bundle_dir.join("data/"))
        .map_err(|e| mapper(&e, "Failed to copy data/ to bundle directory"))?;

    if let Some(compression) = compression {
        let bundle_name = bundle_dir.file_name().unwrap();
        let mut command = match compression {
            Compression::Zip => {
                let mut cmd = Command::new("zip");
                cmd.current_dir(&crate_root)
                    .args(["-r", "maestro_bundle.zip"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Gzip => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["-czf", "maestro_bundle.tar.gz"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Xz => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["-cJf", "maestro_bundle.tar.xz"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Bzip2 => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["-cjf", "maestro_bundle.tar.bz2"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Zstd => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["--zstd", "-cf", "maestro_bundle.tar.zst"])
                    .arg(bundle_name);
                cmd
            }
            Compression::Lzma => {
                let mut cmd = Command::new("tar");
                cmd.current_dir(&crate_root)
                    .args(["--lzma", "-cf", "maestro_bundle.tar.lzma"])
                    .arg(bundle_name);
                cmd
            }
        };
        let status = command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| mapper(&e, "Failed to run compression"))?;
        if !status.success() {
            return Err(report_process_failure(status, "Compression"));
        }
    }

    Ok(())
}
