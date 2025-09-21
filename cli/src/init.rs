use std::{env, fs, path::Path};

use crate::{
    StringResult, cache::prep_cache, dedent, dynamic_err, mapper, rustc_version, static_err,
};

pub(crate) fn initialize(path: Option<String>) -> StringResult {
    let workdir = match path {
        None => {
            let current_dir = env::current_dir()
                .map_err(|e| mapper(&e, "Failed to identify working directory"))?;
            let is_empty = fs::read_dir(&current_dir)
                .map_err(|e| mapper(&e, "Failed to read current directory contents"))?
                .next()
                .is_none();
            if is_empty {
                current_dir
            } else {
                return Err(static_err("Current directory is not empty!"));
            }
        }
        Some(p) => {
            let path = Path::new(&p);
            if path.exists() {
                return Err(dynamic_err(format!("Path {p} is not empty!")));
            } else {
                fs::create_dir_all(path)
                    .map_err(|e| mapper(&e, "Failed to create project directory"))?;
                path.to_path_buf()
            }
        }
    };

    {
        let cargo_toml = workdir.join("Cargo.toml");
        let template = include_str!("../templates/Cargo.toml");
        let crate_name = workdir.file_name().ok_or(dynamic_err(format!(
            "Failed to resolve filename of directory {}",
            workdir.display()
        )))?;
        fs::write(
            cargo_toml,
            dedent(format!(
                r#"
                [package]
                name = "{}"
                {template}
                "#,
                crate_name.display()
            )),
        )
        .map_err(|e| mapper(&e, "Failed to write Cargo.toml"))?;
    }
    {
        let maestro_toml = workdir.join("Maestro.toml");
        fs::write(maestro_toml, include_str!("../templates/Maestro.toml"))
            .map_err(|e| mapper(&e, "Failed to write Maestro.toml"))?;
    }
    {
        let gitignore = workdir.join(".gitignore");
        fs::write(gitignore, include_str!("../templates/.gitignore"))
            .map_err(|e| mapper(&e, "Failed to write .gitignore"))?;
    }
    {
        let data_dir = workdir.join("data");
        fs::create_dir(&data_dir).map_err(|e| mapper(&e, "Failed to create data/"))?;
        fs::write(
            data_dir.join("greeting.txt"),
            include_str!("../templates/greeting.txt"),
        )
        .map_err(|e| mapper(&e, "Failed to write data/greeting.txt"))?;
    }
    {
        let src_dir = workdir.join("src");
        fs::create_dir(&src_dir).map_err(|e| mapper(&e, "Failed to create src/"))?;
        fs::write(
            src_dir.join("main.rs"),
            include_str!("../templates/main.rs"),
        )
        .map_err(|e| mapper(&e, "Failed to write src/main.rs"))?;
    }
    {
        let rustc_version = rustc_version()?;
        fs::write(
            workdir.join("rust-toolchain.toml"),
            dedent(format!(
                r#"
                [toolchain]
                channel = "{rustc_version}"
                "#
            )),
        )
        .map_err(|e| mapper(&e, "Failed to write rust-toolchain.toml"))?;
    }

    env::set_current_dir(&workdir)
        .map_err(|e| mapper(&e, "Failed to set current dir to newly initialized project"))?;
    prep_cache()?;

    Ok(())
}
