use crate::{
    LP, Process,
    executors::Executor,
    process::{CheckTime, StagingMode},
};
use dagger::result::{NodeError, NodeResult};
use serde::Deserialize;
use std::{fmt::Display, io::Write as _, path::PathBuf, process::Command, thread, time::Duration};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlurmExecutor {
    #[serde(default = "default_poll_rate")]
    pub(crate) poll_rate: Duration,
    #[serde(default)]
    pub(crate) staging_mode: StagingMode,
    #[serde(default)]
    pub(crate) modules: Vec<String>,
    #[serde(flatten)]
    pub(crate) config: SlurmConfig,
}
const fn default_poll_rate() -> Duration {
    Duration::from_secs(5)
}

impl Default for SlurmExecutor {
    fn default() -> Self {
        Self {
            poll_rate: default_poll_rate(),
            staging_mode: StagingMode::Symlink,
            modules: Vec::new(),
            config: SlurmConfig::default(),
        }
    }
}

impl SlurmExecutor {
    pub fn with_poll_rate(mut self, rate: Duration) -> Self {
        self.poll_rate = rate;
        self
    }
    pub fn with_staging_mode(mut self, staging_mode: StagingMode) -> Self {
        self.staging_mode = staging_mode;
        self
    }
    pub fn with_module<S: ToString>(mut self, module: S) -> Self {
        self.modules.push(module.to_string());
        self
    }
    pub fn with_modules<S: ToString, M: IntoIterator<Item = S>>(mut self, modules: M) -> Self {
        let transformed_modules = modules.into_iter().map(|module| module.to_string());
        self.modules.extend(transformed_modules);
        self
    }
    pub fn with_config(mut self, config: SlurmConfig) -> Self {
        self.config = config;
        self
    }
    pub fn map_config<F>(mut self, f: F) -> Self
    where
        F: FnOnce(SlurmConfig) -> SlurmConfig,
    {
        self.config = (f)(self.config);
        self
    }
}

#[derive(Default, Clone, Deserialize)]
pub struct SlurmConfig {
    pub cpus: Option<u64>,
    pub memory: Option<MemoryConfig>,
    pub gpus: Option<u64>,
    pub tasks: Option<u64>,
    pub nodes: Option<u64>,
    pub partition: Option<String>,
    pub time: Option<SlurmTime>,
    pub account: Option<String>,
    pub mail_user: Option<String>,
    pub mail_type: Option<MailTypeList>,
    #[serde(default)]
    pub additional_options: Vec<(String, String)>,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SlurmTime {
    days: u16,
    hours: u16,
    mins: u8,
    secs: u8,
}
impl SlurmTime {
    pub fn new(days: u16, hours: u16, mins: u8, secs: u8) -> Option<Self> {
        if mins < 60 && secs < 60 {
            Some(Self {
                days,
                hours,
                mins,
                secs,
            })
        } else {
            None
        }
    }
    pub fn from_hours(hours: u16) -> Self {
        Self {
            days: 0,
            hours,
            mins: 0,
            secs: 0,
        }
    }
    pub fn from_days(days: u16) -> Self {
        Self {
            days,
            hours: 0,
            mins: 0,
            secs: 0,
        }
    }
}
impl Display for SlurmTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}-{:02}:{:02}:{:02}",
            self.days, self.hours, self.mins, self.secs
        )
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MailType {
    None,
    All,
    Begin,
    End,
    Fail,
    Requeue,
    InvalidDepend,
    StageOut,
    #[serde(rename = "TIME_LIMIT_50")]
    TimeLimit50,
    #[serde(rename = "TIME_LIMIT_80")]
    TimeLimit80,
    #[serde(rename = "TIME_LIMIT_90")]
    TimeLimit90,
    TimeLimit,
    ArrayTasks,
}
impl Display for MailType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let flag = match self {
            Self::None => "NONE",
            Self::All => "ALL",
            Self::Begin => "BEGIN",
            Self::End => "END",
            Self::Fail => "FAIL",
            Self::Requeue => "REQUEUE",
            Self::InvalidDepend => "INVALID_DEPEND",
            Self::StageOut => "STAGE_OUT",
            Self::TimeLimit50 => "TIME_LIMIT_50",
            Self::TimeLimit80 => "TIME_LIMIT_80",
            Self::TimeLimit90 => "TIME_LIMIT_90",
            Self::TimeLimit => "TIME_LIMIT",
            Self::ArrayTasks => "ARRAY_TASKS",
        };
        write!(f, "{flag}")
    }
}
#[derive(Clone, Deserialize)]
pub struct MailTypeList(Vec<MailType>);
impl Display for MailTypeList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0.iter();
        if let Some(first_flag) = iter.next() {
            write!(f, "{first_flag}")?;
        }
        for flag in iter {
            write!(f, ",{}", flag)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(tag = "type", content = "amount", rename_all = "snake_case")]
