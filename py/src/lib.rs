use ::maestro::{self as RustMaestro, prelude::Executor};
use pyo3::{exceptions::PyValueError, prelude::*};
use pyo3_stub_gen::{
    define_stub_info_gatherer,
    derive::{gen_stub_pyclass, gen_stub_pyclass_enum, gen_stub_pymethods},
};
use std::{
    borrow::Cow,
    collections::HashMap,
    io,
    ops::{Deref, Not},
    path::PathBuf,
};
use RustMaestro::{
    prelude::LocalExecutor as RustLocalExecutor, Process as RustProcess,
    StagingMode as RustStagingMode,
};

/// The maestro module
#[pymodule]
mod maestro {
    use super::*;

    #[pymodule_export]
    use super::{LocalExecutor, Process, StagingMode};
}
define_stub_info_gatherer!(stub_info);

/// Process struct
#[pyclass]
#[gen_stub_pyclass]
#[derive(Clone)]
struct Process(RustProcess);

impl Deref for Process {
    type Target = RustProcess;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[pymethods]
#[gen_stub_pymethods]
impl Process {
    /// Constructor
    #[new]
    fn __init__(
        name: String,
        script: String,
        inputs: HashMap<String, PathBuf>,
        outputs: HashMap<String, PathBuf>,
        args: HashMap<String, String>,
    ) -> PyResult<Process> {
        if script.trim().starts_with("#!").not() {
            Err(PyValueError::new_err("No shebang!"))
        } else {
            Ok(Process(RustProcess::new(
                name,
                Cow::Owned(script.trim().to_string()),
                inputs
                    .into_iter()
                    .map(|(name, path)| (Cow::Owned(name), path))
                    .collect(),
                outputs
                    .into_iter()
                    .map(|(name, path)| (Cow::Owned(name), path))
                    .collect(),
                args.into_iter()
                    .map(|(name, value)| (Cow::Owned(name), value))
                    .collect(),
            )))
        }
    }
}

/// Staging mode
#[pyclass]
#[gen_stub_pyclass_enum]
#[derive(Clone, Copy)]
enum StagingMode {
    Copy,
    Symlink,
    None,
}
impl From<StagingMode> for RustStagingMode {
    fn from(value: StagingMode) -> Self {
        match value {
            StagingMode::Copy => Self::Copy,
            StagingMode::Symlink => Self::Symlink,
            StagingMode::None => Self::None,
        }
    }
}

#[pyclass]
#[gen_stub_pyclass]
struct LocalExecutor(RustLocalExecutor);

impl Deref for LocalExecutor {
    type Target = RustLocalExecutor;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[pymethods]
#[gen_stub_pymethods]
impl LocalExecutor {
    #[new]
    fn new() -> LocalExecutor {
        LocalExecutor(RustLocalExecutor::default())
    }
    fn with_staging_mode(&mut self, mode: StagingMode) {
        self.0 = self.0.with_staging_mode(mode.into());
    }
    fn with_error_handling(&mut self, y: bool) {
        self.0 = self.0.with_error_handling(y);
    }
    fn exe(&self, process: &Process) -> Result<Vec<PathBuf>, io::Error> {
        self.0.exe(process.0.clone())
    }
}
