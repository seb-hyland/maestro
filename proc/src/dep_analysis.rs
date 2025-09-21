use fxhash::{FxBuildHasher, FxHashSet};
use serde::Serialize;
use std::{
    env,
    fs::File,
    path::Path,
    sync::{LazyLock, Mutex},
};

pub(crate) static DEPENDENCIES_FILE: LazyLock<Mutex<File>> = LazyLock::new(|| {
    Mutex::new(
        File::create(Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("dependencies.toml"))
            .expect("Failed to open dependencies file for writing!"),
    )
});

#[derive(Serialize)]
pub(crate) struct ProcessDependencies {
    pub(crate) executor: String,
    #[serde(flatten)]
    pub(crate) container: Option<ContainerDependency>,
    pub(crate) deps: Vec<String>,
}
#[derive(Serialize)]
#[serde(tag = "engine")]
pub(crate) enum ContainerDependency {
    Docker { image: String },
    Apptainer { image: String },
}

pub(crate) fn analyze_depends(
    script: &str,
    excluded: &mut FxHashSet<String>,
    all_dependencies: &mut FxHashSet<String>,
) {
    let mut push_if_not_excluded = |depend: String| {
        if !excluded.contains(&depend) {
            all_dependencies.insert(depend);
        }
    };
    let is_valid = |input: &str| -> bool {
        input
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
    };
    for line in script.lines() {
        let mut tokens = line
            .split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}')
            .filter(|t| !t.is_empty())
            .peekable();
        if let Some(first) = tokens.next()
            && is_valid(first)
        {
            push_if_not_excluded(first.to_string());
        }
        while let Some(token) = tokens.next()
            && let Some(next_token) = tokens.peek()
        {
            if ((token == "|" || token == "&&" || token == "||") || token.ends_with(';'))
                && is_valid(next_token)
            {
                push_if_not_excluded(next_token.to_string());
            }
        }
    }
}

pub(crate) static SHELL_EXCLUDES: LazyLock<FxHashSet<String>> = LazyLock::new(|| {
    let mut set = FxHashSet::with_capacity_and_hasher(
        SHELL_KEYWORDS.len() + SHELL_BUILTINS.len(),
        FxBuildHasher::default(),
    );
    for exclude in SHELL_KEYWORDS.into_iter().chain(SHELL_BUILTINS) {
        set.insert(exclude.to_string());
    }
    set
});
const SHELL_KEYWORDS: [&str; 17] = [
    "case", "coproc", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in",
    "select", "then", "until", "while", "time",
];
const SHELL_BUILTINS: [&str; 57] = [
    "alias",
    "bg",
    "bind",
    "break",
    "builtin",
    "caller",
    "cd",
    "command",
    "compgen",
    "complete",
    "compopt",
    "continue",
    "declare",
    "typeset",
    "dirs",
    "disown",
    "echo",
    "enable",
    "eval",
    "exec",
    "exit",
    "export",
    "false",
    "fc",
    "fg",
    "getopts",
    "hash",
    "help",
    "history",
    "jobs",
    "kill",
    "let",
    "local",
    "logout",
    "mapfile",
    "readarray",
    "popd",
    "printf",
    "pushd",
    "pwd",
    "read",
    "readonly",
    "return",
    "set",
    "shift",
    "shopt",
    "suspend",
    "test",
    "times",
    "trap",
    "true",
    "type",
    "ulimit",
    "umask",
    "unalias",
    "unset",
    "wait",
];
