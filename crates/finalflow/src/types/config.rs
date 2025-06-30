use crate::session;

pub enum StagingMode {
    Symlink,
    Copy,
}

pub struct Config {
    id: String,
    staging: StagingMode,
}

impl Config {
    fn default() -> Config {
        Config {
            id: session::generate_session_id(),
            staging: StagingMode::Symlink,
        }
    }
}
