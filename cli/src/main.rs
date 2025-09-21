use crate::{
    build::{BuildType, build_project},
    bundle::bundle_project,
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
        Cmd::Bundle { cargo_args } => bundle_project(cargo_args),
        Cmd::UpgradeCache => prep_cache().map(|_| {}),
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
enum Cmd {
    Init {
        path: Option<String>,
    },
    Bundle {
        #[arg(trailing_var_arg = true)]
        cargo_args: Vec<String>,
    },
    UpgradeCache,
    Build {
        #[arg(trailing_var_arg = true)]
        cargo_args: Vec<String>,
    },
    Run {
        binary: Option<PathBuf>,

        #[arg(short, long, default_value_t = false)]
        background: bool,

        cargo_args: Vec<String>,

        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Kill {
        target: PathBuf,
    },
}

fn help_style() -> Styles {
    Styles::default()
        .header(bold_with_colour(AnsiColor::BrightGreen))
        .usage(bold_with_colour(AnsiColor::BrightGreen))
        .literal(bold_with_colour(AnsiColor::BrightCyan))
        .placeholder(bold_with_colour(AnsiColor::BrightCyan))
        .error(bold_with_colour(AnsiColor::BrightRed))
        .invalid(bold_with_colour(AnsiColor::BrightRed))
}
fn get_colour(colour: AnsiColor) -> Option<Color> {
    Some(Color::Ansi(colour))
}
fn bold_with_colour(colour: AnsiColor) -> Style {
    Style::new().bold().fg_color(get_colour(colour))
}

fn mapper(e: &dyn Error, msg: &'static str) -> StringErr {
    Cow::Owned(format!("{msg}: {e}"))
}
fn static_err(msg: &'static str) -> StringErr {
    Cow::Borrowed(msg)
}
fn dynamic_err(msg: String) -> StringErr {
    Cow::Owned(msg)
}

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

fn report_process_failure(status: ExitStatus, process: &'static str) -> StringErr {
    Cow::Owned(match status.code() {
        Some(code) => format!("{process} failed with exit code {code}"),
        None => format!("{process} failed due to external signal"),
    })
}

fn dedent<S: ToString>(s: S) -> String {
    let str = s.to_string();
    str.trim()
        .lines()
        .map(|line| line.trim_start())
        .fold(String::new(), |mut acc, l| {
            acc.push('\n');
            acc.push_str(l);
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
