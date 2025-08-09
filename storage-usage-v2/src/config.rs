use color_eyre::eyre::Context;
use color_eyre::eyre::{self};
use directories_next::ProjectDirs;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::RwLock;

static CACHE_DIR_CACHE: LazyLock<RwLock<Option<PathBuf>>> = LazyLock::new(|| {
    let initial = read_initial_cache_dir().ok().flatten();
    RwLock::new(initial)
});

fn project_config_dir() -> eyre::Result<PathBuf> {
    ProjectDirs::from("com", "TeamDman", "storage-usage-v2")
        .ok_or_else(|| eyre::eyre!("No valid config directory for this platform"))
        .map(|p| p.config_dir().to_path_buf())
}

fn cache_dir_file_path() -> eyre::Result<PathBuf> {
    Ok(project_config_dir()?.join("cache-dir.txt"))
}

fn read_env_cache_dir() -> eyre::Result<Option<PathBuf>> {
    match std::env::var("MFT_CACHE_DIR") {
        Ok(val) => {
            let p = Path::new(val.trim());
            if p.as_os_str().is_empty() {
                return Ok(None);
            }
            let canon =
                fs::canonicalize(p).with_context(|| format!("canonicalizing {}", p.display()))?;
            Ok(Some(canon))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(eyre::eyre!("reading MFT_CACHE_DIR env var: {}", e)),
    }
}

fn read_cache_dir_file() -> eyre::Result<Option<PathBuf>> {
    let path = cache_dir_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let p = Path::new(trimmed);
    let canon = fs::canonicalize(p).with_context(|| format!("canonicalizing {}", p.display()))?;
    Ok(Some(canon))
}

fn read_initial_cache_dir() -> eyre::Result<Option<PathBuf>> {
    if let Some(p) = read_env_cache_dir()? {
        return Ok(Some(p));
    }
    read_cache_dir_file()
}

pub fn get_cache_dir() -> eyre::Result<PathBuf> {
    if let Some(cached) = CACHE_DIR_CACHE.read().unwrap().clone() {
        return Ok(cached);
    }
    let value = read_initial_cache_dir()?;
    match value {
        Some(p) => {
            *CACHE_DIR_CACHE.write().unwrap() = Some(p.clone());
            Ok(p)
        }
        None => Err(eyre::eyre!(
            "cache-dir is not configured. Use: storage-usage-v2.exe config set cache-dir ."
        )),
    }
}

pub fn set_cache_dir(cache_dir: &Path) -> eyre::Result<()> {
    let canon = fs::canonicalize(cache_dir)
        .with_context(|| format!("canonicalizing {}", cache_dir.display()))?;

    let cfg_dir = project_config_dir()?;
    fs::create_dir_all(&cfg_dir).with_context(|| format!("creating {}", cfg_dir.display()))?;

    let file = cfg_dir.join("cache-dir.txt");
    fs::write(&file, canon.to_string_lossy().as_bytes())
        .with_context(|| format!("writing {}", file.display()))?;

    // Update cache
    *CACHE_DIR_CACHE.write().unwrap() = Some(canon);

    Ok(())
}
