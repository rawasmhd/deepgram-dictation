//! The log: `dictation.log` next to the .exe (there is no console in a
//! release build). DICTATION_LOG overrides the path, for the benchmark.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::System::SystemInformation::GetLocalTime;

const MAX_BYTES: u64 = 1_000_000; // start a new log above this size

static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

pub fn init() {
    FILE.get_or_init(|| {
        let path = log_path()?;
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
            let _ = std::fs::remove_file(&path);
        }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path).ok()?;
        Some(Mutex::new(file))
    });
    log(&format!("\n--- started {} ---", now()));
}

fn log_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DICTATION_LOG") {
        return Some(path.into());
    }
    Some(std::env::current_exe().ok()?.with_file_name("dictation.log"))
}

pub fn log(msg: &str) {
    println!("{msg}");
    if let Some(Some(file)) = FILE.get() {
        if let Ok(mut f) = file.lock() {
            let _ = writeln!(f, "{msg}");
        }
    }
}

fn now() -> String {
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}
