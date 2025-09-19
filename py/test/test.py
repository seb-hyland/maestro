from maestro import LocalExecutor, Process

inputs = {
    "test_fasta": "../lib/examples/data/seq1.fasta",
    "test_dir": "../lib/examples/data/"
}
outputs = {
    "output_path": "out.txt"
}
p = Process(
    name = "my_process",
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
output_dir, *outputs = executor.exe(p)

print(outputs)
