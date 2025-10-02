use crate::{
    Container,
    executors::{
        GenericExecutor,
        local::LocalExecutor,
        slurm::{SlurmConfig, SlurmExecutor},
    },
    process::StagingMode,
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    process::exit,
    sync::LazyLock,
    time::Duration,
};

#[derive(Deserialize)]
#[serde(untagged)]
pub enum MaybeInheritingExecutor {
    Inherit {
        inherit: String,
        #[serde(flatten)]
        overrides: Box<Option<PartialExecutor>>,
    },
    Executor(GenericExecutor),
}

#[derive(Clone, Deserialize)]
pub struct PartialExecutor {
    // Either
    container: Option<Container>,
    staging_mode: Option<StagingMode>,
    // Slurm
    poll_rate: Option<Duration>,
    modules: Option<Vec<String>>,
    #[serde(flatten)]
    config: SlurmConfig,
}

impl GenericExecutor {
    pub(crate) fn merge(self, other: Option<PartialExecutor>) -> Option<GenericExecutor> {
        if let Some(other_overrides) = other {
            match self {
                Self::Local(local_exe) => {
                    local_exe.merge(other_overrides).map(GenericExecutor::Local)
                }
                Self::Slurm(slurm_exe) => slurm_exe
                    .merge(other_overrides)
                    .map(|exec| GenericExecutor::Slurm(Box::new(exec))),
            }
        } else {
            Some(self)
        }
    }
}

impl LocalExecutor {
    pub(crate) fn merge(self, other: PartialExecutor) -> Option<LocalExecutor> {
        if other.poll_rate.is_some() || other.modules.is_some() {
            return None;
        }
        let slurm_config = other.config;
        if slurm_config.cpus.is_some()
            || slurm_config.memory.is_some()
            || slurm_config.gpus.is_some()
            || slurm_config.tasks.is_some()
            || slurm_config.nodes.is_some()
            || slurm_config.partition.is_some()
            || slurm_config.time.is_some()
            || slurm_config.account.is_some()
            || slurm_config.mail_user.is_some()
            || slurm_config.mail_type.is_some()
            || !slurm_config.additional_options.is_empty()
        {
            None
        } else {
            Some(LocalExecutor {
                staging_mode: other.staging_mode.unwrap_or(self.staging_mode),
                container: other.container.or(self.container),
            })
        }
    }
}
impl SlurmExecutor {
    pub(crate) fn merge(mut self, other: PartialExecutor) -> Option<SlurmExecutor> {
        Some(SlurmExecutor {
            container: other.container.or(self.container),
            poll_rate: other.poll_rate.unwrap_or(self.poll_rate),
            staging_mode: other.staging_mode.unwrap_or(self.staging_mode),
            modules: {
                let mut other_modules = other.modules.unwrap_or_default();
                other_modules.append(&mut self.modules);
                other_modules
            },
            config: {
                SlurmConfig {
                    cpus: other.config.cpus.or(self.config.cpus),
                    memory: other.config.memory.or(self.config.memory),
                    gpus: other.config.gpus.or(self.config.gpus),
                    tasks: other.config.tasks.or(self.config.tasks),
                    nodes: other.config.nodes.or(self.config.nodes),
                    partition: other.config.partition.or(self.config.partition),
                    time: other.config.time.or(self.config.time),
                    account: other.config.account.or(self.config.account),
                    mail_user: other.config.mail_user.or(self.config.mail_user),
                    mail_type: other.config.mail_type.or(self.config.mail_type),
                    additional_options: {
                        let mut other_options = other.config.additional_options;
                        other_options.append(&mut self.config.additional_options);
                        other_options
                    },
                }
            },
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlConfig {
    pub executor: HashMap<String, MaybeInheritingExecutor>,
    #[serde(default)]
    pub args: HashMap<String, String>,
    #[serde(default)]
    pub inputs: HashMap<String, Vec<String>>,
}

pub struct MaestroConfig {
    pub executors: HashMap<String, GenericExecutor>,
    pub args: HashMap<String, String>,
    pub inputs: HashMap<String, Vec<String>>,
}

pub static MAESTRO_CONFIG: LazyLock<MaestroConfig> = LazyLock::new(|| {
    let config_file = env::var("MAESTRO_CONFIG").unwrap_or("Maestro.toml".to_string());
    let file_contents = fs::read_to_string(config_file).unwrap_or_else(|e| {
        eprintln!("Failed to read config file: {e}");
        exit(1)
    });
    let config: TomlConfig = toml::from_str(&file_contents).unwrap_or_else(|e| {
        eprintln!("Failed to parse Maestro.toml: {e}");
        exit(1)
    });

    let canonicalized_executors = config
        .executor
        .iter()
        .map(|(name, exec)| {
            let executor = match exec {
                MaybeInheritingExecutor::Executor(exec) => exec.clone(),
                MaybeInheritingExecutor::Inherit { .. } => {
                    let mut overrides_vec = Vec::new();
                    let mut seen_map = HashSet::new();
                    seen_map.insert(name.as_str());

                    fn recurse_executors<'a>(
                        current: &'a MaybeInheritingExecutor,
                        overrides_vec: &mut Vec<(&'a Option<PartialExecutor>, &'a str)>,
                        seen_map: &mut HashSet<&'a str>,
                        executors: &'a HashMap<String, MaybeInheritingExecutor>,
                    ) -> &'a MaybeInheritingExecutor {
                        match current {
                            MaybeInheritingExecutor::Inherit { inherit, overrides } => {
                                let inner_executor = match executors.get(inherit) {
                                    Some(v) => v,
                                    None => {
                                        eprintln!("Unable to resolve inherited executor {inherit}");
                                        exit(1)
                                    }
                                };
                                overrides_vec.push((overrides, inherit));
                                if !seen_map.insert(inherit) {
                                    eprintln!("Circular dependence on executor {inherit}");
                                    exit(1)
                                }
                                recurse_executors(inner_executor, overrides_vec, seen_map, executors)
                            }
                            MaybeInheritingExecutor::Executor(_) => current,
                        }
                    }

                    let final_executor =
                        recurse_executors(exec, &mut overrides_vec, &mut seen_map, &config.executor);
                    match final_executor {
                        MaybeInheritingExecutor::Executor(exe) => {
                            let mut composite_executor = exe.clone();
                            for (executor, other_name) in overrides_vec.into_iter().rev() {
                                composite_executor = match composite_executor.merge(executor.clone()) {
                                    Some(v) => v,
                                    None => {
                                        eprintln!("Attempted to inherit from an executor of a different type: {name} from {other_name}");
                                        exit(1)
                                    }
                                }
                            }
                            composite_executor.clone()
                        }
                        _ => unreachable!(),
                    }
                }
            };
            (name.clone(), executor)
        })
        .collect();

    MaestroConfig {
        executors: canonicalized_executors,
        inputs: config.inputs,
        args: config.args,
    }
});
