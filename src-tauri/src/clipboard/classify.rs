//! Content classification for captured clips — fully local, no network.
//!
//! Where `sensitive.rs` answers *"is this a secret?"* (a security flag),
//! `classify.rs` answers *"what kind of thing is this?"* (a UX hint). On
//! capture the result seeds the clip's first auto label (see
//! `Storage::link_auto_label`), shown in the popup as a label chip + filter.
//!
//! Design, mirroring `sensitive.rs`:
//!   - A small set of high-precision detectors, applied to a clip's text.
//!   - **First match wins** (unlike the sensitive `RegexSet`, order matters
//!     here) — detectors are ordered most-specific first.
//!   - Returns `Option<&'static str>`; the key is STABLE (stored as
//!     `clip_labels.auto_key`); `None` = ordinary prose, which gets no auto
//!     label. `auto_label_name` maps each key to its default display name.
//!
//! Privacy: this is regex + a JSON parse, nothing more. No model, no upload —
//! consistent with Klipo's "private, local" posture. Binary/image clips are
//! never classified (the pipeline only calls us with text payloads).

use std::sync::OnceLock;

use regex::Regex;

/// Default Turkish display name for an auto-detected classifier key. Seeds a
/// clip's first label on capture. MUST stay in sync with the CASE in
/// `migrations/005_labels.sql`. Unknown keys fall back to a generic label.
pub fn auto_label_name(key: &str) -> &'static str {
    match key {
        "url" => "Bağlantı",
        "email" => "E-posta",
        "phone" => "Telefon",
        "iban" => "IBAN",
        "color" => "Renk",
        "code" => "Kod",
        "json" => "JSON",
        "number" => "Sayı",
        "path" => "Yol",
        _ => "Etiket",
    }
}

/// Compiled detectors, built once on first use.
struct Patterns {
    url: Regex,
    email: Regex,
    iban: Regex,
    color: Regex,
    number: Regex,
    path: Regex,
    markup: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // Single-token URL (http/https/www), no embedded whitespace.
        url: Regex::new(r"(?i)^(?:https?://|www\.)\S+$").unwrap(),
        // A lone email address (no surrounding prose).
        email: Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap(),
        // IBAN after whitespace is stripped: 2 letters, 2 check digits, 11-30
        // alphanumerics (covers TR's TR + 24 digits and all SEPA formats).
        iban: Regex::new(r"(?i)^[a-z]{2}[0-9]{2}[a-z0-9]{11,30}$").unwrap(),
        // #rgb / #rgba / #rrggbb / #rrggbbaa, or a css color function.
        color: Regex::new(
            r"(?i)^(?:#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})|(?:rgb|rgba|hsl|hsla)\s*\(.*\))$",
        )
        .unwrap(),
        // Integer or decimal, optionally grouped (1.234.567,89 / 3.14 / -42).
        number: Regex::new(r"^[+-]?\d+(?:[.,]\d+)*$").unwrap(),
        // Filesystem path: Windows drive / UNC, Unix absolute, or ~ home.
        path: Regex::new(r"^(?:[A-Za-z]:[\\/]|\\\\|/|~[\\/])[^\r\n]*$").unwrap(),
        // A real HTML/XML/JSX tag (`<div>`, `</span>`, `<br/>`) — the letter
        // after `<` or `</` keeps prose like "3 </ 5" from matching.
        markup: Regex::new(r"</?[a-zA-Z][\w-]*[^>]*>").unwrap(),
    })
}

/// Classify a clip's text into a stable category string, or `None` for
/// ordinary prose. Operates on the trimmed text — leading/trailing whitespace
/// from a sloppy copy shouldn't change the verdict.
pub fn classify(text: &str) -> Option<&'static str> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let p = patterns();

    if p.email.is_match(t) {
        return Some("email");
    }
    if p.url.is_match(t) {
        return Some("url");
    }
    // IBANs are often copied with spaces every 4 chars — fold them out first.
    let despaced: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    if p.iban.is_match(&despaced) {
        return Some("iban");
    }
    if p.color.is_match(t) {
        return Some("color");
    }
    if is_phone(t) {
        return Some("phone");
    }
    if p.number.is_match(t) {
        return Some("number");
    }
    if p.path.is_match(t) {
        return Some("path");
    }
    // JSON before code: a JSON blob is also "code-ish" but more specific.
    if is_json(t) {
        return Some("json");
    }
    if is_code(t, &p.markup) {
        return Some("code");
    }
    None
}

/// Phone detector. The string must be made entirely of phone characters
/// (digits and `+ - ( ) ` space), carry 7-15 digits, use `+` only as a leading
/// prefix, and show some phone signal (a `+`, a separator, or ≥10 digits) so a
/// short run of bare digits is left to the `number` detector. `.` is
/// deliberately NOT a phone separator — it collides with grouped numbers.
fn is_phone(t: &str) -> bool {
    if !t
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')'))
    {
        return false;
    }
    let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
    if !(7..=15).contains(&digits) {
        return false;
    }
    // `+` is only meaningful as an international prefix.
    if t.matches('+').count() > 1 || (t.contains('+') && !t.starts_with('+')) {
        return false;
    }
    let has_sep = t.chars().any(|c| matches!(c, ' ' | '-' | '(' | ')'));
    t.starts_with('+') || has_sep || digits >= 10
}

