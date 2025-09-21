use maestro::prelude::*;

fn main() {
    test_workflow(0).unwrap();
    println!("{}", arg!("print_statement"));
}

fn test_workflow(run: i32) -> io::Result<Vec<PathBuf>> {
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
        cat "$test_fasta"
        cat "$test_dir"/seq2.fasta
        ls -R "$test_dir" > "$output_path"
        "#
    };
    execute!(process)
}
