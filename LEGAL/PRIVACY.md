# Klipo Privacy Policy

**Effective from Klipo version 0.1.3.**
**Last updated: 2026-05-08.**

This Privacy Policy explains how Klipo (the "Software"), distributed by **Aydoğan Dağıdır, trading as bluedev** ("bluedev", "we", "us"), handles your data.

> **TL;DR:** Klipo is offline-first. Your clipboard contents stay on your machine. Telemetry is opt-in. We do not have a cloud account system in v0.1.x.

---

## 1. Data we do **not** collect

Klipo does **not** transmit any of the following off your device:

- Clipboard contents (text, images, files, RTF, HTML).
- Search queries you type into the popup.
- Pinned, deleted, or otherwise interacted-with clips.
- File paths, file contents, or screenshots.
- Personally identifiable information (name, email, phone), unless you contact us via support and provide it voluntarily.
- Browsing history, keystrokes outside of Klipo's own input fields, or screen content.

All clipboard data captured by Klipo is stored **locally** in an encrypted SQLite database under your operating system's standard application-data directory:

- Windows: `%APPDATA%\Klipo\` (typically `C:\Users\<you>\AppData\Roaming\Klipo\`).

## 2. Data we may collect (only with your opt-in)

If you enable "Send anonymous usage data" in **Settings → Privacy**, the Software may transmit:

- **Crash reports** — stack traces, OS version, Klipo version. No clipboard contents are ever included.
- **Feature usage counters** — aggregate counts of which Settings panels are opened or which keyboard shortcuts are used. No values, no clip content.
- **Anonymous installation ID** — a randomly generated UUID stored locally; does not identify you personally.

This setting is **off by default**. You can disable it at any time, and any future telemetry events will stop immediately. We do not retain telemetry data beyond 90 days.

## 3. Update checks

Klipo periodically checks for updates by fetching a small JSON manifest from a public GitHub Releases URL. This request is unauthenticated and does not include any personal information. GitHub's standard server logs may record the request IP per their own privacy policy.

## 4. License validation (when applicable)

If you purchased a license through Gumroad, license-key validation may transmit your license key to a bluedev validation endpoint at activation time and at periodic intervals. The endpoint logs only the license key and a timestamp.

## 5. Data we receive when you contact support

If you email **support@bluedev.dev**, we receive the email address you write from, the message contents, and any attachments you choose to send. We retain support correspondence for up to 24 months for service quality and dispute-resolution purposes.

## 6. Data sharing

bluedev does not sell, rent, or share user data with third parties for marketing or advertising purposes. We share data only:

- with payment processors (e.g., Gumroad) to fulfill your purchase, in accordance with their privacy policies;
- with hosting providers (e.g., GitHub for release manifests) strictly for technical delivery;
- when required by applicable law or valid legal process.

## 7. Your rights

Depending on your jurisdiction (EU, UK, Türkiye KVKK, California CCPA), you may have rights to:

- request access to the personal data we hold about you;
- request correction or deletion of that data;
- withdraw consent for telemetry at any time;
- lodge a complaint with a data-protection authority.

To exercise any of these rights, email **support@bluedev.dev**. We will respond within 30 days.

## 8. Children

Klipo is not directed at children under 13 (or under 16 in the EEA). We do not knowingly collect personal data from children.

## 9. Changes to this policy

If we materially change this policy, we will publish the updated version in a new release of the Software, note the change in the release notes, and update the "Last updated" date above. Continued use of the Software after such changes indicates acceptance.

---

## 10. Contact

**Data controller:** Aydoğan Dağıdır, trading as bluedev
**Website:** https://bluedev.dev
**Privacy contact:** support@bluedev.dev