/// Structurally-valid JSON object or array. We require `{`/`[` … `}`/`]`
/// bookends so a bare number or quoted string (both valid JSON values) isn't
/// swallowed here — those belong to `number` / prose.
fn is_json(t: &str) -> bool {
    let first = t.as_bytes().first().copied();
    let last = t.as_bytes().last().copied();
    let bookended =
        matches!(first, Some(b'{') | Some(b'[')) && matches!(last, Some(b'}') | Some(b']'));
    bookended && serde_json::from_str::<serde_json::Value>(t).is_ok()
}

/// Heuristic source-code detector. Conservative on purpose: ordinary prose
/// must not be tagged. A real markup tag is sufficient on its own; otherwise
/// we require at least two independent code signals.
fn is_code(t: &str, markup: &Regex) -> bool {
    if markup.is_match(t) {
        return true;
    }

    let mut score = 0;

    if t.contains('{') && t.contains('}') {
        score += 1;
    }
    if t.contains("=>") || t.contains("->") {
        score += 1;
    }
    if t.contains(");") || t.contains("};") || t.contains(";\n") {
        score += 1;
    }
    if t.contains("::")
        || t.contains("==")
        || t.contains("!=")
        || t.contains("&&")
        || t.contains("||")
    {
        score += 1;
    }

    const KW: &[&str] = &[
        "function ",
        "const ",
        "let ",
        "var ",
        "import ",
        "export ",
        "class ",
        "def ",
        "fn ",
        "func ",
        "public ",
        "private ",
        "return ",
        "#include",
        "package ",
        "SELECT ",
        "select ",
        "<?php",
    ];
    if KW.iter().any(|k| t.contains(k)) {
        score += 1;
    }

    let indented = t
        .lines()
        .filter(|l| l.starts_with("    ") || l.starts_with('\t'))
        .count();
    if indented >= 2 {
        score += 1;
    }

    score >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_compile() {
        let _ = patterns();
    }

    #[test]
    fn plain_prose_is_uncategorized() {
        assert_eq!(classify("just a normal note to self"), None);
        assert_eq!(classify("Merhaba, bugün hava çok güzel."), None);
        assert_eq!(classify(""), None);
        assert_eq!(classify("   "), None);
    }

    #[test]
    fn detects_email() {
        assert_eq!(classify("aydogan.dagidir@yahoo.com.tr"), Some("email"));
        assert_eq!(classify("  user@example.com  "), Some("email"));
        // An email embedded in prose is NOT a lone email → not category email.
        assert_ne!(
            classify("mail me at user@example.com please"),
            Some("email")
        );
    }

    #[test]
    fn detects_url() {
        assert_eq!(classify("https://bluedev.dev/products/klipo"), Some("url"));
        assert_eq!(classify("http://example.com/a?b=c#d"), Some("url"));
        assert_eq!(classify("www.example.com"), Some("url"));
    }

    #[test]
    fn detects_iban() {
        // TR IBAN with the usual 4-char grouping.
        assert_eq!(classify("TR33 0006 1005 1978 6457 8413 26"), Some("iban"));
        assert_eq!(classify("DE89370400440532013000"), Some("iban"));
    }

    #[test]
    fn detects_color() {
        assert_eq!(classify("#0a84ff"), Some("color"));
        assert_eq!(classify("#FFF"), Some("color"));
        assert_eq!(classify("rgb(10, 132, 255)"), Some("color"));
        assert_eq!(classify("hsla(210, 100%, 52%, 0.8)"), Some("color"));
    }

    #[test]
    fn detects_phone() {
        assert_eq!(classify("+90 555 123 45 67"), Some("phone"));
        assert_eq!(classify("0555 123 45 67"), Some("phone"));
        assert_eq!(classify("(212) 555-0199"), Some("phone"));
    }

    #[test]
    fn detects_number() {
        assert_eq!(classify("42"), Some("number"));
        assert_eq!(classify("3.14"), Some("number"));
        assert_eq!(classify("1.234.567,89"), Some("number"));
        assert_eq!(classify("-17"), Some("number"));
        // A short bare run of digits is a number, not a phone.
        assert_eq!(classify("1234567"), Some("number"));
    }

    #[test]
    fn detects_path() {
        assert_eq!(classify(r"C:\Users\adagidir\Desktop"), Some("path"));
        assert_eq!(classify("/usr/local/bin/klipo"), Some("path"));
        assert_eq!(classify(r"\\server\share\file.txt"), Some("path"));
        assert_eq!(classify("~/.config/klipo"), Some("path"));
    }

    #[test]
    fn detects_json() {
        assert_eq!(classify(r#"{"a": 1, "b": [2, 3]}"#), Some("json"));
        assert_eq!(classify("[1, 2, 3]"), Some("json"));
        // A bare number is valid JSON but must NOT be tagged json.
        assert_eq!(classify("123"), Some("number"));
        // Malformed object → falls through (prose).
        assert_eq!(classify("{not valid json at all"), None);
    }

    #[test]
    fn detects_code() {
        let rust = "fn main() {\n    println!(\"hi\");\n}";
        assert_eq!(classify(rust), Some("code"));
        let js = "const add = (a, b) => {\n    return a + b;\n};";
        assert_eq!(classify(js), Some("code"));
        let html = "<div class=\"x\">\n  <span>hi</span>\n</div>";
        assert_eq!(classify(html), Some("code"));
    }

    #[test]
    fn prose_with_one_brace_is_not_code() {
        // A single weak signal must not be enough to call prose "code".
        assert_eq!(classify("I think { this } is just a note"), None);
    }
}
