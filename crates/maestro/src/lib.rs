#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub mod executors;
mod macros;
mod session;
pub mod workflow;

pub struct Script<'a> {
    pub script: &'a str,
    pub vars: &'a mut [(&'a str, Injection)],
}
impl<'a> Script<'a> {
    fn display_vars(&self) -> Vec<(&'a str, String)> {
        self.vars
            .iter()
            .map(|(k, val)| match val {
                Injection::Param(s) => (*k, s.to_string()),
                Injection::File(p) => (*k, p.to_string_lossy().to_string()),
            })
            .collect()
    }
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
