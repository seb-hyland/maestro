use std::{io, path::PathBuf};

use maestro::{
    StagingMode,
    executors::{
        Executor,
        local::LocalExecutor,
        slurm::{Memory, MemoryConfig, SlurmExecutor, SlurmTime},
    },
};
use maestro_macros::process;

fn main() {
    test_workflow(13).unwrap();
    test_workflow(0).unwrap();
    test_workflow(5).unwrap();
    // test_workflow_inline().unwrap();
}

fn test_workflow(run: i32) -> io::Result<Vec<PathBuf>> {
    let test_fasta = PathBuf::from("tester/data/seq1.fasta");
    let test_dir = PathBuf::from("tester/data/");
    let output_path = String::from("out.txt");

    let process = process! {
        name = format!("test_{run}"),
        inputs = [
            test_fasta,
            test_dir
        ],
        outputs = [
            output_path
        ],
        inline = true,
        process = r#"
        cat $test_fasta
        cat $test_dir/seq2.fast
        tree $test_dir > $output_path
        "#
    };
    SlurmExecutor::default()
        .with_staging_mode(StagingMode::None)
        .with_module("gcc")
        .map_config(|config| {
            config
                .with_account("st-shallam-1")
                .with_nodes(1)
                .with_cpus(1)
                .with_memory(MemoryConfig::PerNode(Memory::from_gb(8)))
                .with_time(SlurmTime::from_hours(1))
        })
        .exe(process)
    // LocalExecutor::default()
    //     .with_error_handling(false)
    //     .with_staging_mode(StagingMode::Symlink)
    //     .exe(process)
}

// fn test_workflow_inline() -> io::Result<()> {
//     let test_str = "Hello, world!";
//     let [output_1, output_2] = paths!["echoed.txt", "copied.txt"];
//     let output_3 = "final.txt";

//     let process = inline_process!(
//         r#"#!/bin/bash
//         echo "$test_str" > $output_1
//         echo "$test_str" > $output_2
//         echo "$test_str" > $output_3
//         "#,
//         test_str,
//         output_1,
//         output_2,
//         output_3
//     );

//     let output_path = LocalExecutor::default().exe(process).unwrap();
//     let all_outputs = paths![output_1, output_2, output_3];
//     assert_exists!(all_outputs);
//     let outputs = output_path.join_outputs(all_outputs);

//     assert_exists!(outputs);
//     println!("{outputs:?}");

//     Ok(())
// }
