use std::{
    fmt::Display,
    io::{self, Write as _},
    path::PathBuf,
    process::Command,
    thread,
    time::Duration,
};

use crate::{Script, StagingMode, executors::Executor};

pub struct SlurmExecutor {
    poll_rate: Duration,
    staging_mode: StagingMode,
    config: SlurmConfig,
}

impl Default for SlurmExecutor {
    fn default() -> Self {
        Self {
            poll_rate: Duration::from_secs(5),
            staging_mode: StagingMode::Symlink,
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

#[derive(Default, Clone)]
pub struct SlurmConfig {
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

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub enum MailType {
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
#[derive(Clone)]
struct MailTypeList(Vec<MailType>);
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

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
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
        let transformed_args: Vec<_> = args
            .as_ref()
            .iter()
            .map(|(arg, value)| (arg.to_string(), value.to_string()))
            .collect();
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
    fn exe(self, mut script: Script) -> io::Result<PathBuf> {
        let (workdir, (log_path, mut log_handle), (launcher_path, mut launcher_handle)) =
            script.prep_script_workdir()?;
        writeln!(launcher_handle, "{}", self.config)?;
        script.stage_inputs(&mut launcher_handle, &workdir, &self.staging_mode)?;
        writeln!(
            launcher_handle,
            "./.maestro.sh > .maestro.out 2> .maestro.err"
        )?;

        let output = Command::new("sbatch")
            .arg(launcher_path)
            .current_dir(&workdir)
            .output()?;

        let job_id = if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let job_id = stdout
                .split_whitespace()
                .last()
                .and_then(|id| id.parse::<u32>().ok())
                .ok_or(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Failed to parse sbatch output into a job code: {stdout}"),
                ));
            match job_id {
                Ok(id) => writeln!(log_handle, ":: Job submitted successfully! Id: {id}"),
                Err(_) => writeln!(
                    log_handle,
                    ":: Failed to parse sbatch output into a job id\nstdout: {}",
                    stdout
                ),
            }?;
            job_id?.to_string()
        } else {
            let error_code = output
                .status
                .code()
                .map(|code| format!("Error code: {}\n", code))
                .unwrap_or(String::new());
            let stderr = String::from_utf8_lossy(&output.stderr);
            writeln!(
                log_handle,
                ":: Job failed to submit via sbatch!\n{error_code}stderr: {stderr}",
            )?;
            return Err(io::Error::other(format!(
                "Job did not submit successfully. Logs at {}",
                log_path.display()
            )));
        };

        let mut process_started = false;
        let mut process_started_msg =
            || -> io::Result<()> { writeln!(log_handle, ":: Job execution started") };

        loop {
            let squeue_out = Command::new("squeue")
                .args(["-j", job_id.as_str(), "-h", "-o", "%T"])
                .output()?;

            if squeue_out.stdout.is_empty() {
                // Process finished
                break;
            } else if !process_started {
                let stdout = String::from_utf8_lossy(&squeue_out.stdout);
                // Process started
                if stdout.trim() != "PENDING" {
                    process_started = true;
                    process_started_msg()?;
                }
            }
            thread::sleep(self.poll_rate);
        }

        // Process start was never read
        if !process_started {
            process_started_msg()?;
        }
        let job_info = Command::new("sacct")
            .args(["-j", job_id.as_str(), "-o", "JobID,JobName,ExitCode,Elapsed,Start,End,TotalCPU,AveCPU,MaxRSS,AveRSS,MaxVMSize,AveVMSize"])
            .output()?;
        let stdout = String::from_utf8_lossy(&job_info.stdout);
        writeln!(log_handle, ":: Job information\n{}", stdout)?;
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
                    writeln!(log_handle, ":: Job completed successfully!")?;
                    Ok(workdir)
                } else {
                    writeln!(
                        log_handle,
                        ":: Job completed with non-zero exit code {c1}:{c2}\nstderr: .maestro.err"
                    )?;
                    Err(io::Error::other(format!(
                        "Job completed with non-zero exit code. Logs at {}",
                        log_path.display()
                    )))
                }
            }
            None => {
                writeln!(log_handle, ":: Failed to parse job status")?;
                Err(io::Error::other(format!(
                    "Failed to parse job status. Logs at {}",
                    log_path.display()
                )))
            }
        }
    }
}
