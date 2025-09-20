use crate::{
    StringErr, dedent, find_crate_root, mapper, report_process_failure, rustc_version, static_err,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

pub(crate) fn prep_cache() -> Result<PathBuf, StringErr> {
    let crate_root = find_crate_root()?;
    let cache_dir = crate_root.join(".maestro_cache");
    if !cache_dir.exists() {
        fs::create_dir(&cache_dir).map_err(|e| mapper(&e, "Failed to create .maestro_cache/"))?;
    }

    let cmd = Command::new("cargo")
        .args(["vendor", "--versioned-dirs", ".maestro_cache/vendor"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(&crate_root)
        .status()
        .map_err(|e| mapper(&e, "Failed to vendor dependencies"))?;
    if !cmd.success() {
        return Err(report_process_failure(cmd, "Dependency vendoring"));
    }

    let vendor_dir = cache_dir.join("vendor");
    let mut maestro_dir = None;
    let mut maestro_macro_dir = None;
    for dependency in fs::read_dir(&vendor_dir)
        .map_err(|e| mapper(&e, "Failed to read vendor directory"))?
        .flatten()
    {
        if let Ok(file_type) = dependency.file_type()
            && file_type.is_dir()
        {
            let file_name_ref = dependency.file_name();
            let file_name = file_name_ref.to_string_lossy();
            if file_name.contains("maestro-macros") {
                maestro_macro_dir = Some(dependency.path());
            } else if file_name.contains("maestro") {
                maestro_dir = Some(dependency.path());
            }
        }
    }
    if maestro_dir.is_none() || maestro_macro_dir.is_none() {
        return Err(static_err(
            "Failed to find maestro in vendored dependencies",
        ));
    }

    let maestro_dir = maestro_dir.unwrap();
    let maestro_macro_dir = maestro_macro_dir.unwrap();
    let maestro_macro_dirname = maestro_macro_dir.file_name().unwrap();

    let maestro_dirname = maestro_dir.file_name().unwrap().to_string_lossy();
    let maestro_version = maestro_dirname.strip_prefix("maestro-").unwrap();

    let rustc_version = rustc_version()?;
    let output_dir = cache_dir.join(format!("maestro-{maestro_version}_rustc-{rustc_version}"));
    if output_dir.exists() {
        return Ok(output_dir);
    }

    let maestro_toml = maestro_dir.join("Cargo.toml");
    let maestro_toml_str = fs::read_to_string(&maestro_toml)
        .map_err(|e| mapper(&e, "Failed to read Cargo.toml of libmaestro"))?;
    let maestro_toml_updated = maestro_toml_str
        .lines()
        .map(|l| {
            if l.contains("path = \"../proc\"") {
                format!("path = \"../{}\"", maestro_macro_dirname.display())
            } else {
                l.to_string()
            }
        })
        .reduce(|mut acc, s| {
            acc.push_str(&s);
            acc.push('\n');
            acc
        })
        .unwrap_or_default();
    fs::write(maestro_toml, maestro_toml_updated.as_bytes())
        .map_err(|e| mapper(&e, "Failed to update Cargo.toml of libmaestro"))?;

    let config_dir = maestro_dir.join(".cargo/");
    fs::create_dir(&config_dir).map_err(|e| {
        mapper(
            &e,
            "Failed to create .cargo directory while building libmaestro",
        )
    })?;
    fs::write(
        config_dir.join("config.toml"),
        dedent(
            r#"
            [source.crates-io]
            replace-with = "vendored-sources"

            [source.vendored-sources]
            directory = "../"
            "#,
        ),
    )
    .map_err(|e| mapper(&e, "Failed to write config.toml while building libmaestro"))?;

    let build_dep = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&maestro_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| mapper(&e, "Failed to build libmaestro"))?;
    if !build_dep.success() {
        return Err(report_process_failure(build_dep, "Building libmaestro"));
    }

    fs::create_dir(&output_dir)
        .map_err(|e| mapper(&e, "Failed to create dependency cache directory"))?;
    let target_dir = maestro_dir.join("target/release/");
    fs::copy(
        target_dir.join("libmaestro.rlib"),
        output_dir.join("libmaestro.rlib"),
    )
    .map_err(|e| mapper(&e, "Failed to copy libmaestro to dependency cache"))?;

    let dep_dir = output_dir.join("deps");
    fs::create_dir(&dep_dir)
        .map_err(|e| mapper(&e, "Failed to create libmaestro dependency cache directory"))?;
    for dep in fs::read_dir(target_dir.join("deps/"))
        .map_err(|e| mapper(&e, "Failed to read libmaestro dependency directory"))?
    {
        let dep = dep.map_err(|e| {
            mapper(
                &e,
                "Failed to read all entries in libmaestro dependency directory",
            )
        })?;
        let path = dep.path();

        fs::copy(path, dep_dir.join(dep.file_name())).map_err(|e| {
            mapper(
                &e,
                "Failed to copy libmaestro dependency to cache directory",
            )
        })?;
    }

    Ok(output_dir)
}