pub enum MemoryConfig {
    PerNode(Memory),
    PerCpu(Memory),
}
impl Display for MemoryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryConfig::PerCpu(Memory(v)) => write!(f, "{v}M"),
            MemoryConfig::PerNode(Memory(v)) => write!(f, "{v}M"),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
pub struct Memory(u64);
impl Memory {
    pub fn from_mb(memory: u64) -> Self {
        Self(memory)
    }
    pub fn from_gb(memory: u64) -> Self {
        Self(memory * 1024)
    }
}

macro_rules! impl_setter {
    ($field:ident, $fn_name:ident, $field_type:ty) => {
        pub fn $fn_name(mut self, $field: $field_type) -> Self {
            self.$field = Some($field);
            self
        }
    };
}
macro_rules! impl_string_setter {
    ($field:ident, $fn_name:ident) => {
        pub fn $fn_name<S: ToString>(mut self, $field: S) -> Self {
            self.$field = Some($field.to_string());
            self
        }
    };
}

impl SlurmConfig {
    impl_setter!(cpus, with_cpus, u64);
    impl_setter!(memory, with_memory, MemoryConfig);
    impl_setter!(gpus, with_gpus, u64);
    impl_setter!(tasks, with_tasks, u64);
    impl_setter!(nodes, with_nodes, u64);
    impl_string_setter!(partition, with_partition);
    impl_string_setter!(account, with_account);
    impl_string_setter!(mail_user, with_mail_user);

    pub fn with_time(mut self, time: SlurmTime) -> Self {
        self.time = Some(time);
        self
    }
    pub fn with_mail_types<M: AsRef<[MailType]>>(mut self, mail_types: M) -> Self {
        self.mail_type = Some(MailTypeList(mail_types.as_ref().to_vec()));
        self
    }
    pub fn with_arg<S: ToString>(mut self, arg: S, value: S) -> Self {
        self.additional_options
            .push((arg.to_string(), value.to_string()));
        self
    }
    pub fn with_args<S1, S2, A>(mut self, args: A) -> Self
    where
        S1: ToString,
        S2: ToString,
        A: AsRef<[(S1, S2)]>,
    {
        let transformed_args = args
            .as_ref()
            .iter()
            .map(|(arg, value)| (arg.to_string(), value.to_string()));
        self.additional_options.extend(transformed_args);
        self
    }
}

impl Display for SlurmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn write_arg<A: Display>(
            flag: &'static str,
            maybe_arg: &Option<A>,
            buffer: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            if let Some(arg) = maybe_arg {
                writeln!(buffer, "#SBATCH --{flag}={arg}")?;
            }
            Ok(())
        }
        write_arg("cpus-per-task", &self.cpus, f)?;
        write_arg("gpus", &self.gpus, f)?;

        let memory_flag = match self.memory {
            Some(MemoryConfig::PerCpu(_)) => "mem-per-cpu",
            Some(MemoryConfig::PerNode(_)) => "mem",
            None => "",
        };
        write_arg(memory_flag, &self.memory, f)?;
        write_arg("ntasks", &self.tasks, f)?;
        write_arg("nodes", &self.nodes, f)?;
        write_arg("partition", &self.partition, f)?;
        write_arg("time", &self.time, f)?;
        write_arg("account", &self.account, f)?;
        write_arg("mail-user", &self.mail_user, f)?;
        write_arg("mail-type", &self.mail_type, f)?;

        for (flag, arg) in &self.additional_options {
            writeln!(f, "#SBATCH --{flag}={arg}")?;
        }

        Ok(())
    }
}

impl Executor for SlurmExecutor {
    fn exe(&self, mut process: Process) -> NodeResult<Vec<PathBuf>> {
        let (workdir, (log_path, mut log_handle), (launcher_path, mut launcher_handle)) =
            process.prep_script_workdir()?;
        writeln!(launcher_handle, "{}", self.config)
            .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;

        let staging_mode = match process.container {
            None => &self.staging_mode,
            Some(_) => &StagingMode::Copy,
        };
        process.stage_inputs(&mut launcher_handle, &workdir, staging_mode)?;
        for module_name in &self.modules {
            writeln!(launcher_handle, "module load {module_name}")
                .map_err(|e| NodeError::msg(format!("Failed to write to launcher: {e}")))?;
        }
        Process::write_execution(launcher_handle, &process)?;

        let output = Command::new("sbatch")
            .args([
                "-o",
                ".maestro.log",
                "-e",
                ".maestro.err",
                "--open-mode=append",
            ])
            .arg(launcher_path)
            .current_dir(&workdir)
            .output()
            .map_err(|e| {
                NodeError::msg(format!("Failed to spawn sbatch for job submission: {e}"))
            })?;

        struct SlurmJobGuard<'a> {
            job_id: Option<&'a str>,
        }
        impl<'a> Drop for SlurmJobGuard<'a> {
            fn drop(&mut self) {
                if let Some(id) = &self.job_id {
                    let _ = Command::new("scancel").arg(id).status();
                }
            }
        }

