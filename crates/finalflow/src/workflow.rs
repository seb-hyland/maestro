use crate::{
    session::{generate_hash, gwd},
    types::workflow::{EnvVar, EnvVarValue, ExecutionError, ExecutionResult, Script, Workflow},
};
use std::{
    fs::{self, OpenOptions, create_dir},
    io::Write,
    os::unix::fs::{OpenOptionsExt, symlink},
    path::PathBuf,
    process::Command,
};

impl Workflow {
    const SCRIPT_NAME: &str = "script.sh";

    pub fn new(cmd: Script, outputs: Vec<PathBuf>) -> Workflow {
        Workflow { cmd, outputs }
    }

    fn prep_workdir(
        Workflow { cmd, outputs: _ }: &mut Workflow,
    ) -> Result<PathBuf, ExecutionError> {
        let workdir = gwd().map_err(|e| ExecutionError::DirectoryError(e.to_string()))?;
        if !workdir.exists() {
            create_dir(&workdir).map_err(|e| ExecutionError::DirectoryError(e.to_string()))?;
        }

        let mut hashed_workdir = workdir.join(generate_hash());
        // For rare case where hashes are generated identically multiple times
        // Use bounded iterator; it is almost impossible for this to occur multiple times
        for _ in 0..2 {
            if !hashed_workdir.exists() {
                break;
            }
            let hash = generate_hash();
            hashed_workdir = workdir.join(hash);
        }
        let input_dir = hashed_workdir.join("inputs");
        create_dir(&hashed_workdir).map_err(|e| ExecutionError::DirectoryError(e.to_string()))?;
        create_dir(&input_dir).map_err(|e| ExecutionError::DirectoryError(e.to_string()))?;
        let mut script_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(input_dir.join(Self::SCRIPT_NAME))
            .map_err(|e| ExecutionError::WriteError(e.to_string()))?;
        script_file
            .write_all(cmd.contents.as_bytes())
            .map_err(|e| ExecutionError::WriteError(e.to_string()))?;

        cmd.env.iter_mut().try_for_each(|env_var| {
            if let EnvVar(_, EnvVarValue::File(input_path)) = env_var {
                let filename =
                    input_path
                        .file_name()
                        .ok_or(ExecutionError::WriteError(format!(
                            "Failed to obtain name of file {:?}",
                            input_path
                        )))?;
                let sym_path = input_dir.join(filename);
                let canonical_path = input_path
                    .canonicalize()
                    .map_err(|e| ExecutionError::WriteError(e.to_string()))?;
                if !canonical_path.exists() {
                    return Err(ExecutionError::WriteError(
                        "Canonicalized input path {:?} does not exist!".to_string(),
                    ));
                }
                fs::copy(canonical_path, &sym_path)
                    .map_err(|e| ExecutionError::WriteError(e.to_string()))?;
                *input_path = sym_path;
            }
            Ok(())
        })?;
        Ok(hashed_workdir)
    }

    pub fn execute(mut self) -> ExecutionResult {
        let workdir = Self::prep_workdir(&mut self)?;
        let script = workdir.join("inputs").join(Self::SCRIPT_NAME);
        let Workflow { cmd, outputs } = self;
        let vars: Vec<_> = cmd
            .env
            .into_iter()
            .map(|EnvVar(k, val)| match val {
                EnvVarValue::Param(s) => (k, s),
                EnvVarValue::File(p) => (k, p.to_string_lossy().to_string()),
            })
            .collect();
        let cmd = Command::new(script)
            .envs(vars)
            .current_dir(&workdir)
            .output()
            .map_err(|e| ExecutionError::ProcessError(e.to_string()))?;
        if !cmd.status.success() {
            return Err(ExecutionError::ProcessError(format!(
                "Stdout: {}\nStderr: {}",
                String::from_utf8_lossy(&cmd.stdout),
                String::from_utf8_lossy(&cmd.stderr)
            )));
        }

        let output_checks: Vec<PathBuf> = outputs
            .iter()
            .map(|p| workdir.join(p))
            .filter(|p| !p.exists())
            .collect();
        match output_checks.is_empty() {
            true => {
                let absolute_outputs = outputs.iter().map(|p| workdir.join(p)).collect();
                Ok(absolute_outputs)
            }
            false => Err(ExecutionError::OutputsNotFound(output_checks)),
        }
    }
}
