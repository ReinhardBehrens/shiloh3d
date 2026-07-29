//! Tracing-based logging bootstrap (pure Rust).

/// Initializes a default `tracing` subscriber if none is set.
///
/// No-op when the `logging` feature is disabled.
pub fn init() {
    #[cfg(feature = "logging")]
    {
        use tracing_subscriber::EnvFilter;

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));

        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .try_init();
    }
}

/// Convenience re-export of the `tracing` macros' crate for dependents.
pub use tracing;
