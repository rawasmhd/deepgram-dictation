//! Custom words (key terms) from words.txt, sent to Deepgram so it
//! recognizes names and other rare words. docs/keyterm-prompting.md

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::log;

/// The list that Deepgram rejected (a hash of it), and whether the user
/// still has to be told.
struct Rejected {
    hash: u64,
    notify: bool,
}

static REJECTED: Mutex<Option<Rejected>> = Mutex::new(None);

/// words.txt next to the .exe.
pub fn path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.with_file_name("words.txt"))
}

/// The terms to send with the next dictation. Read from the file each time,
/// so a change works without a restart. Empty if Deepgram rejected this list.
pub fn active() -> Vec<String> {
    let terms = path().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Vec::new, |t| parse(&t));
    if is_rejected(&terms) {
        return Vec::new();
    }
    terms
}

/// Deepgram rejected exactly this list.
pub fn is_rejected(terms: &[String]) -> bool {
    REJECTED.lock().unwrap().as_ref().is_some_and(|r| r.hash == hash(terms))
}

/// One term per line. Empty lines and lines that start with # are skipped,
/// and so is a term that is already in the list (case-insensitive).
pub fn parse(text: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !terms.iter().any(|t| t.eq_ignore_ascii_case(line)) {
            terms.push(line.to_string());
        }
    }
    terms
}

/// Deepgram rejected these terms. Do not send them again until the file
/// changes, and tell the user once.
pub fn reject(terms: &[String]) {
    log(&format!("Deepgram rejected the custom words ({} terms); dictating without them until words.txt changes", terms.len()));
    *REJECTED.lock().unwrap() = Some(Rejected { hash: hash(terms), notify: true });
}

/// True once after a list was rejected: time to tell the user.
pub fn take_notice() -> bool {
    REJECTED.lock().unwrap().as_mut().is_some_and(|r| std::mem::take(&mut r.notify))
}

fn hash(terms: &[String]) -> u64 {
    let mut h = DefaultHasher::new();
    terms.hash(&mut h);
    h.finish()
}

/// Percent-encode a term for a query string. A space becomes %20.
pub fn url_encode(term: &str) -> String {
    let mut out = String::with_capacity(term.len());
    for b in term.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skips_comments_blanks_and_duplicates() {
        let text = "# my words\n\n  Rawas  \nGitHub Actions\r\nrawas\n#Nope\nDr. Smith\n";
        assert_eq!(parse(text), ["Rawas", "GitHub Actions", "Dr. Smith"]);
        assert!(parse("").is_empty());
        assert!(parse("# only a comment\n\n").is_empty());
    }

    #[test]
    fn url_encode_keeps_safe_characters() {
        assert_eq!(url_encode("GitHub Actions"), "GitHub%20Actions");
        assert_eq!(url_encode("Dr. Smith"), "Dr.%20Smith");
        assert_eq!(url_encode("C++ & C#"), "C%2B%2B%20%26%20C%23");
        assert_eq!(url_encode("Zoë"), "Zo%C3%AB");
    }

    #[test]
    fn a_rejected_list_is_reported_once_and_skipped_until_it_changes() {
        let list = vec!["Alpha".to_string(), "Beta".to_string()];
        reject(&list);
        assert!(take_notice());
        assert!(!take_notice());
        assert!(is_rejected(&list));
        assert!(!is_rejected(&["Alpha".to_string()]));
        *REJECTED.lock().unwrap() = None;
    }
}
