use std::path::{Path, PathBuf};

use maestro::{
    OutputMapper,
    executors::{Executor, LocalExecutor},
    paths,
    workflow::StagingMode,
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
        .with_copy_mode(StagingMode::Symlink)
        .exe(process);

    execution_result.unwrap();
}

fn test_workflow_inline() {
    let test_str = "Hello, world!";
    let [output_1, output_2] = paths!["echoed.txt", "copied.txt"];
    let output_3 = "final.txt";

    let process = inline_process!(
        r#"
        #!/bin/bash
        echo "$test_str" > $output_1
        echo "$test_str" > $output_2
        echo "$test_str" > $output_3
        "#,
        test_str,
        output_1,
        output_2,
        output_3
    );

    let output_path = LocalExecutor::default().exe(process).unwrap();
    let outputs = output_path.join_outputs(paths![output_1, output_2, output_3]);
    println!("{outputs:?}");
}
