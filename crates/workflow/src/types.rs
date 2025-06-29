use std::path::PathBuf;

pub struct Workflow {
    pub cmd: Script,
    pub outputs: Vec<PathBuf>,
}

pub struct Script {
    pub contents: &'static str,
    pub runtime: Runtime,
}

pub enum Runtime {
    Local,
    Container(Container),
}

pub enum Container {
    Apptainer(&'static str),
}
