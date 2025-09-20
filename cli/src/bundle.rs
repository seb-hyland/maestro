use crate::{
    StringResult,
    build::{BuildType, build_project},
    find_crate_root, mapper,
};
use std::{ffi::OsStr, fs, io, os::unix::ffi::OsStrExt, path::Path};

pub(crate) fn build_and_bundle(additional_args: Vec<String>) -> StringResult {
    let crate_root = find_crate_root()?;
    let bundle_dir = crate_root.join("maestro_bundle");
    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir)
            .map_err(|e| mapper(&e, "Failed to clean maestro_bundle directory"))?;
    }
    fs::create_dir_all(&bundle_dir)
        .map_err(|e| mapper(&e, "Failed to create maestro_bundle directory"))?;

    build_project(additional_args, BuildType::Build)?;

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

    Ok(())
}
