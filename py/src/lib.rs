use ::maestro::{self as RustMaestro};
use pyo3::{exceptions::PyValueError, prelude::*, types::PyCFunction};
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
    time::Duration,
};
use RustMaestro::{
    executors::{
        local::LocalExecutor as RustLocalExecutor,
        slurm::{
            MailTypeList, MemoryConfig, SlurmConfig as RustSlurmConfig,
            SlurmExecutor as RustSlurmExecutor, SlurmTime,
        },
        Executor,
    },
    process::StagingMode as RustStagingMode,
    Container as RustContainer, Process as RustProcess,
};

/// The maestro module
#[pymodule]
fn maestro(m: &Bound<'_, PyModule>) -> PyResult<()> {
    ::maestro::initialize();

    m.add_class::<Process>()?;
    m.add_class::<Container>()?;
    m.add_class::<StagingMode>()?;
    m.add_class::<LocalExecutor>()?;
    m.add_class::<SlurmExecutor>()?;
    m.add_class::<SlurmConfig>()?;

    Python::with_gil(|gil| -> PyResult<()> {
        let atexit = gil.import("atexit")?;
        let deinit = PyCFunction::new_closure(gil, None, None, |_args, _kwargs| {
            ::maestro::deinitialize();
        })?;
        atexit.call_method1("register", (deinit,))?;
        Ok(())
    })?;

    Ok(())
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

#[pyclass]
#[gen_stub_pyclass_enum]
#[derive(Clone)]
enum Container {
    Docker(String),
    Apptainer(String),
}
impl From<Container> for RustContainer {
    fn from(value: Container) -> Self {
        match value {
            Container::Docker(img) => RustContainer::Docker(Cow::Owned(img)),
            Container::Apptainer(img) => RustContainer::Apptainer(Cow::Owned(img)),
        }
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
        container: Option<Container>,
        inputs: HashMap<String, PathBuf>,
        outputs: HashMap<String, PathBuf>,
        args: HashMap<String, String>,
    ) -> PyResult<Process> {
        if script.trim().starts_with("#!").not() {
            Err(PyValueError::new_err("No shebang!"))
        } else {
            Ok(Process(RustProcess::new(
                name,
                container.map(|c| c.into()),
                inputs
                    .into_iter()
                    .map(|(name, path)| (Cow::Owned(name), path))
                    .collect(),
                args.into_iter()
                    .map(|(name, value)| (Cow::Owned(name), value))
                    .collect(),
                outputs
                    .into_iter()
                    .map(|(name, path)| (Cow::Owned(name), path))
                    .collect(),
                Cow::Owned(script.trim().to_string()),
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

#[pymethods]
#[gen_stub_pymethods]
impl LocalExecutor {
    #[new]
    fn __init__() -> LocalExecutor {
        LocalExecutor(RustLocalExecutor::default())
    }
    fn with_staging_mode(&mut self, mode: StagingMode) {
        self.0 = self.0.with_staging_mode(mode.into());
    }
    fn exe(&self, process: &Process) -> Result<Vec<PathBuf>, io::Error> {
        self.0.exe(process.0.clone())
    }
}

#[pyclass]
#[gen_stub_pyclass]
struct SlurmExecutor(RustSlurmExecutor);

#[pymethods]
#[gen_stub_pymethods]
impl SlurmExecutor {
    #[new]
    fn __init__() -> SlurmExecutor {
        SlurmExecutor(RustSlurmExecutor::default())
    }
    fn with_poll_rate(&mut self, rate: Duration) {
        self.0 = self.0.clone().with_poll_rate(rate);
    }
    fn with_staging_mode(&mut self, mode: StagingMode) {
        self.0 = self.0.clone().with_staging_mode(mode.into());
    }
    fn with_module(&mut self, module: String) {
        self.0 = self.0.clone().with_module(module);
    }
    fn with_modules(&mut self, modules: Vec<String>) {
        self.0 = self.0.clone().with_modules(modules);
    }
    fn with_config(&mut self, config: SlurmConfig) {
        self.0 = self.0.clone().with_config(config.into())
    }
    fn exe(&self, process: &Process) -> Result<Vec<PathBuf>, io::Error> {
        self.0.exe(process.0.clone())
    }
}

#[pyclass]
#[gen_stub_pyclass]
#[derive(Clone)]
struct SlurmConfig {
    cpus: Option<u64>,
    memory: Option<MemoryConfig>,
    gpus: Option<u64>,
    tasks: Option<u64>,
    nodes: Option<u64>,
    partition: Option<String>,
    time: Option<SlurmTime>,
    account: Option<String>,
    mail_user: Option<String>,
    mail_type: Option<MailTypeList>,
    additional_options: Vec<(String, String)>,
}
impl From<SlurmConfig> for RustSlurmConfig {
    fn from(value: SlurmConfig) -> Self {
        Self {
            cpus: value.cpus,
            memory: value.memory,
            gpus: value.gpus,
            tasks: value.tasks,
            nodes: value.nodes,
            partition: value.partition,
            time: value.time,
            account: value.account,
            mail_user: value.mail_user,
            mail_type: value.mail_type,
            additional_options: value.additional_options,
        }
    }
}
