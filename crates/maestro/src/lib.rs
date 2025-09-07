#![allow(dead_code)]

use std::path::{Path, PathBuf};

mod macros;
mod session;
pub mod workflow;

pub struct Script<'a> {
    pub script: &'a str,
    pub vars: &'a mut [(&'static str, Injection)],
}

pub enum Injection {
    Param(String),
    File(PathBuf),
}
impl From<String> for Injection {
    fn from(s: String) -> Self {
        Self::Param(s)
    }
}
impl<'a> From<&'a str> for Injection {
    fn from(s: &'a str) -> Self {
        Self::Param(s.to_string())
    }
}
impl From<PathBuf> for Injection {
    fn from(p: PathBuf) -> Self {
        Self::File(p)
    }
}
impl<'a> From<&'a Path> for Injection {
    fn from(p: &'a Path) -> Self {
        Self::File(p.to_path_buf())
    }
}
