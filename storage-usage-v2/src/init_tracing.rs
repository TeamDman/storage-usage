use tracing::Level;
use tracing::debug;

/// Initialize tracing subscriber with the given log level.
/// In debug builds, include file and line number without timestamp.
/// In release builds, include timestamp and log level.
pub fn init_tracing(level: Level) {
    let builder = tracing_subscriber::fmt().with_max_level(level);
    #[cfg(debug_assertions)]
    let subscriber = builder
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .finish();
    #[cfg(not(debug_assertions))]
    let subscriber = builder.finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
    debug!("Tracing initialized with level: {:?}", level);
}
