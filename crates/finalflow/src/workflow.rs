use crate::{
    session::{generate_hash, generate_session_id, gwd},
    types::workflow::{EnvVar, EnvVarValue, ExecutionError, ExecutionResult, Script, Workflow},
};
use std::{
    fs::{self, OpenOptions, create_dir},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    panic,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

impl Workflow {
    const SCRIPT_NAME: &str = "script.sh";

    pub fn new(cmd: Script, outputs: Vec<PathBuf>) -> Workflow {
        Workflow { cmd, outputs }
    }

    pub(crate) fn prep_workdir(&mut self) -> Result<PathBuf, ExecutionError> {
        let Workflow { cmd, outputs: _ } = self;
        let workdir = gwd().map_err(|e| ExecutionError::DirectoryError(e.to_string()))?;
        if !workdir.exists() {
            create_dir(&workdir).map_err(|e| ExecutionError::DirectoryError(e.to_string()))?;
        }

        let mut session_workdir = workdir.join(generate_session_id());
        // For rare case where hashes are generated identically multiple times
        // Use bounded iterator; it is almost impossible for this to occur multiple times
        for _ in 0..2 {
            if !session_workdir.exists() {
                break;
            }
            let id = generate_session_id();
            session_workdir = workdir.join(id);
        }
        let input_dir = session_workdir.join(".finalflow_inputs");

        create_dir(&session_workdir).map_err(|e| ExecutionError::DirectoryError(e.to_string()))?;
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
        Ok(session_workdir)
    }

    pub(crate) fn start_process(&self, workdir: &PathBuf) -> Result<Child, ExecutionError> {
        let script = workdir.join(".finalflow_inputs").join(Self::SCRIPT_NAME);
        let Workflow { cmd, outputs: _ } = self;
        let vars: Vec<_> = cmd
            .env
            .iter()
            .map(|EnvVar(k, val)| match val {
                EnvVarValue::Param(s) => (k, s.clone()),
                EnvVarValue::File(p) => (k, p.to_string_lossy().to_string()),
            })
            .collect();
        Command::new(script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(vars)
            .current_dir(workdir)
            .spawn()
            .map_err(|e| ExecutionError::ProcessError(e.to_string()))
    }

    pub(crate) fn handle_process_output(&self, child: Child, workdir: PathBuf) -> ExecutionResult {
        let Workflow { cmd: _, outputs } = self;

        let cmd = child
            .wait_with_output()
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

    pub fn exe(mut self) -> ExecutionResult {
        let workdir = self.prep_workdir()?;
        let child = self.start_process(&workdir)?;
        self.handle_process_output(child, workdir)
    }

    pub(crate) fn exe_abortable(mut self, cancel_flag: Arc<AtomicBool>) -> ExecutionResult {
        let check_not_cancelled = |workdir: Option<&PathBuf>| -> Result<(), ExecutionError> {
            if cancel_flag.load(Ordering::Acquire) {
                if let Some(dir) = workdir {
                    let _ = fs::write(
                        dir.join(".finalflow_log"),
                        "Process cancelled due to other process's failure!",
                    );
                    Err(ExecutionError::Aborted)
                } else {
                    Err(ExecutionError::Aborted)
                }
            } else {
                Ok(())
            }
        };

        check_not_cancelled(None)?;
        let workdir = self.prep_workdir()?;

        check_not_cancelled(Some(&workdir))?;
        let mut child = self.start_process(&workdir)?;

        loop {
            check_not_cancelled(Some(&workdir))?;
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        check_not_cancelled(Some(&workdir))?;
        self.handle_process_output(child, workdir)
    }
}

pub trait WorkflowVecExt {
    fn par_exe(self) -> Vec<ExecutionResult>;
    fn par_exe_abort(self) -> Result<Vec<Vec<PathBuf>>, ExecutionError>;
}

impl WorkflowVecExt for Vec<Workflow> {
    fn par_exe(self) -> Vec<ExecutionResult> {
        self.into_iter()
            .map(|workflow| thread::spawn(|| workflow.exe()))
            .map(|handle| handle.join())
            .map(|joinresult| match joinresult {
                Err(e) => {
                    let process_error: Box<String> =
                        e.downcast().unwrap_or(Box::new(String::new()));
                    Err(ExecutionError::ProcessError(process_error.to_string()))
                }
                Ok(v) => v,
            })
            .collect()
    }

    fn par_exe_abort(self) -> Result<Vec<Vec<PathBuf>>, ExecutionError> {
        let (tx, rx) = mpsc::channel();
        let cancellation_flag = Arc::new(AtomicBool::new(false));
        let handles = self
            .into_iter()
            .enumerate()
            .map(|(i, workflow)| {
                let tx = tx.clone();
                let flag = cancellation_flag.clone();
                thread::spawn(move || {
                    // Catch if process panics
                    let result = panic::catch_unwind(|| {
                        let workflow_result = workflow.exe_abortable(flag);
                        (i, workflow_result)
                    })
                    .unwrap_or_else(|panic_payload| {
                        let err = ExecutionError::ProcessError(format!(
                            "Process {i} panicked with error {panic_payload:?}"
                        ));
                        (i, Err(err))
                    });
                    let _ = tx.send(result);
                })
            })
            .collect::<Vec<_>>();

        // Drop the original sender
        drop(tx);

        let mut results: Vec<Option<Vec<PathBuf>>> = vec![None; handles.len()];
        (0..handles.len()).try_for_each(|_| {
            let (i, result) = rx.recv().expect("Channel send should not fail!");
            match result {
                Err(e) => {
                    cancellation_flag.store(true, Ordering::Release);
                    return Err(e);
                }
                Ok(v) => results[i] = Some(v),
            };
            Ok(())
        })?;

        let unwrapped_results = results
            .into_iter()
            .map(|v| v.expect("All results should have been received!"))
            .collect::<Vec<_>>();

        handles.into_iter().for_each(|handle| {
            // Make sure all threads have cleaned up
            let _ = handle.join();
        });

        Ok(unwrapped_results)
    }
}
