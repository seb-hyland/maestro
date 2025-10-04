use crate::{
    build::{BuildType, build_project},
    bundle::{Arch, Compression, ContainerRuntime, bundle_project},
    cache::prep_cache,
    init::initialize,
    kill::kill_process,
};
use clap::{
    Parser,
    builder::{
        Styles,
        styling::{AnsiColor, Color, Style},
    },
};
use std::{
    borrow::Cow,
    env,
    error::Error,
    path::PathBuf,
    process::{self, Command, ExitStatus},
};

mod build;
mod bundle;
mod cache;
mod init;
mod kill;

type StringErr = Cow<'static, str>;
type StringResult = Result<(), StringErr>;

fn main() {
    let command = Cmd::parse();
    if let Err(e) = match command {
        Cmd::Init { path } => initialize(path),
        Cmd::Bundle {
            cargo_args,
            compress,
            arch,
            runtime,
        } => bundle_project(
            cargo_args,
            compress,
            arch,
            runtime.unwrap_or(ContainerRuntime::Docker),
        ),
        Cmd::UpdateCache => prep_cache().map(|_| {}),
        Cmd::Build { cargo_args } => build_project(cargo_args, Vec::new(), BuildType::Build),
        Cmd::Run {
            binary,
            background,
            cargo_args,
            args,
        } => build_project(cargo_args, args, BuildType::Run { background, binary }),
        Cmd::Kill { target } => kill_process(&target),
    } {
        eprintln!("{e}");
        process::exit(1);
    }
}

#[derive(Parser)]
#[command(version, about, styles = help_style())]
/// Subcommands in the maestro CLI
pub enum Cmd {
    /// Initialize a new maestro project
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Compile a project and package it for redistribution
    Bundle {
        /// Compresses the bundle into an archive
        #[arg(short, long, value_enum)]
        compress: Option<Compression>,
        /// Bundle for a target architecture;
        /// defaults to the host arch
        #[arg(short, long, value_enum)]
        arch: Option<Arch>,
        /// Container runtime for multi-arch builds;
        /// only read if --arch is set
        #[arg(short, long, value_enum)]
        runtime: Option<ContainerRuntime>,
        /// Arguments to pass to cargo build
        #[arg(trailing_var_arg = true)]
        cargo_args: Vec<String>,
    },
    /// Build a project
    Build {
        /// Arguments to pass to cargo build
        #[arg(trailing_var_arg = true)]
        cargo_args: Vec<String>,
    },
    /// Run a binary or project
    Run {
        /// A binary to run; when unspecified, the current project will be run
        binary: Option<PathBuf>,
        /// Run detached from the current shell session
        #[arg(short, long, default_value_t = false)]
        background: bool,
        /// Arguments to pass to cargo run
        cargo_args: Vec<String>,
        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Kill a running maestro process
    Kill {
        /// The process to kill, by name or path
        target: PathBuf,
    },
    /// Update the libmaestro cache
    UpdateCache,
}

#[doc(hidden)]
fn help_style() -> Styles {
    Styles::default()
        .header(bold_with_colour(AnsiColor::BrightGreen))
        .usage(bold_with_colour(AnsiColor::BrightGreen))
        .literal(bold_with_colour(AnsiColor::BrightCyan))
        .placeholder(bold_with_colour(AnsiColor::BrightCyan))
        .error(bold_with_colour(AnsiColor::BrightRed))
        .invalid(bold_with_colour(AnsiColor::BrightRed))
}
#[doc(hidden)]
fn get_colour(colour: AnsiColor) -> Option<Color> {
    Some(Color::Ansi(colour))
}
#[doc(hidden)]
fn bold_with_colour(colour: AnsiColor) -> Style {
    Style::new().bold().fg_color(get_colour(colour))
}

/// Constructs a [`StringErr`] from an [`Error`] and a static [`&str`] description
fn mapper(e: &dyn Error, msg: &'static str) -> StringErr {
    Cow::Owned(format!("{msg}: {e}"))
}
/// Constructs a [`StringErr`] from a static [`&str`]
fn static_err(msg: &'static str) -> StringErr {
    Cow::Borrowed(msg)
}
/// Constructs a [`StringErr`] from a [`String`]
fn dynamic_err(msg: String) -> StringErr {
    Cow::Owned(msg)
}

/// Determines the crate root by traversing parent directories
fn find_crate_root() -> Result<PathBuf, StringErr> {
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

/// Converts failure of a [`std::process::Command`] to a [`StringErr`]
fn report_process_failure(status: ExitStatus, process: &'static str) -> StringErr {
    Cow::Owned(match status.code() {
        Some(code) => format!("{process} failed with exit code {code}"),
        None => format!("{process} failed due to external signal"),
    })
}

/// Trims trailing whitespace on all lines of a string
///
/// Particularly useful for string literals
fn dedent<S: ToString>(s: S) -> String {
    let str = s.to_string();
    str.trim()
        .lines()
        .map(|line| line.trim_start())
        .fold(String::new(), |mut acc, l| {
            acc.push_str(l);
            acc.push('\n');
            acc
        })
}

fn rustc_version() -> Result<String, StringErr> {
    let rustc_version_cmd = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|e| mapper(&e, "Failed to determine rustc version"))?;
    if !rustc_version_cmd.status.success() {
        return Err(report_process_failure(
            rustc_version_cmd.status,
            "Determining rustc version",
        ));
    }
    let rustc_output = String::from_utf8_lossy(&rustc_version_cmd.stdout);
    rustc_output
        .lines()
        .last()
        .ok_or(static_err("Failed to parse empty rustc output"))?
        .split_whitespace()
        .nth(1)
        .ok_or(static_err("Failed to parse rustc version output"))
        .map(|v| v.to_string())
}
