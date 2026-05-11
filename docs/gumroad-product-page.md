# Klipo — Gumroad Product Page

> Source copy for the Gumroad listing. Drop into the product description, FAQ,
> and "What's included" fields. Launch price: **$29**.
>
> **TODO before publish:** WA contacts exporter formatına göre re-order section'lar
> (path verilince adapt edilecek — header/sub-header pozisyonu, FAQ Q&A sayısı,
> demo video yerleşimi, kapak ölçüleri).

---

## Headline (one-liner)

**Klipo — A keyboard-first clipboard manager that respects your time and your data.**

## Sub-headline (under the price)

Capture every `Ctrl+C` you do. Find any clip in milliseconds. Paste it back with a single key. Built for Windows by [bluedev](https://bluedev.dev).

---

## Product description (≈ 250 words)

You copy hundreds of things a day — code snippets, links, addresses, error messages, that one Stripe key you swear you saved somewhere. They all vanish the next time you copy.

**Klipo remembers them for you.** It runs silently in the Windows tray, captures every clipboard event (text, images, files, RTF, HTML) into a local SQLite database, and gives you instant search via `Ctrl+Alt+V`.

What makes it different:

- **Fast.** Popup opens in under 100 ms. Search across 10,000+ items finishes in under 50 ms thanks to SQLite FTS5 with Türkçe-aware character folding.
- **Private.** All data stays on your device. No cloud, no telemetry by default. Sensitive content (API keys, credit cards, JWTs, 13 patterns total) is auto-detected and protected with a paste-confirm dialog.
- **Keyboard-first.** Every action has a shortcut. Mouse is optional. Type to filter, ↑/↓ to navigate, Enter to paste back into your previous app.
- **Native.** Built on Tauri 2 + Rust. ~3.8 MB installer. No Electron bloat, no background memory leak, no Chromium tab tax.

**$29 — lifetime updates in the v0.x series.** Use the 14-day free trial to decide before you pay. See [the EULA](https://github.com/aydogandagidir/klipo/blob/main/LEGAL/EULA.md).

> Heads-up: Klipo is **Windows 10 (1809+) / Windows 11 — Windows-only at this time. macOS and Linux are not currently supported.**

---

## What's included

- ✅ Klipo for Windows 10 / 11 (NSIS installer, ~3.8 MB; publisher metadata: bluedev)
- ✅ All updates in the v0.x series (currently v0.1.3) at no extra cost — auto-update built-in
- ✅ Per-seat license — install on up to 3 of your own devices
- ✅ Email support: support@bluedev.dev
- ✅ 14-day free trial — use it to decide before you pay

---

## Feature list (bullets for Gumroad's "Features" section)

- Capture every clipboard event: text, images, files, RTF, HTML
- Lightning-fast search across 10,000+ items (SQLite FTS5, BM25-ranked)
- Türkçe character folding (ğ/g, ş/s, ı/i, ç/c, ö/o, ü/u — all foldable)
- 13-pattern sensitive content guard (API keys, credit cards, JWTs, etc.)
- Per-app exclusion list (password managers excluded by default)
- Native OS drag-and-drop from popup → any target app
- Custom global hotkey, dark/light theme, autostart, full Settings UI
- Signed auto-updater — upgrades arrive in seconds with no manual download
- All data stored locally in encrypted SQLite — never leaves your machine
- Works fully offline; no account required

---

## FAQ

**Is Klipo really only for Windows?**
Yes — Windows 10 (1809+) and Windows 11 only. macOS and Linux are not currently supported. If we ever ship them, that will be announced separately on bluedev.dev/products/klipo — there is no buying claim attached today.

**Where is my clipboard history stored?**
Locally, in an encrypted SQLite database at `%APPDATA%\Klipo\`. Nothing leaves your machine. Even crash reports require explicit opt-in in Settings → Privacy.

**How many devices can I install on?**
Personal license = up to 3 devices that you personally own. Team licenses are available on request — email support@bluedev.dev.

**Do I get the source code?**
The historical v0.1.0–0.1.2 versions are freely available under Apache-2.0. From v0.1.3 onward, Klipo is a commercial product under a proprietary EULA, but bluedev publishes detailed architecture docs and changelogs for transparency.

**What if I don't like it?**
That's exactly what the 14-day free trial is for — install Klipo, use it for two weeks with every Pro feature unlocked, then decide if it's worth $29. If you do pay and Klipo isn't a fit, email support@bluedev.dev and we'll talk it through. Statutory consumer rights (e.g. the EU 14-day right of withdrawal) still apply where required by law.

**Why does Windows show "Unknown publisher" on first install?**
Klipo is currently distributed without an Authenticode (EV) code-signing certificate. Acquiring one costs $200–400/yr and requires hardware tokens or cloud HSM — that overhead would push the indie price ($29 lifetime) out of reach. Instead, the installer carries `Publisher: bluedev` metadata (right-click installer → Properties → Details to verify), the auto-update payload itself **is** Ed25519-signed (so all future updates verify against a public key embedded in your installed copy), and the trust anchors are bluedev's public release notes + the brand at [bluedev.dev](https://bluedev.dev). On first install: click **More info** → **Run anyway**. We'll add Authenticode in a future release once the brand is established.

**Why $29 and not $9?**
Klipo is built on Tauri 2 + Rust + SQLite FTS5 — engineering you can verify in the public release notes and architecture docs. The 13-pattern sensitive-content guard, the per-app exclusion list, the encrypted local store, the keyboard-first UX, the auto-update plumbing — they're not weekend-project work. $29 buys you the engineered version *plus* every v0.x update at no extra cost. Compare to clipboard managers that charge $4-8/month subscription — $29 lifetime is cheaper after 4-6 months.

**Is it worth it for me?**
If you copy code, links, snippets, or anything you might want again later — yes. The pivot moment is the first time Klipo saves you 20 minutes by remembering something you'd otherwise have to re-find. That moment usually happens on day 1.

**Will my clipboard sync across devices?**
Not today. End-to-end encrypted sync is being explored for a future release, but it isn't built yet — there is no buying claim or timeline attached. Until then, each device has its own local history.

**Can I see the source code before buying?**
v0.1.0–v0.1.2 are still public on GitHub under Apache-2.0 — you can audit the architecture, sensitive-content patterns, and storage schema there. v0.1.3+ is private commercial source, but the architecture/security/perf-budget docs are linked in every release.

---

## Cover image (Gumroad listing card)

Use `assets/cover-1280x720.png` (TODO: produce — derive from `assets/hero.gif` first frame, add the headline above on dark background, with the bluedev wordmark in the bottom-right).

## Thumbnail

Use `assets/gumroad-thumbnail.png` (TODO: 600×600, app icon over a colored gradient).

---

## Categories / tags

- Software
- Productivity
- Windows app
- Developer tools
- Clipboard manager

---

## License (one-liner for Gumroad's "License" field)

Per-seat commercial license. Up to 3 devices, lifetime updates within v0.x. Full terms in the [EULA](https://github.com/aydogandagidir/klipo/blob/main/LEGAL/EULA.md).
