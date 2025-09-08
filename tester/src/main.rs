use std::path::PathBuf;

use maestro::{
    executors::{Executor, LocalExecutor},
    workflow::CopyMode,
};
use maestro_macros::{inline_process, process};

fn main() {
    test_workflow();
    test_workflow_inline();
}

fn test_workflow() {
    let test_fasta = PathBuf::from("tester/data/seq1.fasta");
    let process = process!("tester/scripts/test.sh", test_fasta);

    let execution_result = LocalExecutor::default()
        .with_copy_mode(CopyMode::Symlink)
        .exe(process);

    execution_result.unwrap();
}

fn test_workflow_inline() {
    let test_str = "Hello, world!";
    let process = inline_process!(
        r#"
        #!/bin/bash
        echo "$test_str"
        "#,
        test_str
    );

    let execution_result = LocalExecutor::default()
        .with_copy_mode(CopyMode::Symlink)
        .exe(process);

    execution_result.unwrap();
}
