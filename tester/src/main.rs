use std::path::PathBuf;

use maestro::{Injection, Script, workflow::CopyMode};

fn main() {
    echo();
}

fn echo() {
    let script = Script {
        script: include_str!("../scripts/test.sh"),
        vars: &mut [(
            "TESTFASTA",
            Injection::File(PathBuf::from("tester/data/seq1.fasta")),
        )],
    };
    script.execute_local(CopyMode::Copy).unwrap();
}
