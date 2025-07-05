use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;

/// Converts a Rust `&str` to a null-terminated wide string (`Vec<u16>`).
pub fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(once(0)) // Append null terminator
        .collect()
}
