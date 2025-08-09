use std::path::PathBuf;
use tracing::info;


/// Show a single MFT file using the unified multi-file TUI (wrapped as a single-item Vec)
pub fn show_mft_file(
    mft_file: PathBuf,
    _verbose: bool,
    _show_paths: bool,
    _max_entries: Option<usize>,
) -> eyre::Result<()> {
    let app = crate::tui::app::MftShowApp::new(vec![mft_file]);
    app.run()
}

/// Expand glob patterns and analyze (one or many) MFT files with the unified TUI
pub fn show_mft_files(
    pattern: &str,
    _verbose: bool,
    _show_paths: bool,
    _max_entries: Option<usize>,
    _threads: Option<usize>,
) -> eyre::Result<()> {
    let mft_files = expand_glob_pattern(pattern)?;
    info!(
        "Found {} MFT files matching pattern '{}'",
        mft_files.len(),
        pattern
    );
    if mft_files.is_empty() {
        return Err(eyre::eyre!("At least one MFT file is required to proceed"));
    }
    let app = crate::tui::app::MftShowApp::new(mft_files);
    app.run()
}

/// Expand glob pattern to find MFT files
fn expand_glob_pattern(pattern: &str) -> eyre::Result<Vec<PathBuf>> {
    use glob::glob;

    let mut files = Vec::new();

    if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
        for entry in glob(pattern).map_err(|e| eyre::eyre!("Invalid glob pattern: {}", e))? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        files.push(path);
                    }
                }
                Err(e) => {
                    tracing::warn!("Error accessing path in glob: {}", e);
                }
            }
        }
    } else {
        let path = PathBuf::from(pattern);
        if path.is_file() {
            files.push(path);
        } else if !path.exists() {
            return Err(eyre::eyre!("File not found: {}", pattern));
        } else {
            return Err(eyre::eyre!("Path is not a file: {}", pattern));
        }
    }

    files.sort();

    Ok(files)
}
