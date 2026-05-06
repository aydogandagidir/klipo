# Klipo Security Model & Threat Analysis

**Status:** Draft, locks before v0.1 ships.
**Audience:** Security reviewers, contributors evaluating safety before integrating.
**Companion:** [`crypto.md`](./crypto.md), [`sync-protocol.md`](./sync-protocol.md).

---

## 1. Trust Boundaries

```
┌────────────────────────────────────────────────────────────────────┐
│                         User's machine (TRUSTED)                    │
│                                                                     │
│  ┌────────────────┐   IPC   ┌──────────────────┐                    │
│  │  WebView2 / WK │ ──────► │   Rust core      │                    │
│  │  (frontend)    │ ◄────── │   (storage,      │                    │
│  │  TRUSTED       │         │    clipboard,    │                    │
│  │  but isolated  │         │    crypto)       │                    │
│  └────────────────┘         └────────┬─────────┘                    │
│                                      │                              │
│                                      ▼                              │
│                         ┌────────────────────────┐                  │
│                         │  SQLite + blob disk    │                  │
│                         │  (encrypted at rest    │                  │
│                         │   if user enables)     │                  │
│                         └────────────────────────┘                  │
└────────────────────────────────────────────────────────────────────┘
                                      │ HTTPS (only with sync enabled)
                                      ▼
                       ┌──────────────────────────────────┐
                       │     Sync server (UNTRUSTED)      │
                       │     Sees ciphertext + metadata   │
                       │     Cannot read content          │
                       └──────────────────────────────────┘
```

**Trust assumptions:**
- The user's OS is not compromised at the kernel level. (Defense against kernel rootkits is out of scope; if your OS is owned, all clipboard managers are owned.)
- The user's account password is reasonably strong; we'll enforce min length and recommend a passphrase.
- Other apps on the user's machine can read the clipboard via OS API — that's by design of the OS, not a Klipo attack surface. We don't paint that as our problem; we just don't make it worse.
- Network is hostile (assume MitM possible).
- Sync server may be fully compromised.

---

## 2. STRIDE Threat Model

### 2.1 Spoofing

| Threat | Where | Mitigation | Acceptance Test |
|---|---|---|---|
| Attacker pairs unauthorized device to vault | Pairing flow | PSK in QR + ECDH + 6-word verification | Unit test: replay of pairing payload without PSK fails. |
| Forged device token to server | API auth | Ed25519 signature on token by device key (in OS keychain) | Integration: token without valid sig → 401. |
| Phishing site disguised as Klipo settings page | Frontend | Tauri custom protocol; no public web entry; CSP blocks external scripts | CSP header inspected on every release. |
| Counterfeit installer bundle | Distribution | Code-signed Windows MSI (post-EV cert); Apple notarized DMG; SHA-256 published in release notes | Release checklist mandates signature verification step. |

### 2.2 Tampering

| Threat | Where | Mitigation | Acceptance Test |
|---|---|---|---|
| Sync server alters ciphertext (flip bits) | Sync pull | XChaCha20-Poly1305 AEAD detects any tampering | Fault-injection test: server returns flipped byte → client logs error, drops record. |
| Server swaps two records' ciphertexts | Sync pull | AAD binds (vault_id ‖ record_id ‖ hlc); swap fails AEAD | Fault-injection test in sync integration suite. |
| Local DB corrupted (disk error or malicious app) | Storage | SQLite checksums + AEAD on per-record level | On read failure, isolate corrupted row, surface to user, allow restore from backup. |
| Hostile clipboard write between read and paste (TOCTOU) | Paste flow | We write our chosen content to clipboard then immediately invoke paste; any change in 5ms window aborts | Manual e2e test with adversarial polling app. |
| Malicious snippet executes shell command | Snippet engine (P1) | Snippet engine is **text expansion only**; no shell, no filesystem, no network calls. | Static lint: snippet eval path must not import process/exec/fs modules. |

### 2.3 Repudiation

Repudiation is a low priority for a single-user product (no audit-of-record requirement). Becomes relevant for Team tier.

| Threat | Mitigation | Phase |
|---|---|---|
| User claims a clip wasn't theirs | Clip records carry signed `device_id` + HLC; export has chain. | v0.3 |
| Team admin denies revoking a member | Server-side audit log of `DELETE /devices`; user-side mirror. | v1.0 (Team) |

### 2.4 Information Disclosure

