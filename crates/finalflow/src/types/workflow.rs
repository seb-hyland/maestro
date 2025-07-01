use std::{
    fs::{OpenOptions, create_dir},
    io::Write,
    os::unix::fs::{OpenOptionsExt, symlink},
    path::PathBuf,
    process::Command,
};

use crate::session::{generate_hash, gwd};

pub struct Workflow {
    pub cmd: Script,
    pub outputs: Vec<PathBuf>,
}

pub struct Script {
    pub contents: &'static str,
    pub env: Vec<EnvVar>,
}

pub struct EnvVar(pub &'static str, pub EnvVarValue);

pub enum EnvVarValue {
    Param(String),
    File(PathBuf),
}
impl From<String> for EnvVarValue {
    fn from(s: String) -> Self {
        Self::Param(s)
    }
}
impl From<&str> for EnvVarValue {
    fn from(s: &str) -> Self {
        Self::Param(s.to_string())
    }
}
impl From<PathBuf> for EnvVarValue {
    fn from(p: PathBuf) -> Self {
        Self::File(p)
    }
}

pub type WorkflowResult = Result<Workflow, PathBuf>;
pub type ExecutionResult = Result<Vec<PathBuf>, ExecutionError>;

#[derive(Debug)]
pub enum ExecutionError {
    DirectoryError,
    WriteError,
    ProcessError,
    OutputsNotFound(Vec<PathBuf>),
}

impl Workflow {
    const SCRIPT_NAME: &str = "script.sh";

    pub fn new(cmd: Script, outputs: Vec<PathBuf>) -> WorkflowResult {
        Ok(Workflow { cmd, outputs })
    }

    fn prep_workdir(
        Workflow { cmd, outputs: _ }: &mut Workflow,
    ) -> Result<PathBuf, ExecutionError> {
        let hash = generate_hash();
        let workdir = gwd().map_err(|_| ExecutionError::DirectoryError)?;
        if !workdir.exists() {
            create_dir(&workdir).map_err(|_| ExecutionError::DirectoryError)?;
        }

        let mut hashed_workdir = workdir.join(hash);
        // For rare case where hashes are generated identically multiple times
        // Use bounded iterator; it is almost impossible for this to occur multiple times
        for _ in 0..2 {
            let hash = generate_hash();
            hashed_workdir = workdir.join(hash);
            if !hashed_workdir.exists() {
                break;
            }
        }
        create_dir(&hashed_workdir).map_err(|_| ExecutionError::DirectoryError)?;
        let mut script_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o755)
            .open(hashed_workdir.join(Self::SCRIPT_NAME))
            .map_err(|_| ExecutionError::WriteError)?;
        script_file
            .write_all(cmd.contents.as_bytes())
            .map_err(|_| ExecutionError::WriteError)?;

        cmd.env.iter_mut().try_for_each(|env_var| {
            if let EnvVar(_, EnvVarValue::File(input_path)) = env_var {
                let filename = input_path.file_name().ok_or(ExecutionError::WriteError)?;
                let sym_path = hashed_workdir.join(filename);
                symlink(&*input_path, &sym_path).map_err(|_| ExecutionError::WriteError)?;
                *input_path = sym_path;
            }
            Ok(())
        })?;
        Ok(hashed_workdir)
    }

    pub fn execute(mut self) -> ExecutionResult {
        let workdir = Self::prep_workdir(&mut self)?;
        let script = workdir.join(Self::SCRIPT_NAME);
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
            .map_err(|_| ExecutionError::ProcessError)?;
        if !cmd.status.success() {
            return Err(ExecutionError::ProcessError);
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
