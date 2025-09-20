use crate::{bundle::build_and_bundle, init::initialize, toolchain::install_toolchain};
use clap::Parser;
use std::{borrow::Cow, env, error::Error, path::PathBuf, process};

mod bundle;
mod init;
mod toolchain;

fn main() {
    let command = Command::parse();
    if let Err(e) = match command {
        Command::Init { path } => initialize(path),
        Command::Bundle { args } => build_and_bundle(args),
        Command::InstallToolchain { toolchain } => install_toolchain(toolchain),
    } {
        eprintln!("{e}");
        process::exit(1);
    }
}

#[derive(Parser)]
#[command(version, about)]
enum Command {
    Init {
        path: Option<String>,
    },
    Bundle {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    InstallToolchain {
        toolchain: String,
    },
}

const TOOLCHAIN_VERSION: &str = "1.90.0";

type StringResult = Result<(), Cow<'static, str>>;
fn mapper(e: &dyn Error, msg: &'static str) -> Cow<'static, str> {
    Cow::Owned(format!("{msg}: {e}"))
}
fn static_err(msg: &'static str) -> Cow<'static, str> {
    Cow::Borrowed(msg)
}
fn dynamic_err(msg: String) -> Cow<'static, str> {
    Cow::Owned(msg)
}

fn find_crate_root() -> Result<PathBuf, Cow<'static, str>> {
    let working_directory =
        env::current_dir().map_err(|e| mapper(&e, "Failed to identify working directory"))?;
    let mut current_dir = working_directory;
    while !current_dir.join("Cargo.toml").exists() {
        current_dir = match current_dir.parent() {
            Some(parent) => parent.to_path_buf(),
            None => {
                return Err(static_err(
                    "Failed to find Cargo.toml in a parent of the working directory",
                ));
            }
        }
    }
    Ok(current_dir)
}
