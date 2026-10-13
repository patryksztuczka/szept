use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;

use color_eyre::eyre::{Result, eyre};
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// `~/Library/Application Support/<bundle id>`, or `SZEPT_SUPPORT_DIR` when
/// set. The override keeps tests and previews away from real user data.
pub fn support_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("SZEPT_SUPPORT_DIR") {
        return Ok(path.into());
    }
    Ok(dirs::data_dir()
        .ok_or_else(|| eyre!("application data directory is unavailable"))?
        .join(crate::BUNDLE_ID))
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(support_dir()?.join("settings.json"))
}

pub fn logs_dir() -> Result<PathBuf> {
    Ok(support_dir()?.join("logs"))
}

/// Logs to stderr and appends to `logs/process.log`. Filter with `RUST_LOG`.
pub fn init_logging() -> Result<()> {
    let logs = logs_dir()?;
    fs::create_dir_all(&logs)?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("process.log"))?;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("szept=info,warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr.and(Mutex::new(file)))
        .init();
    Ok(())
}
