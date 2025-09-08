use std::path::PathBuf;

use maestro::{
    Injection, Script,
    executors::{Executor, LocalExecutor},
    workflow::CopyMode,
};

fn main() {
    echo();
}

fn echo() {
    let script = Script {
        script: include_str!("../scripts/test.sh"),
        vars: &mut [
            (
                "TESTFASTA",
                Injection::File(PathBuf::from("tester/data/seq1.fasta")),
            ),
            ("TESTDIR", Injection::File(PathBuf::from("tester/data/"))),
        ],
    };
    let executor = LocalExecutor::default().with_copy_mode(CopyMode::Symlink);
    let result = executor.exe(script);
    if let Err(e) = result {
        dbg!(&e);
    }
}
