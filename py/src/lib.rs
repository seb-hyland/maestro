use ::maestro::{self as RustMaestro};
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::PyCFunction,
};
use pyo3_stub_gen::{
    define_stub_info_gatherer,
    derive::{
        gen_stub_pyclass, gen_stub_pyclass_complex_enum, gen_stub_pyclass_enum, gen_stub_pymethods,
    },
};
use std::{borrow::Cow, collections::HashMap, path::PathBuf, time::Duration};
use RustMaestro::{
    executors::{
        local::LocalExecutor as RustLocalExecutor,
        slurm::{
            MailType as RustMailType, MailTypeList as RustMailTypeList, Memory as RustMemory,
            MemoryConfig as RustMemoryConfig, SlurmConfig as RustSlurmConfig,
            SlurmExecutor as RustSlurmExecutor, SlurmTime as RustSlurmTime,
        },
        Executor,
    },
    prelude::NodeError as RustNodeError,
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
    m.add_class::<MemoryConfig>()?;
    m.add_class::<Memory>()?;
    m.add_class::<MailType>()?;

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

#[pyclass]
#[gen_stub_pyclass_complex_enum]
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
        inputs: HashMap<String, PathBuf>,
        outputs: HashMap<String, PathBuf>,
        args: HashMap<String, String>,
    ) -> PyResult<Process> {
        if !script.trim().starts_with("#!") {
            Err(PyValueError::new_err("No shebang!"))
        } else {
            Ok(Process(RustProcess::new(
                name,
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

struct NodeError(RustNodeError);
impl From<NodeError> for PyErr {
    fn from(value: NodeError) -> Self {
        PyRuntimeError::new_err(format!("{:?}", value.0))
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
    fn with_container(&mut self, container: Container) {
        self.0 = self.0.clone().with_container(container.into());
    }
    fn with_staging_mode(&mut self, mode: StagingMode) {
        self.0 = self.0.clone().with_staging_mode(mode.into());
    }
    fn exe(&self, process: &Process) -> Result<Vec<PathBuf>, NodeError> {
        self.0.exe(process.0.clone()).map_err(NodeError)
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
    fn with_container(&mut self, container: Container) {
        self.0 = self.0.clone().with_container(container.into());
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
    fn exe(&self, process: &Process) -> Result<Vec<PathBuf>, NodeError> {
        self.0.exe(process.0.clone()).map_err(NodeError)
    }
}

#[pyclass]
#[gen_stub_pyclass]
#[derive(Clone, Default)]
struct SlurmConfig {
    cpus: Option<u64>,
    memory: Option<MemoryConfig>,
    gpus: Option<u64>,
    tasks: Option<u64>,
    nodes: Option<u64>,
    partition: Option<String>,
    time: Option<Duration>,
    account: Option<String>,
    mail_user: Option<String>,
    mail_type: Option<Vec<MailType>>,
    additional_options: Vec<(String, String)>,
}

#[pymethods]
#[gen_stub_pymethods]
impl SlurmConfig {
    #[new]
    fn __init__() -> Self {
        Self::default()
    }
    fn with_cpus(&mut self, cpus: u64) {
        self.cpus = Some(cpus)
    }
    fn with_memory(&mut self, memory: MemoryConfig) {
        self.memory = Some(memory)
    }
    fn with_gpus(&mut self, gpus: u64) {
        self.gpus = Some(gpus)
    }
    fn with_tasks(&mut self, tasks: u64) {
        self.tasks = Some(tasks)
    }
    fn with_nodes(&mut self, nodes: u64) {
        self.nodes = Some(nodes);
    }
    fn with_partition(&mut self, partition: String) {
        self.partition = Some(partition);
    }
    fn with_time(&mut self, time: Duration) {
        self.time = Some(time);
    }
    fn with_account(&mut self, account: String) {
        self.account = Some(account);
    }
    fn with_mail_user(&mut self, mail_user: String) {
        self.mail_user = Some(mail_user);
    }
    fn with_mail_type(&mut self, mail_type: Vec<MailType>) {
        self.mail_type = Some(mail_type);
    }
    fn with_additional_options(&mut self, additional_options: Vec<(String, String)>) {
        self.additional_options = additional_options;
    }
}

impl From<SlurmConfig> for RustSlurmConfig {
    fn from(value: SlurmConfig) -> Self {
        Self {
            cpus: value.cpus,
            memory: value.memory.map(|memory| memory.into()),
            gpus: value.gpus,
            tasks: value.tasks,
            nodes: value.nodes,
            partition: value.partition,
            time: value.time.map(|time| {
                let total_secs = time.as_secs();
                let days = total_secs / 86_400;
                let hours = (total_secs % 86_400) / 3_600;
                let mins = (total_secs % 3_600) / 60;
                let secs = total_secs % 60;
                RustSlurmTime::new(days as u16, hours as u16, mins as u8, secs as u8)
                    .expect("Conversion should not be fallible")
            }),
            account: value.account,
            mail_user: value.mail_user,
            mail_type: value.mail_type.map(|types| {
                let mail_types = types.into_iter().map(|ty| ty.into()).collect();
                RustMailTypeList(mail_types)
            }),
            additional_options: value.additional_options,
        }
    }
}

#[pyclass]
#[gen_stub_pyclass_complex_enum]
#[derive(Clone, Copy)]
enum MemoryConfig {
    PerNode(Memory),
    PerCpu(Memory),
}
#[pyclass]
#[gen_stub_pyclass_complex_enum]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
enum Memory {
    gb(u64),
    mb(u64),
}
impl From<MemoryConfig> for RustMemoryConfig {
    fn from(value: MemoryConfig) -> Self {
        match value {
            MemoryConfig::PerCpu(quant) => RustMemoryConfig::PerCpu(match quant {
                Memory::gb(gb) => RustMemory::from_gb(gb),
                Memory::mb(mb) => RustMemory::from_mb(mb),
            }),
            MemoryConfig::PerNode(quant) => RustMemoryConfig::PerNode(match quant {
                Memory::gb(gb) => RustMemory::from_gb(gb),
                Memory::mb(mb) => RustMemory::from_mb(mb),
            }),
        }
    }
}

#[pyclass]
#[gen_stub_pyclass_enum]
#[derive(Clone, Copy)]
enum MailType {
    None,
    All,
    Begin,
    End,
    Fail,
    Requeue,
    InvalidDepend,
    StageOut,
    TimeLimit50,
    TimeLimit80,
    TimeLimit90,
    TimeLimit,
    ArrayTasks,
}
impl From<MailType> for RustMailType {
    fn from(value: MailType) -> Self {
        match value {
            MailType::None => Self::None,
            MailType::All => Self::All,
            MailType::Begin => Self::Begin,
            MailType::End => Self::End,
            MailType::Fail => Self::Fail,
            MailType::Requeue => Self::Requeue,
            MailType::InvalidDepend => Self::InvalidDepend,
            MailType::StageOut => Self::StageOut,
            MailType::TimeLimit50 => Self::TimeLimit50,
            MailType::TimeLimit80 => Self::TimeLimit80,
            MailType::TimeLimit90 => Self::TimeLimit90,
            MailType::TimeLimit => Self::TimeLimit,
            MailType::ArrayTasks => Self::ArrayTasks,
        }
    }
}
