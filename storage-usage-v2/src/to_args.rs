use std::ffi::OsString;

/// Trait for converting CLI structures to command line arguments
pub trait ToArgs {
    fn to_args(&self) -> Vec<OsString> {
        Vec::new()
    }
}

/// Unit struct representing the current invocation's arguments
#[derive(Debug, Clone)]
pub struct ThisInvocation;

impl ToArgs for ThisInvocation {
    fn to_args(&self) -> Vec<OsString> {
        std::env::args_os().skip(1).collect()
    }
}