| Threat | Where | Mitigation | Acceptance Test |
|---|---|---|---|
| Clipboard content logged to disk in plaintext | Logging system | Logger has hard ban on logging `text_content`/blob bytes; only hash + size. CI lint checks. | Static analysis: `grep -r "text_content\|blob" src-tauri/src/log` returns 0 hits in production paths. |
| Sensitive content (passwords, tokens) auto-captured | Clipboard watcher | Regex set in [crypto/sensitive.rs] flags + RAM-zero after 30s + UI confirms before paste. | Unit test: clipboard contains an API key → DB row has `sensitive=1`, RAM cache cleared at T+30s. |
| Process memory dump leaks unencrypted MVK | Crypto | `mlock` + `Zeroize` + minimal residency window. Can't fully prevent dump-during-unlock. | Manual: generate Windows minidump while vault unlocked → look for known plaintext markers. (Document residual risk; see §5.) |
| Sync server learns user activity patterns | Sync wire | Ciphertext-only; CBOR record sizes are uniform-ish; v1.0 dummy traffic. | N/A (passive risk). Privacy whitepaper documents server-visible metadata. |
| Backups stored in cloud (e.g., OneDrive) leak data | Local storage | Full-DB export is **always encrypted** (separate vault key). Plain JSON export is per-clip user choice. | Manual: export → file is openable only with passphrase. |
| Debug build leaks via console.log | Frontend | Production build strips `console.*` via Vite plugin. CSP forbids `unsafe-eval`. | CI: `pnpm build` then `grep "console.log" dist/` returns 0. |
| Paste target sees data we didn't intend | Paste flow | Paste only writes user-selected clip; never multi-clip; never blob path. | Unit + manual. |
| URL parameters in clipboard contain tokens | Sensitive detection | URL-with-token regex (covers `?key=`, `?token=`, `?api_key=`) | Pattern set test. |

### 2.5 Denial of Service

| Threat | Mitigation |
|---|---|
| Clipboard flood (malicious app writes 1000 clips/sec) | Rate-limit insertion: max 20 clips/sec to DB; excess dropped with telemetry counter. |
| Vast image clip OOMs us | Hard cap at 50MB per blob; >50MB skipped with warning toast. |
| Sync server flood | Server rate-limits per device (100 req/min). Token revocation on abuse. |
| Quota exhaustion | Storage tier shows usage; auto-prune oldest-non-pinned at 90% threshold. |
| Search regex DoS | We don't accept user regex in search; only FTS5 query syntax. |

### 2.6 Elevation of Privilege

| Threat | Mitigation |
|---|---|
| Frontend (untrusted scripts via injection) calls arbitrary Rust commands | Tauri allowlist: only declared commands callable. CSP forbids inline JS, eval, remote scripts. |
| Snippet body could shell out | Snippet engine is sandboxed string interpolation; never shell. |
| Clipboard polling code escalates | Runs as user; no admin. We never request admin. |
| Auto-update spoofs server | Updater verifies Tauri-signed manifest with known public key. |

---

## 3. Sensitive-Content Auto-Detection

The clipboard watcher runs all incoming text through a regex set. Matches mark `sensitive=true` on the row and trigger UI behavior (red border, blurred preview, paste confirmation).

### 3.1 Initial Pattern Set

```rust
pub const SENSITIVE_PATTERNS: &[(&str, &str)] = &[
    ("credit_card",       r"\b(?:\d[ -]*?){13,19}\b"),
    ("aws_access_key",    r"AKIA[0-9A-Z]{16}"),
    ("aws_secret_key",    r"(?i)aws(.{0,20})?(?-i)['\"]?[A-Za-z0-9/+=]{40}['\"]?"),
    ("openai_key",        r"sk-[A-Za-z0-9]{32,}"),
    ("anthropic_key",     r"sk-ant-[A-Za-z0-9_-]{40,}"),
    ("github_token",      r"gh[pousr]_[A-Za-z0-9]{36,}"),
    ("google_api_key",    r"AIza[0-9A-Za-z_-]{35}"),
    ("stripe_key",        r"(?:sk_live|pk_live|rk_live|sk_test)_[A-Za-z0-9]{24,}"),
    ("jwt",               r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
    ("private_key",       r"-----BEGIN [A-Z ]+PRIVATE KEY-----"),
    ("ssh_private_key",   r"-----BEGIN OPENSSH PRIVATE KEY-----"),
    ("url_with_token",    r"(?i)https?://[^\s]+[?&](?:token|api[_-]?key|access[_-]?token|password)="),
    ("password_field_label", r"(?im)^(?:password|passwd|pwd|secret)\s*[:=]\s*\S{8,}"),
];
```

### 3.2 False Positive / Negative Trade-offs

- **Credit cards** flag aggressively (any 13-19 digit number with separators). Acceptable because user can override.
- **Generic 40-char base64** for AWS secret would FP heavily; we anchor to "aws" within 20 chars before. Real-world tradeoff.
- **JWTs** are sometimes public (e.g., demo tokens); we still flag. User can dismiss.

User can:
- Disable a pattern.
- Add custom patterns.
- Mark a clip "not sensitive" (per-clip override).

### 3.3 What "Sensitive" Triggers

- DB stores `sensitive=1`.
- UI renders red border + blurred body until hover/focus.
- Paste action requires Enter twice (confirm) when sensitive.
- RAM cache for that clip purges at T+30s after capture.
- Excluded from cloud sync **by default** (user can opt-in per-pattern: e.g., "sync GH tokens but not passwords").
- Excluded from export by default (separate "include sensitive" toggle in export dialog).

---

## 4. Excluded Apps (Source Allowlist)

The Windows path uses `GetForegroundWindow` + `GetWindowThreadProcessId` + `QueryFullProcessImageName` to determine the **source app** at clipboard write time.

