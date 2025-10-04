use maestro::prelude::*;

#[maestro::main]
fn main() {
    test_workflow(0).unwrap();
}

fn test_workflow(run: i32) -> WorkflowResult {
    let test_fasta = Path::new("lib/examples/data/seq1.fasta");
    let test_dir = Path::new("lib/examples/data/");
    let output_path = Path::new("out.txt");
    process! {
        /// This is a docstring that describes this process
        /// Maybe I talk more about what it does
        /// ...so the user knows how they should configure its resources
        name = format!("test_{run}"),
        executor = "slurm",
        inputs = [
            test_fasta,
            test_dir
        ],
        outputs = [
            // output_path
        ],
        dependencies = ["!cat", "gromacs"],
        inline = false,
        script = "lib/examples/scripts/test.sh"
    }
}

// fn test_workflow_1(run: i32) -> io::Result<Vec<PathBuf>> {
//     let test_fasta = Path::new("lib/examples/data/seq1.fasta");
//     let test_dir = Path::new("lib/examples/data/");
//     let output_path = Path::new("out.txt");

//     process! {
//         "This is a docstring"
//         name = format!("test_{run}"),
//         executor = "chae",
//         inputs = [
//             test_fasta,
//             test_dir
//         ],
//         dependencies = ["tree", "gromacs", "mybin"],
//         outputs = [
//             output_path
//         ],
//         process = r#"
//         cat "$test_fasta"
//         cat "$test_dir"/seq2.fasta
//         ls -R "$test_dir" > "$output_path"
//         "#
//     }
// }
