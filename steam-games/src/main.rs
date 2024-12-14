use chrono::DateTime;
use eyre::Context;
use eyre::OptionExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct LibraryFolder {
    path: String,
    label: Option<String>,
    #[serde(rename = "contentid")]
    content_id: Option<i64>,
    #[serde(rename = "totalsize")]
    total_size: Option<u64>,
    update_clean_bytes_tally: Option<u64>,
    time_last_update_verified: Option<u64>,
    apps: Option<HashMap<u64, u64>>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct AppState {
    #[serde(rename = "appid")]
    pub app_id: u64,
    #[serde(rename = "Universe")]
    pub universe: Option<u64>,
    #[serde(rename = "LauncherPath")]
    pub launcher_path: Option<PathBuf>,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "StateFlags")]
    pub state_flags: Option<i64>,
    #[serde(rename = "installdir")]
    pub install_dir: Option<String>,
    #[serde(rename = "LastUpdated")]
    pub last_updated: Option<u64>,
    #[serde(rename = "LastPlayed")]
    pub last_played: Option<i64>,
    #[serde(rename = "SizeOnDisk")]
    pub size_on_disk: Option<u64>,
    #[serde(rename = "StagingSize")]
    pub staging_size: Option<u64>,
    #[serde(rename = "buildid")]
    pub build_id: Option<u64>,
    #[serde(rename = "LastOwner")]
    pub last_owner: Option<u64>,
    #[serde(rename = "AutoUpdateBehavior")]
    pub auto_update_behavior: Option<u64>,
    #[serde(rename = "AllowOtherDownloadsWhileRunning")]
    pub allow_other_downloads_while_running: Option<u32>,
    #[serde(rename = "ScheduledAutoUpdate")]
    pub scheduled_auto_update: Option<u32>,
    #[serde(rename = "InstalledDepots")]
    pub installed_depots: Option<HashMap<u32, Depot>>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct Depot {
    pub manifest: u64,
    pub size: u64,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    // Update this path to the correct location of your `libraryfolders.vdf`
    let library_folders_path = r"C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf";

    // Parse the libraryfolders.vdf file
    let library_content =
        fs::read_to_string(library_folders_path).context("Failed to read libraryfolders.vdf")?;

    let library_folders: HashMap<usize, LibraryFolder> =
        keyvalues_serde::from_str(&library_content)
            .context("Failed to parse libraryfolders.vdf")?;

    // Find installed games and their last played times
    for library_folder in library_folders.values() {
        let library_path = Path::new(&library_folder.path);
        let steamapps_path = Path::new(&library_path).join("steamapps");
        if !steamapps_path.exists() {
            println!("Library path not found: {}", steamapps_path.display());
            continue;
        }
        println!("Scanning library at: {}", steamapps_path.display());
        let dir_contents =
            fs::read_dir(steamapps_path).context("Failed to read steamapps directory")?;
        for entry in dir_contents {
            let entry = entry.context("Failed to get directory entry")?;
            let path = entry.path();
            let Some(file_name) = path.file_name() else {
                continue;
            };
            let file_name = file_name.to_string_lossy();
            if !(file_name.starts_with("appmanifest_") && file_name.ends_with(".acf")) {
                continue;
            }
            let appmanifest_content =
                fs::read_to_string(&path).expect("Failed to read appmanifest file");
            let manifest: AppState = keyvalues_serde::from_str(&appmanifest_content)
                .context("Failed to parse appmanifest file")?;
            let last_played = match manifest.last_played {
                Some(last_played) => format!(
                    "{}",
                    DateTime::from_timestamp(last_played, 0)
                        .ok_or_eyre("Failed to parse last played time")?
                ),
                None => "Never".to_string(),
            };
            println!(" - {} (Last Played: {})", manifest.name, last_played);
        }
    }
    Ok(())
}
