//! The crash log: a panic anywhere in the engine appends its message, place and backtrace to `crash.log` in the
//! settings folder (`render_settings::settings_path`'s folder; none when the settings file is disabled), then the
//! default hook prints it as before. A panic inside the window's event loop is followed by winit's "no handler was
//! set" line, which alone says nothing; the log keeps the real cause.

use std::io::Write;

/// The log's path (the settings folder's `crash.log`).
pub fn path() -> Option<std::path::PathBuf> { crate::render_settings::settings_path().and_then(|p| p.parent().map(|d| d.join("crash.log"))) }

/// Installs the hook (once, at start-up).
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(p) = path() {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let thread = std::thread::current().name().unwrap_or("?").to_string();
            let bt = std::backtrace::Backtrace::force_capture();
            if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
                let _ = writeln!(f, "=== panic at unix time {secs}, version {}, thread {thread}\n{info}\n{bt}\n", env!("CARGO_PKG_VERSION"));
                eprintln!("crash log: {}", p.display());
            }
        }
        #[cfg(windows)]
        crate::user_error::report(&format!("The game stopped unexpectedly: {info}\n\nSee crash.log in the settings folder for details."));
        default(info);
    }));
}
