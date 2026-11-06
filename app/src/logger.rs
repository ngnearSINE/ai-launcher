//! Minimal file logger: `<config>/launcher.log`, trimmed to the last ~512 KB at start-up.
//! No credentials are ever logged (usage errors carry only step names and status codes).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use parking_lot::Mutex;

struct FileLogger {
    file: Mutex<File>,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= self.level && (m.target().starts_with("ai_launcher") || m.target().starts_with("launcher_") || m.level() <= log::Level::Warn)
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            let _ = writeln!(self.file.lock(), "{} {:5} {} {}", chrono::Local::now().format("%m-%d %H:%M:%S%.3f"), r.level(), r.target(), r.args());
        }
    }
    fn flush(&self) {
        let _ = self.file.lock().flush();
    }
}

pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("launcher.log");
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 512 * 1024 {
            if let Ok(bytes) = std::fs::read(&path) {
                let _ = std::fs::write(&path, &bytes[bytes.len() - 256 * 1024..]);
            }
        }
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else { return };
    let level = if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info };
    let logger: &'static FileLogger = Box::leak(Box::new(FileLogger { file: Mutex::new(file), level }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
}
