use maestro::prelude::*;
use std::{
    io,
    path::{Path, PathBuf},
};

fn main() {
    println!("{}", MAESTRO_CONFIG["print_statement"]);
    test_workflow(0).unwrap();
}

fn test_workflow(run: i32) -> io::Result<Vec<PathBuf>> {
    let test_fasta = Path::new("lib/examples/data/seq1.fasta");
    let test_dir = Path::new("lib/examples/data/");
    let output_path = Path::new("out.txt");

    let process = process! {
        name = format!("test_{run}"),
        container = Container::from_docker("ubuntu:rolling"),
        inputs = [
            test_fasta,
            test_dir
        ],
        dependencies = ["!cat", "gromacs"],
        outputs = [
            output_path
        ],
        inline = true,
        process = r#"
        cat "$test_fasta"
        cat "$test_dir"/seq2.fasta
        ls -R "$test_dir" > "$output_path"
        "#
    };
    MAESTRO_CONFIG.execute(process)
}