        let job_id = if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let job_id = stdout
                .split_whitespace()
                .last()
                .and_then(|id| id.parse::<u32>().ok())
                .ok_or(NodeError::msg(format!(
                    "Failed to parse sbatch output into a job code: {stdout}"
                )));
            let _ = match job_id {
                Ok(id) => writeln!(log_handle, "{LP} Job submitted successfully! Id: {id}"),
                Err(_) => writeln!(
                    log_handle,
                    "{LP} Failed to parse sbatch output into a job id\nstdout: {}",
                    stdout
                ),
            };
            job_id?.to_string()
        } else {
            let error_code = output
                .status
                .code()
                .map(|code| format!("Error code: {}\n", code))
                .unwrap_or(String::new());
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = writeln!(
                log_handle,
                "{LP} Job failed to submit via sbatch!\n{error_code}stderr: {stderr}",
            );
            return Err(NodeError::msg(format!(
                "Job did not submit successfully. Logs at {}",
                log_path.display()
            )));
        };
        let mut job_guard = SlurmJobGuard {
            job_id: Some(&job_id),
        };

        let mut process_started = false;
        let mut process_started_msg = || {
            let _ = writeln!(log_handle, ":: Job execution started");
        };

        loop {
            let squeue_out = Command::new("squeue")
                .args(["-j", job_id.as_str(), "-h", "-o", "%T"])
                .output()
                .map_err(|e| {
                    NodeError::msg(format!("Failed to spawn squeue to monitor job status: {e}"))
                })?;

            if squeue_out.stdout.is_empty() {
                // Process finished
                break;
            } else if !process_started {
                let stdout = String::from_utf8_lossy(&squeue_out.stdout);
                // Process started
                if stdout.trim() != "PENDING" {
                    process_started = true;
                    process_started_msg();
                }
            }
            thread::sleep(self.poll_rate);
        }

        job_guard.job_id = None;
        // Process start was never read
        if !process_started {
            process_started_msg();
        }
        let job_info = Command::new("sacct")
            .args(["-j", job_id.as_str(), "-o", "JobID,JobName,ExitCode,Elapsed,Start,End,TotalCPU,AveCPU,MaxRSS,AveRSS,MaxVMSize,AveVMSize"])
            .output().map_err(|e| NodeError::msg(format!("Failed to spawn sacct to resolve job information: {e}")))?;
        let stdout = String::from_utf8_lossy(&job_info.stdout);
        let _ = writeln!(log_handle, "{LP} Job information\n{}", stdout);
        let job_status = stdout
            .lines()
            .nth(2)
            .and_then(|line| line.split_whitespace().nth(2))
            .and_then(|codes| codes.split_once(':'))
            .and_then(|(p1, p2)| {
                let code_1: u32 = p1.parse().ok()?;
                let code_2: u32 = p2.parse().ok()?;
                Some((code_1, code_2))
            });
        match job_status {
            Some((c1, c2)) => {
                if c1 == 0 && c2 == 0 {
                    let _ = writeln!(log_handle, "{LP} Job completed successfully!");
                } else {
                    let _ = writeln!(
                        log_handle,
                        "{LP} Job completed with non-zero exit code {c1}:{c2}\nstderr: .maestro.err"
                    );
                    return Err(NodeError::msg(format!(
                        "Job completed with non-zero exit code. Logs at {}",
                        log_path.display()
                    )));
                }
            }
            None => {
                let _ = writeln!(log_handle, ":: Failed to parse job status");
                return Err(NodeError::msg(format!(
                    "Failed to parse job status. Logs at {}",
                    log_path.display()
                )));
            }
        };

        process.check_files(CheckTime::Output, Some(&workdir))?;
        let mut outputs: Vec<_> = process
            .outputs
            .iter()
            .map(|(_, p)| workdir.join(p))
            .collect();
        outputs.push(workdir);
        Ok(outputs)
    }
}
