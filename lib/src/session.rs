use crate::LP;
use session_gen::generate_session_id;
use std::{
    env,
    fs::{self},
    io,
    path::PathBuf,
    process,
    sync::OnceLock,
};

pub(crate) static SESSION_WORKDIR: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn setup_session_workdir() -> Result<PathBuf, io::Error> {
    let session_id = env::var("MAESTRO_SESSION_ID").unwrap_or(generate_session_id());
    let maestro_workdir = match env::var("MAESTRO_WORKDIR") {
        Ok(v) => PathBuf::from(v),
        Err(_) => env::current_dir()?.join("maestro_work"),
    };
    let session_workdir = maestro_workdir.join(&session_id);
    fs::create_dir_all(&session_workdir)?;

    // Check if there are any existing directories in the session working directory
    if fs::read_dir(&session_workdir)?.any(|item| {
        if let Ok(entry) = item
            && let Ok(ty) = entry.file_type()
            && ty.is_dir()
        {
            true
        } else {
            false
        }
    }) {
        return Err(io::Error::new(
            io::ErrorKind::DirectoryNotEmpty,
            format!("Directory {} is not empty!", session_workdir.display()),
        ));
    }

    fs::write(
        session_workdir.join(".maestro.active"),
        process::id().to_string(),
    )?;
    println!(
        "{LP} New maestro session initialized!\n{LP} ID: {}\n{LP} Workdir: {}",
        session_id,
        session_workdir.display()
    );
    Ok(session_workdir)
}
