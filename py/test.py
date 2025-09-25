from maestro import LocalExecutor, Process, SlurmExecutor, StagingMode

inputs = {
    "test_fasta": "../lib/examples/data/seq1.fasta",
    "test_dir": "../lib/examples/data/"
}
outputs = {
    "output_path": "out.txt"
}
p = Process(
    name = "my_process",
    container = None,
    script =
    """
    #!/bin/bash
    cat "$test_fasta"
    cat "$test_dir"/seq2.fasta
    tree "$test_dir" > "$output_path"
    """,
    inputs = inputs,
    outputs = outputs,
    args = {}
)

executor = LocalExecutor()
executor.with_staging_mode(StagingMode.Copy)
*outputs, output_dir = executor.exe(p)

print(outputs)
