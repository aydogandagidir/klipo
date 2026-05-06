//! Detection of sensitive clipboard content (passwords, API keys, credit
//! cards, private keys).
//!
//! Strategy: a small set of high-precision regex patterns, applied to a
//! payload's text representation only. A single match is enough to flag the
//! whole clip as `sensitive=1`. Image/binary payloads are never scanned —
//! sensitive heuristics there would be far too lossy.
//!
//! Patterns are stable across releases; adding/removing a pattern is a
//! breaking UX change because users come to rely on what gets flagged.
//! Mirror of `docs/security.md` §3.1.

use std::sync::OnceLock;

use regex::RegexSet;

/// Names + patterns. Keep order stable so test snapshots line up.
///
/// **Careful:** every pattern must compile cleanly under Rust `regex`'s
/// dialect (no look-arounds, no backreferences). The unit test below
/// asserts the entire set compiles.
const PATTERN_DEFS: &[(&str, &str)] = &[
    ("credit_card", r"\b(?:\d[ -]*?){13,19}\b"),
    ("aws_access_key", r"AKIA[0-9A-Z]{16}"),
    (
        "aws_secret_key",
        r"(?i)aws[A-Za-z0-9_]{0,20}['\x22]?[A-Za-z0-9/+=]{40}['\x22]?",
    ),
    ("openai_key", r"sk-[A-Za-z0-9]{32,}"),
    ("anthropic_key", r"sk-ant-[A-Za-z0-9_\-]{40,}"),
    ("github_token", r"gh[pousr]_[A-Za-z0-9]{36,}"),
    ("google_api_key", r"AIza[0-9A-Za-z_\-]{35}"),
    (
        "stripe_key",
        r"(?:sk_live|pk_live|rk_live|sk_test)_[A-Za-z0-9]{24,}",
    ),
    (
        "jwt",
        r"eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}",
    ),
    ("private_key", r"-----BEGIN [A-Z ]+PRIVATE KEY-----"),
    ("ssh_private_key", r"-----BEGIN OPENSSH PRIVATE KEY-----"),
    (
        "url_with_token",
        r"(?i)https?://[^\s]+[?&](?:token|api[_\-]?key|access[_\-]?token|password)=",
    ),
    (
        "password_field_label",
        r"(?im)^(?:password|passwd|pwd|secret)\s*[:=]\s*\S{8,}",
    ),
];

/// Returns the compiled `RegexSet`, building it on first use.
fn pattern_set() -> &'static RegexSet {
    static SET: OnceLock<RegexSet> = OnceLock::new();
    SET.get_or_init(|| {
        let patterns: Vec<&str> = PATTERN_DEFS.iter().map(|(_, pat)| *pat).collect();
        RegexSet::new(&patterns).expect("sensitive regex set compiles at startup")
    })
}

/// Detection outcome. `matched` lists pattern names so callers can log /
/// surface to the user without re-running the regex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensitiveScan {
    pub matched: Vec<&'static str>,
}

impl SensitiveScan {
    pub fn is_sensitive(&self) -> bool {
        !self.matched.is_empty()
    }
}

/// Run the full pattern set against `text`. Empty result = clean.
pub fn scan(text: &str) -> SensitiveScan {
    let matches = pattern_set().matches(text);
    let names = matches
        .into_iter()
        .map(|i| PATTERN_DEFS[i].0)
        .collect::<Vec<_>>();
    SensitiveScan { matched: names }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_set_compiles() {
        // Force initialization; if any pattern is malformed this panics.
        let _ = pattern_set();
        assert_eq!(pattern_set().len(), PATTERN_DEFS.len());
    }

    #[test]
    fn benign_text_is_clean() {
        let r = scan("hello world, just a normal note");
        assert!(!r.is_sensitive(), "matched: {:?}", r.matched);
    }

    #[test]
    fn detects_credit_card_like_digits() {
        let r = scan("my card: 4111 1111 1111 1111 thanks");
        assert!(r.matched.contains(&"credit_card"));
    }

    #[test]
    fn detects_aws_access_key() {
        let r = scan("export AWS_ACCESS_KEY=AKIAIOSFODNN7EXAMPLE\n");
        assert!(r.matched.contains(&"aws_access_key"));
    }

    #[test]
    fn detects_anthropic_key() {
        let r = scan("ANTHROPIC_API_KEY=sk-ant-api03-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-foo");
        assert!(r.matched.contains(&"anthropic_key"));
    }

    #[test]
    fn detects_openai_key() {
        let r = scan("OPENAI_KEY=sk-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(r.matched.contains(&"openai_key"));
    }

    #[test]
    fn detects_github_token() {
        let r = scan("token=ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        assert!(r.matched.contains(&"github_token"));
    }

    #[test]
    fn detects_jwt() {
        // Real-shape JWT (header.payload.signature, each ≥10 chars).
        let r = scan(
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        );
        assert!(r.matched.contains(&"jwt"));
    }

    #[test]
    fn detects_pem_private_key_header() {
        let r = scan("-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n");
        assert!(r.matched.contains(&"private_key"));
    }

    #[test]
    fn detects_url_with_token() {
        let r = scan("Visit https://api.example.com/v1?api_key=secret123");
        assert!(r.matched.contains(&"url_with_token"));
    }

    #[test]
    fn detects_password_field_label() {
        let r = scan("password: hunter2hunter2");
        assert!(r.matched.contains(&"password_field_label"));
    }

    #[test]
    fn multiple_matches_collected() {
        let r = scan("token=ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA and AKIAIOSFODNN7EXAMPLE");
        assert!(r.matched.contains(&"github_token"));
        assert!(r.matched.contains(&"aws_access_key"));
        assert!(r.matched.len() >= 2);
    }

    #[test]
    fn ordinary_url_is_not_flagged() {
        let r = scan("Check out https://example.com/articles/intro");
        assert!(!r.is_sensitive(), "matched: {:?}", r.matched);
    }
}
