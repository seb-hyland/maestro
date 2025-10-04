from maestro import *

proc_inputs = {
    "test_fasta": "data/seq1.fasta",
    "test_dir": "data/",
}
proc_outputs = {
    "output_path": "out.txt"
}
proc = Process(
    name = "my_proc",
    inputs = proc_inputs,
    outputs = proc_outputs,
    args = {},
    script =
    """
    #!/bin/bash
    cat "$test_fasta"
    tree "$test_dir" > "$output_path"
    """
)

proc_executor = executor("default")
output_files = proc_executor.exe(proc)
print(output_files)
