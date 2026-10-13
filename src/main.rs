mod api_key;
mod app_paths;
mod settings;
mod soniox;
mod transcript;

#[cfg(target_os = "macos")]
mod app_window;
#[cfg(target_os = "macos")]
mod desktop_ui;
#[cfg(target_os = "macos")]
mod live;
#[cfg(target_os = "macos")]
mod login_item;
#[cfg(target_os = "macos")]
mod sparkle;
#[cfg(target_os = "macos")]
mod window_level;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;

/// Display name used in menus and window titles. Keep in sync with
/// `scripts/app-config.sh`; a unit test enforces it.
pub const APP_NAME: &str = "Szept";
/// Bundle identifier. It also names the data directory.
pub const BUNDLE_ID: &str = "com.patryksztuczka.szept";

#[derive(Parser)]
#[command(
    name = "szept",
    version,
    about = "Live English captions with Polish translation"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// A top-level pane of the main window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Pane {
    Live,
    Settings,
}

#[derive(Subcommand)]
enum Command {
    /// Open the desktop app. This is the default when no command is given.
    App {
        /// The pane to show first.
        #[arg(long, value_enum, default_value_t = Pane::Live)]
        pane: Pane,
    },
    /// Print the data, settings, API key, and log locations.
    Paths,
    /// Print the saved settings as JSON.
    Settings,
    /// Print live audio levels, to check capture works without an API key.
    Meter {
        #[arg(long, value_enum, default_value_t = settings::AudioSource::Meeting)]
        source: settings::AudioSource,
        #[arg(long, default_value_t = 10)]
        seconds: u64,
    },
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let command = Cli::parse()
        .command
        .unwrap_or(Command::App { pane: Pane::Live });
    match command {
        Command::App { pane } => {
            app_paths::init_logging()?;
            run_app(pane)
        }
        Command::Paths => {
            println!("data:     {}", app_paths::support_dir()?.display());
            println!("settings: {}", app_paths::settings_path()?.display());
            println!("api key:  {}", api_key::path()?.display());
            println!("logs:     {}", app_paths::logs_dir()?.display());
            Ok(())
        }
        Command::Settings => {
            let settings = settings::Settings::load()?;
            println!("{}", serde_json::to_string_pretty(&settings)?);
            Ok(())
        }
        Command::Meter { source, seconds } => run_meter(source, seconds),
    }
}

#[cfg(target_os = "macos")]
fn run_app(pane: Pane) -> Result<()> {
    app_window::run(pane);
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_meter(source: settings::AudioSource, seconds: u64) -> Result<()> {
    live::meter(source, seconds).map_err(|error| color_eyre::eyre::eyre!(error))
}

#[cfg(not(target_os = "macos"))]
fn run_meter(_source: settings::AudioSource, _seconds: u64) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "audio capture currently targets macOS only"
    ))
}

#[cfg(not(target_os = "macos"))]
fn run_app(_pane: Pane) -> Result<()> {
    Err(color_eyre::eyre::eyre!(
        "the desktop app currently targets macOS only"
    ))
}

#[cfg(test)]
mod tests {
    /// The packaging scripts and the binary must agree on the app identity.
    #[test]
    fn identity_matches_packaging_config() {
        let config = include_str!("../scripts/app-config.sh");
        let value = |key: &str| {
            config
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{key}=")))
                .unwrap_or_else(|| panic!("{key} is missing from scripts/app-config.sh"))
                .to_owned()
        };
        assert_eq!(value("APP_NAME"), super::APP_NAME);
        assert_eq!(value("BUNDLE_ID"), super::BUNDLE_ID);
        assert_eq!(value("EXECUTABLE"), env!("CARGO_BIN_NAME"));
    }
}
