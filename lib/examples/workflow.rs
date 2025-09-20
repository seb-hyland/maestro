use maestro::prelude::*;
use std::{
    io,
    path::{Path, PathBuf},
    sync::LazyLock,
};

fn main() {
    test_workflow(0).unwrap();
}

static EXECUTOR: LazyLock<GenericExecutor> = LazyLock::new(|| {
    GenericExecutor::Local(LocalExecutor::default().with_staging_mode(StagingMode::Symlink))
});
static EXECUTOR_SLURM: LazyLock<GenericExecutor> = LazyLock::new(|| {
    GenericExecutor::Slurm(Box::new(
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
            }),
    ))
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
    EXECUTOR.exe(process)
}
