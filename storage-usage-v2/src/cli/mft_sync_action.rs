use crate::config::get_cache_dir;
use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Args;
use color_eyre::eyre;
use std::ffi::OsString;
use std::fs;
use super::drive_letter_pattern::DriveLetterPattern;

/// Arguments for syncing MFT files into the cache directory
#[derive(Args, Clone, PartialEq, Debug)]
pub struct MftSyncArgs {
    /// Drive letter pattern to match drives to sync (e.g., "*", "C", "CD", "C,D")
    #[clap(default_value_t = DriveLetterPattern::default())]
    pub drive_pattern: DriveLetterPattern,

    /// Overwrite existing cached MFT files
    #[clap(long)]
    pub overwrite_existing: bool,
}

impl<'a> Arbitrary<'a> for MftSyncArgs {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let drive_letter = {
            let idx = u8::arbitrary(u)? % 26;
            let c = (b'A' + idx) as char;
            DriveLetterPattern(c.to_string())
        };
        let overwrite_existing = bool::arbitrary(u)?;
        Ok(Self { drive_pattern: drive_letter, overwrite_existing })
    }
}

impl MftSyncArgs {
    pub fn run(self) -> eyre::Result<()> {
        let drives = self.drive_pattern.resolve()?;
        let cache = get_cache_dir()?;
        fs::create_dir_all(&cache)?;
        for d in drives {
            let out = cache.join(format!("{}.mft", d));
            crate::mft_dump::dump_mft_to_file(&out, self.overwrite_existing, d)?;
        }
        Ok(())
    }
}

impl ToArgs for MftSyncArgs {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.push(self.drive_pattern.to_string().into());
        if self.overwrite_existing {
            args.push("--overwrite-existing".into());
        }
        args
    }
}
