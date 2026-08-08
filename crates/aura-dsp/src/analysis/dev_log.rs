//! Dev-only file logging — enable with feature `dev-logging` (no-op otherwise).
//!
//! Call [`init`] once from plugin/editor setup. Logs go to
//! `%TEMP%/clap-dev.log`, panics to `%TEMP%/clap-dev-panic.log`.
//! Synchronous writes so messages survive crash/segfault.

#[cfg(feature = "dev-logging")]
mod inner {
    use std::sync::{Mutex, OnceLock};

    static INIT: OnceLock<()> = OnceLock::new();

    pub fn init(plugin_name: &'static str) {
        INIT.get_or_init(|| {
            std::panic::set_hook(Box::new(move |info| {
                let path = std::env::temp_dir().join("clap-dev-panic.log");
                let msg = format!(
                    "[{}] PANIC at {:?}:\n{}\n---\n",
                    plugin_name,
                    info.location(),
                    info
                );
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    let _ = f.write_all(msg.as_bytes());
                }
            }));

            let log_path = std::env::temp_dir().join("clap-dev.log");
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .expect("clap-dev.log open failed");

            let _ = tracing_subscriber::fmt()
                .with_writer(Mutex::new(file))
                .with_ansi(false)
                .with_target(false)
                .with_max_level(tracing::Level::DEBUG)
                .try_init();

            tracing::info!("[{}] === dev logging started ===", plugin_name);
        });
    }
}

pub fn init(_plugin_name: &'static str) {
    #[cfg(feature = "dev-logging")]
    inner::init(_plugin_name);
}
