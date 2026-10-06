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

/// Deepgram's limit for all key terms in one request.
pub const TOKEN_LIMIT: usize = 500;

/// The first lines of a new words.txt.
const HEADER: &str = "# Custom words for Deepgram Dictation: one word or phrase per line.\n\
# Write each one the way you want to see it, for example GitHub.\n";

/// How full the list is, from the token estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    Ok,
    /// 80% of the limit or more.
    Near,
    /// More than the limit: Deepgram will probably reject it.
    Over,
}

/// words.txt next to the .exe.
pub fn path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.with_file_name("words.txt"))
}

/// The terms to send with the next dictation. Read from the file each time,
/// so a change works without a restart. Empty if Deepgram rejected this list.
pub fn active() -> Vec<String> {
    let terms = load();
    if is_rejected(&terms) {
        return Vec::new();
    }
    terms
}

/// All the terms in words.txt. Empty if there is no file.
pub fn load() -> Vec<String> {
    path().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Vec::new, |t| parse(&t))
}

/// Write the terms to words.txt. The comment lines stay.
pub fn save(terms: &[String]) -> Result<PathBuf, String> {
    let path = path().ok_or("no folder for words.txt")?;
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, render(&old, terms)).map_err(|e| format!("{}: {e}", path.display()))?;
    log(&format!("custom words: saved {} terms to {}", terms.len(), path.display()));
    Ok(path)
}

/// The new file: the comment lines of the old file (or a header), then one
/// term per line. Windows line endings, so Notepad shows it correctly.
pub fn render(old: &str, terms: &[String]) -> String {
    let old = old.strip_prefix('\u{FEFF}').unwrap_or(old);
    let comments: Vec<&str> = old.lines().map(str::trim_end).filter(|l| l.trim_start().starts_with('#')).collect();
    let mut out = String::new();
    if comments.is_empty() {
        out.push_str(HEADER);
    } else {
        for line in comments {
            out.push_str(line);
            out.push('\n');
        }
    }
    for term in terms {
        out.push_str(term);
        out.push('\n');
    }
    out.replace('\n', "\r\n")
}

/// A safe (high) guess of how many tokens Deepgram counts. Deepgram does
/// not publish its method: count one token for every 3 characters, and at
/// least one for each word.
pub fn estimate_tokens(terms: &[String]) -> usize {
    terms.iter().flat_map(|t| t.split_whitespace()).map(|w| w.chars().count().div_ceil(3).max(1)).sum()
}

pub fn fill(tokens: usize) -> Fill {
    if tokens > TOKEN_LIMIT {
        Fill::Over
    } else if tokens * 5 >= TOKEN_LIMIT * 4 {
        Fill::Near
    } else {
        Fill::Ok
    }
}

/// Deepgram rejected exactly this list.
pub fn is_rejected(terms: &[String]) -> bool {
    REJECTED.lock().unwrap().as_ref().is_some_and(|r| r.hash == hash(terms))
}

/// One term per line. Empty lines and lines that start with # are skipped,
/// and so is a term that is already in the list (case-insensitive).
pub fn parse(text: &str) -> Vec<String> {
    // Notepad can save a byte order mark at the start
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
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
        assert_eq!(parse("\u{FEFF}# saved by Notepad\r\nRawas\r\n"), ["Rawas"]);
    }

    #[test]
    fn render_keeps_comments_and_writes_one_term_per_line() {
        let terms = ["Rawas".to_string(), "GitHub Actions".to_string()];
        let out = render("# mine\nOld\n  # indented\n", &terms);
        assert_eq!(out, "# mine\r\n  # indented\r\nRawas\r\nGitHub Actions\r\n");
        assert_eq!(render("\u{FEFF}# bom\r\nOld\r\n", &terms[..1]), "# bom\r\nRawas\r\n");
        let new = render("", &terms);
        assert!(new.starts_with("# Custom words"));
        assert_eq!(parse(&new), terms);
        assert!(parse(&render("", &[])).is_empty());
    }

    #[test]
    fn token_estimate_is_at_least_one_per_word() {
        let t = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(estimate_tokens(&t(&[])), 0);
        assert_eq!(estimate_tokens(&t(&["a"])), 1);
        assert_eq!(estimate_tokens(&t(&["Rawas"])), 2);
        assert_eq!(estimate_tokens(&t(&["GitHub Actions", "Dr. Smith"])), 2 + 3 + 1 + 2);
        // the list that Deepgram rejected in the test on 2026-10-06 is "Over"
        let many: Vec<String> = (0..300).map(|i| format!("Zyxquar Plimbet Vorshnik{i}")).collect();
        assert_eq!(fill(estimate_tokens(&many)), Fill::Over);
    }

    #[test]
    fn fill_levels() {
        assert_eq!(fill(0), Fill::Ok);
        assert_eq!(fill(399), Fill::Ok);
        assert_eq!(fill(400), Fill::Near);
        assert_eq!(fill(500), Fill::Near);
        assert_eq!(fill(501), Fill::Over);
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
