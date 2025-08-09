use crate::config::get_cache_dir;
use crate::config::set_cache_dir;
use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use clap::Subcommand;
use clap::ValueEnum;
use color_eyre::eyre;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum, Arbitrary)]
pub enum ConfigKey {
    #[clap(name = "cache-dir")]
    CacheDir,
}

impl ConfigKey {
    fn as_str(&self) -> &'static str {
        match self {
            ConfigKey::CacheDir => "cache-dir",
        }
    }
}

#[derive(Args, Arbitrary, PartialEq, Debug, Clone)]
pub struct ConfigArgs {
    #[clap(subcommand)]
    pub action: ConfigAction,
}

impl ConfigArgs {
    pub fn run(self) -> eyre::Result<()> {
        self.action.run()
    }
}

impl ToArgs for ConfigArgs {
    fn to_args(&self) -> Vec<OsString> {
        self.action.to_args()
    }
}

#[derive(Subcommand, Arbitrary, PartialEq, Debug, Clone)]
pub enum ConfigAction {
    /// Show all config values (human-readable)
    Show,
    /// Get a config value by name
    Get {
        /// Name of the config value to get
        key: ConfigKey,
    },
    /// Set a config value by name
    Set {
        /// Name of the config value to set
        key: ConfigKey,
        /// Value to set (defaults to current directory)
        #[clap(default_value = ".")]
        value: PathBuf,
    },
}

impl ConfigAction {
    pub fn run(self) -> eyre::Result<()> {
        match self {
            ConfigAction::Show => show_all(),
            ConfigAction::Get { key } => get_one(key),
            ConfigAction::Set { key, value } => set_one(key, value),
        }
    }
}

impl ToArgs for ConfigAction {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        match self {
            ConfigAction::Show => {
                args.push("show".into());
            }
            ConfigAction::Get { key } => {
                args.push("get".into());
                args.push(key.as_str().into());
            }
            ConfigAction::Set { key, value } => {
                args.push("set".into());
                args.push(key.as_str().into());
                args.push(value.as_os_str().to_os_string());
            }
        }
        args
    }
}

fn show_all() -> eyre::Result<()> {
    use owo_colors::OwoColorize;

    // cache-dir
    match get_cache_dir() {
        Ok(p) => {
            println!(
                "{} {} {}",
                "cache-dir".bright_blue().bold(),
                "=".dimmed(),
                p.display().to_string().bright_green()
            );
        }
        Err(_) => {
            println!(
                "{} {} {}",
                "cache-dir".bright_blue().bold(),
                "=".dimmed(),
                "<unset>".yellow()
            );
        }
    }

    Ok(())
}

fn get_one(key: ConfigKey) -> eyre::Result<()> {
    match key {
        ConfigKey::CacheDir => {
            let p = get_cache_dir()?;
            println!("{}", p.display());
            Ok(())
        }
    }
}

fn set_one(key: ConfigKey, value: PathBuf) -> eyre::Result<()> {
    match key {
        ConfigKey::CacheDir => set_cache_dir(&value),
    }
}
