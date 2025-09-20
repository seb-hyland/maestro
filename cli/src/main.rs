use std::{borrow::Cow, error::Error, process};

use crate::init::initialize;
use clap::Parser;

mod init;

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

fn main() {
    let command = Command::parse();
    if let Err(e) = match command {
        Command::Init { path } => initialize(path),
    } {
        eprintln!("{e}");
        process::exit(1);
    }
}

#[derive(Parser)]
#[command(version, about)]
enum Command {
    Init { path: Option<String> },
}
