use tracing_subscriber::EnvFilter;

pub fn init(default_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("wgent={default_level}")));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
