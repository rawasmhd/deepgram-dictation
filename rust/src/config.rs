//! Settings, the API key, and autostart.

use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::*;

use crate::overlay::wide;

const KEY_NAME: &str = "DEEPGRAM_API_KEY";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "DeepgramDictation";
/// The Startup shortcut that setup.bat creates for the Python version.
const PYTHON_SHORTCUT: &str = r"Microsoft\Windows\Start Menu\Programs\Startup\Deepgram Dictation.lnk";

/// "streaming" (default) or "batch". The benchmark can override it with
/// DICTATION_MODE, so both versions run with the same settings.
pub fn streaming() -> bool {
    std::env::var("DICTATION_MODE").map_or(true, |m| m != "batch")
}

/// Streaming only: paste each phrase while you speak (default, like
/// dictate.py). DICTATION_LIVE_PASTE=0 pastes all the text when you stop.
pub fn live_paste() -> bool {
    std::env::var("DICTATION_LIVE_PASTE").map_or(true, |v| !matches!(v.as_str(), "0" | "false"))
}

// -- API key ------------------------------------------------------------------

/// DEEPGRAM_API_KEY from the environment, or from the first .env file
/// next to the .exe or in a folder above it (so a build in
/// rust/target/release finds the repo's .env).
pub fn api_key() -> Option<String> {
    if let Ok(key) = std::env::var(KEY_NAME) {
        if !key.trim().is_empty() {
            return Some(key.trim().to_string());
        }
    }
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().skip(1).take(5).map(|dir| dir.join(".env")).find_map(|p| read_env_file(&p))
}

fn read_env_file(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    parse_env(&text)
}

fn parse_env(text: &str) -> Option<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == KEY_NAME).then(|| value.trim().trim_matches(['"', '\'']).to_string())
    })
}

/// Save the key to .env next to the .exe. Other lines in the file stay.
pub fn save_api_key(key: &str) -> Result<PathBuf, String> {
    let path = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name(".env");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, with_key(&old, key)).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// `text` (a .env file) with the key line replaced, or added.
fn with_key(text: &str, key: &str) -> String {
    let line = format!("{KEY_NAME}={key}");
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| match l.split_once('=') {
            Some((name, _)) if name.trim() == KEY_NAME && !l.trim_start().starts_with('#') => {
                found = true;
                line.clone()
            }
            _ => l.to_string(),
        })
        .collect();
    if !found {
        lines.push(line);
    }
    lines.join("\n") + "\n"
}

// -- autostart ----------------------------------------------------------------

pub fn autostart_enabled() -> bool {
    unsafe {
        let mut size = 0u32;
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(RUN_KEY).as_ptr(),
            wide(RUN_VALUE).as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        ) == ERROR_SUCCESS
    }
}

/// Start (or not) at login, through the user's Run registry key. Returns
/// true if it removed the Python version's Startup shortcut, because only
/// one version can run at a time.
pub fn set_autostart(enabled: bool) -> Result<bool, String> {
    unsafe {
        if !enabled {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), wide(RUN_VALUE).as_ptr());
            return Ok(false);
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let command = wide(&format!("\"{}\"", exe.display()));
        let status = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            wide(RUN_KEY).as_ptr(),
            wide(RUN_VALUE).as_ptr(),
            REG_SZ,
            command.as_ptr().cast(),
            (command.len() * 2) as u32,
        );
        if status != ERROR_SUCCESS {
            return Err(format!("registry error {status}"));
        }
    }
    let shortcut = std::env::var_os("APPDATA").map(|d| Path::new(&d).join(PYTHON_SHORTCUT));
    Ok(shortcut.is_some_and(|s| std::fs::remove_file(s).is_ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_quoted_key() {
        assert_eq!(parse_env("DEEPGRAM_API_KEY=\"abc\"\n"), Some("abc".into()));
    }

    #[test]
    fn ignores_comments_blanks_and_other_names() {
        let text = "# comment\n\nOTHER=1\n DEEPGRAM_API_KEY = 'xyz' \n";
        assert_eq!(parse_env(text), Some("xyz".into()));
    }

    #[test]
    fn missing_key_is_none() {
        assert_eq!(parse_env("OTHER=1\n"), None);
    }

    #[test]
    fn saving_replaces_the_key_and_keeps_other_lines() {
        let old = "# my keys\nOTHER=1\nDEEPGRAM_API_KEY=old\n";
        assert_eq!(with_key(old, "new"), "# my keys\nOTHER=1\nDEEPGRAM_API_KEY=new\n");
    }

    #[test]
    fn saving_adds_the_key_when_missing() {
        assert_eq!(with_key("", "new"), "DEEPGRAM_API_KEY=new\n");
        assert_eq!(with_key("# DEEPGRAM_API_KEY=x\n", "new"), "# DEEPGRAM_API_KEY=x\nDEEPGRAM_API_KEY=new\n");
    }
}
