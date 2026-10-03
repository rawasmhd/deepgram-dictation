//! Settings and the API key.
//!
//! The full key handling (setup, messages, .env next to the installed
//! .exe) is issue #10. This is the minimum that #8 needs.

use std::path::{Path, PathBuf};

/// "streaming" (default) or "batch". The benchmark can override it with
/// DICTATION_MODE, so both versions run with the same settings.
pub fn streaming() -> bool {
    std::env::var("DICTATION_MODE").map_or(true, |m| m != "batch")
}

/// DEEPGRAM_API_KEY from the environment, or from the first .env file
/// next to the .exe or in a folder above it (so a build in
/// rust/target/release finds the repo's .env).
pub fn api_key() -> Option<String> {
    if let Ok(key) = std::env::var("DEEPGRAM_API_KEY") {
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
        (name.trim() == "DEEPGRAM_API_KEY").then(|| value.trim().trim_matches(['"', '\'']).to_string())
    })
}

/// Where log lines go, besides the console: DICTATION_LOG, if set.
pub fn log_file() -> Option<PathBuf> {
    std::env::var_os("DICTATION_LOG").map(PathBuf::from)
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
}
