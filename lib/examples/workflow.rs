use maestro::prelude::*;
use std::{
    io,
    path::{Path, PathBuf},
    sync::LazyLock,
};

fn main() {
    test_workflow(0).unwrap();
}

static EXECUTOR_GENERIC: LazyLock<GenericExecutor> = LazyLock::new(|| {
    let toml_str = r#"
        [Local]
        staging_mode = "Copy"
    "#;
    toml::from_str(toml_str).unwrap()
});
static EXECUTOR_SLURM_GENERIC: LazyLock<GenericExecutor> = LazyLock::new(|| {
    let toml_str = r#"
        [Slurm]
        staging_mode = "Copy"
        cpus = 8
        memory = { type = "PerNode", amount = 8192 }
        tasks = 1
        nodes = 1
        time = { days = 1 }
        account = "st-shallam-1"
        mail_user = "myemail@gmail.com"
        mail_type = ["All"]
        additional_options = [
            ["qos", "high"]
        ]
    "#;
    toml::from_str(toml_str).unwrap()
});

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
    EXECUTOR_SLURM_GENERIC.exe(process)
}
