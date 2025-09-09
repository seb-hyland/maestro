use std::{io, path::PathBuf};

use maestro::{
    OutputMapper, assert_exists,
    executors::{Executor, local::LocalExecutor},
    paths,
    workflow::StagingMode,
};
use maestro_macros::{inline_process, process};

fn main() {
    test_workflow().unwrap();
    test_workflow_inline().unwrap();
}

fn test_workflow() -> io::Result<PathBuf> {
    let test_fasta = PathBuf::from("tester/data/seq1.fasta");
    assert_exists!(test_fasta);

    let process = process!("tester/scripts/test.sh", test_fasta);
    LocalExecutor::default()
        .with_copy_mode(StagingMode::Symlink)
        .exe(process)
}

fn test_workflow_inline() -> io::Result<()> {
    let test_str = "Hello, world!";
    let [output_1, output_2] = paths!["echoed.txt", "copied.txt"];
    let output_3 = "final.txt";

    let process = inline_process!(
        r#"#!/bin/bash
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
    let all_outputs = paths![output_1, output_2, output_3];
    assert_exists!(all_outputs);
    let outputs = output_path.join_outputs(all_outputs);

    assert_exists!(outputs);
    println!("{outputs:?}");

    Ok(())
}
