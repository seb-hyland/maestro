use maestro::prelude::*;
use std::{
    io,
    path::{Path, PathBuf},
};

fn main() {
    // println!("{}", MAESTRO_CONFIG["print_statement"]);
    test_workflow(0, None).unwrap();
    test_workflow(1, Some("celiste")).unwrap();
}

fn test_workflow(run: i32, executor: Option<&'static str>) -> io::Result<Vec<PathBuf>> {
    let test_fasta = Path::new("lib/examples/data/seq1.fasta");
    let test_dir = Path::new("lib/examples/data/");
    let output_path = Path::new("out.txt");

    let process = process! {
        name = format!("test_{run}"),
        container = Docker("ubuntu:rolling"),
        inputs = [
            test_fasta,
            test_dir
        ],
        dependencies = ["!cat", "gromacs"],
        outputs = [
            output_path
        ],
        process = r#"
        sleep 5s
        cat "$test_fasta"
        cat "$test_dir"/seq2.fasta
        ls -R "$test_dir" > "$output_path"
        "#
    };
    match executor {
        None => MAESTRO_CONFIG.exe(process),
        Some(name) => MAESTRO_CONFIG.exe_custom(process, name),
    }
}
