use crate::config::Config;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let cfg = Config::load();
        let filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new(&cfg.log_level))
            .unwrap_or_else(|_| EnvFilter::new("info"));

        let layer = fmt::layer().with_target(false).compact();
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(layer)
            .try_init();
    });
}
