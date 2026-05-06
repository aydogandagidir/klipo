# Security Policy

Klipo handles arguably the most sensitive data on a desktop — every API key, password, and snippet of source code that passes through the clipboard. We take that seriously.

## Reporting a Vulnerability

If you find a security issue, **please do not file a public GitHub issue.** Instead:

1. Email **security@klipo.app** with a description, reproduction steps, and the version + OS you tested on.
2. We acknowledge receipt within **72 hours** (usually faster).
3. We aim for an initial assessment within **7 days** and a fix or disclosure plan within **30 days** for critical issues, **90 days** for everything else.

Encrypted reports welcome — see the project's PGP key at <https://klipo.app/pgp.asc> (fingerprint published on the first stable release).

## What's in Scope

| Component                                 | Scope                                                                  |
| ----------------------------------------- | ---------------------------------------------------------------------- |
| Klipo desktop app (Windows / macOS)       | ✅ in scope                                                            |
| `tauri-plugin-*` crates                   | ✅ in scope when triggered through Klipo's surface                     |
| Sync server (Faz D, ships with v0.3)      | ✅ in scope when launched                                              |
| Third-party dependencies                  | Out of scope unless we missed a CVE in our pin                         |
| Social engineering, physical attacks      | Out of scope                                                           |

## What Counts as a Vulnerability

- **Privacy leakage**: any path where clipboard contents leave the user's machine without explicit opt-in.
- **Sensitive-content bypass**: a way to make the auto-detect regex set fail open for a payload that should have been flagged.
- **Excluded-apps bypass**: clipboard captures from a foreground process the user has excluded.
- **Crypto** (v0.3+): vault key derivation, key exchange, payload encryption / decryption.
- **Privilege escalation**: any path that lets Klipo run code outside its sandbox.
- **Persistent injection**: any way to plant payloads in the SQLite store that execute during search / paste.

## Recognition

If you'd like public credit, we'll add you to the security acknowledgments section of the release notes. Anonymous reports are equally welcome.

We don't have a paid bug bounty program yet (Klipo is pre-1.0 and self-funded). When we add one, this section will be updated.

## Architecture and Threat Model

The full architecture-level threat model lives in [`docs/security.md`](./docs/security.md). It covers excluded-apps, sensitive-content auto-detect, RAM zeroize windows, and the sync server trust boundary (planned for v0.3).