If `source_app` matches an entry in `excluded_apps` table, **the clip is dropped**. No DB row written.

The default seed list ships in [`src-tauri/src/storage/migrations/001_initial.sql`](../src-tauri/src/storage/migrations/001_initial.sql) and covers common password manager process names (Windows `.exe`) and bundle ids (macOS `com.*`). Contents are technical identifiers the OS produces — required as literal strings for the matcher to work.

User-editable: the settings UI surfaces the current foreground app for one-click adding, plus free-text entry for less common tools.

---

## 5. Residual Risks (Accepted)

We document risks we **cannot mitigate** at the product level so reviewers can decide if Klipo fits their threat model.

- **Memory dump while unlocked** — A privileged local attacker can read live process memory. Standard for any password manager. Mitigation: lock vault when not in use; OS-level disk + memory encryption.
- **Keylogger** — Captures the master password before we ever see it. Same as above.
- **OS clipboard reading by other apps** — By design; not Klipo's surface.
- **Compromised dependency at build time** — Mitigated by `cargo-audit`, `cargo-deny`, `pnpm audit` in CI. Cannot fully prevent supply-chain attacks; we monitor advisories.
- **Hardware vulnerabilities (Spectre, Meltdown class)** — Out of scope.
- **Coercion** — Klipo offers no plausible-deniability features. If you need that, this isn't your tool.

---

## 6. Secure Defaults Checklist

Every release ships these defaults:

- [ ] Telemetry **OFF**.
- [ ] Sync **OFF** (must opt-in, requires vault password setup).
- [ ] Sensitive-content auto-detection **ON** with full pattern set.
- [ ] Excluded apps list **populated** with known password managers.
- [ ] Auto-update **ON** (security patches matter).
- [ ] Crash reporting **OFF** (opt-in; PII-stripped).
- [ ] Clipboard polling on macOS **500ms** (not 100ms — battery).
- [ ] CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' https://updates.klipo.app; img-src 'self' data:; object-src 'none'; base-uri 'self'`.
- [ ] No third-party fonts / CDNs at runtime; everything bundled.
- [ ] Tauri allowlist: minimal set of declared commands.

---

## 7. Vulnerability Reporting

Posted in `SECURITY.md` at root of public repo (created in v0.1 release):

```
Email: security@klipo.app  (or temporarily aydogan.dagidir@yahoo.com.tr)
PGP key fingerprint: <to be generated before v0.1 ships>
Disclosure policy: 90 days from acknowledgment to public disclosure.
                   Critical issues get a CVE filed via MITRE.
```

We commit to:
- Acknowledging within 48 hours.
- Patching critical-severity issues within 7 days.
- Crediting reporters in release notes (unless they prefer anonymity).

---

## 8. Acceptance Tests Per Mitigation

For each row in §2 above, an automated test exists or is filed as a v0.1 issue. Tracked in `docs/security-tests.md` (new file in M2 of Phase B).

Sample tests:

```rust
#[tokio::test]
async fn aead_swap_attack_rejected() {
    let (clip_a, ct_a, aad_a) = encrypt_record(/* ... */);
    let (clip_b, ct_b, aad_b) = encrypt_record(/* ... */);
    // Swap ciphertexts but keep original AADs
    let result = decrypt_record(&EncryptedRecord { ciphertext: ct_a, .. ct_b }, &records_key, &aad_b);
    assert!(matches!(result, Err(CryptoError::AeadTagMismatch)));
}

#[tokio::test]
async fn sensitive_content_zeroizes_after_30s() {
    let clip_id = insert_clip(b"sk-anthropic-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").await;
    let cache = ram_cache.get(&clip_id);
    assert!(cache.is_some());
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(31)).await;
    let cache = ram_cache.get(&clip_id);
    assert!(cache.is_none(), "sensitive plaintext lingered past 30s");
}
```

---

## 9. Defense-in-Depth Inventory (Quick Reference)

What protects what:

| Asset | Layer 1 | Layer 2 | Layer 3 |
|---|---|---|---|
| Clip plaintext at rest | OS disk encryption | Optional SQLCipher (v0.2+) | AEAD on per-clip blob |
| Clip plaintext in transit | TLS 1.3 | XChaCha20-Poly1305 AEAD | AAD binds record identity |
| Master key | Argon2id (256MB cost) | mlock + zeroize | OS keychain seal |
| Device identity | OS keychain (DPAPI/Keychain) | Ed25519 (no password derivation) | n/a |
| Sync auth tokens | TLS | Ed25519 signature | Short expiry (1h) |

Three layers per asset. Failing any one falls back to the next.

---

## 10. Out of Scope (For Now)

- Hardware security keys (YubiKey) for vault unlock — v1.0+
- Multi-factor unlock (password + TOTP) — v1.0+
- FIPS-140-3 validated crypto — not pursued
- SOC 2 Type II for sync service — only if Team tier ships
- Plausible deniability / hidden vaults — not pursued; complexity > value
- Anti-debugging / anti-tampering of binary — not pursued (open source)
- Local clipboard hijack from rogue browser extensions — Browser maker's job
