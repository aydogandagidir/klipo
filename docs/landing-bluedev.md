# Klipo — Landing Page Copy for bluedev.dev

> Drop into `bluedev.dev/products/klipo` (or wherever the product subpage lives).
> Designed to fit a hero + 3-feature grid + screenshots + FAQ + CTA layout,
> consistent with the bluedev brand voice (professional, AI-automation focused, B2B-friendly).

---

## SEO / meta

```html
<title>Klipo — keyboard-first clipboard manager for Windows | bluedev</title>
<meta name="description" content="Klipo captures every clipboard event you make and lets you find any of them in milliseconds. Local, private, keyboard-first. Built by bluedev." />
<meta property="og:title" content="Klipo — never lose a snippet again" />
<meta property="og:description" content="A clipboard manager that respects your time and your data. From bluedev." />
<meta property="og:image" content="https://bluedev.dev/assets/klipo-og-image.png" />
<meta property="og:url" content="https://bluedev.dev/products/klipo" />
<meta name="twitter:card" content="summary_large_image" />
```

---

## Hero

> **Never lose a snippet again.**
>
> Klipo is a keyboard-first clipboard manager that captures every `Ctrl+C` you make and lets you find it again in milliseconds. Local. Private. Fast.

**Primary CTA:** `Buy on Gumroad — $29`
**Secondary CTA:** `Read the docs`

Hero asset: `assets/hero.gif` (already produced).

---

## Why Klipo

The clipboard is one of the most-used and least-respected pieces of software on your computer. You copy hundreds of things a day; the OS remembers exactly one. Klipo fixes that — without sending a byte to the cloud, without auto-pasting, without making you click through anything.

> Built by **bluedev** — the same software & AI-automation studio behind every other tool you'll find on this site. Same engineering bar: fast, secure, opinionated.

---

## Three things it does well

### ⌨️ Capture everything, instantly

Text, images, files, RTF, HTML — everything you copy is queued into a local SQLite database with hash-based deduplication. No more "wait, where did I copy that link from?"

### 🔍 Find anything in <50 ms

SQLite FTS5 with BM25 ranking and Türkçe character folding. 10,000 items? No problem. Type a fragment, hit Enter, paste it back into the app you came from.

### 🛡️ Keep secrets secret

Detect API keys, credit cards, JWTs, and 10 more patterns automatically. Sensitive clips get a red border, blurred preview, and a paste-confirm dialog — so you never accidentally drop a token into a public Slack channel.

---

## What you see

| Captured clipboard | Sensitive content guarded | Polished settings |
|:---:|:---:|:---:|
| ![](assets/screenshots/popup.png) | ![](assets/screenshots/popup-sensitive.png) | ![](assets/screenshots/settings.png) |
| `Ctrl+Alt+V` from any app summons the popup. | API keys & secrets are visually flagged. | Theme, hotkey rebind, autostart, signed auto-updates. |

---

## Built right

- **Native Windows app** built on Tauri 2 + Rust. ~3.8 MB installer. No Electron tax.
- **Local-first.** Encrypted SQLite under `%APPDATA%\Klipo\`. Nothing leaves your machine unless you opt in.
- **Signed installer.** Authenticode + Tauri-signed updater. No SmartScreen drama.
- **Keyboard-first UI.** Every action has a shortcut. Mouse is optional.
- **Open architecture.** Detailed perf budget and security docs published on GitHub.

---

## Pricing

| Tier | What you get | Price |
|:---|:---|:---|
| **Personal** | Klipo for up to 3 devices you own. Lifetime v0.x updates. macOS upgrade free when v0.2 ships. | **$29** |
| **Team** | Same, on N seats. Centralized license management. Priority email support. | **Contact us** |

> 30-day refund. Per-seat license. [Read the EULA](https://github.com/aydogandagidir/klipo/blob/main/LEGAL/EULA.md).

**Buy on Gumroad →** `https://gumroad.com/l/klipo` _(TODO: link once listed)_

---

## FAQ

**Is Klipo really only Windows today?**
Yes — Windows 10 (1809+) and Windows 11. macOS is the next milestone (v0.2); your purchase includes that upgrade free.

**Does it phone home?**
Only if you opt into anonymous usage telemetry in Settings → Privacy. Off by default. Clipboard contents are **never** transmitted.

**Can I export my history?**
Yes. Settings → Data → Export to JSON / Markdown.

**Will it sync across devices?**
End-to-end encrypted sync is on the v0.3 roadmap. Today, each device has its own local history.

**Is the source open?**
v0.1.0–0.1.2 are Apache-2.0 (historical). v0.1.3+ is a commercial product under a proprietary EULA. Architecture and changelog docs remain public for transparency.

---

## About bluedev

bluedev builds software and AI-driven automation tools for productivity-minded developers and small teams. We ship products we'd want to use ourselves and price them so you don't have to think about it twice.

**More from bluedev →** `https://bluedev.dev`

---

## CTA footer

> **Try it. Refund it within 30 days if it isn't right for you.**
>
> [Buy on Gumroad — $29](https://gumroad.com/l/klipo) · [Docs](https://github.com/aydogandagidir/klipo) · [Support](mailto:support@bluedev.dev)

---

## Page-build TODOs (for whoever implements the page)

- [ ] Replace `$29` with the chosen price (recommend $19–29 for Personal at launch).
- [ ] Replace `https://gumroad.com/l/klipo` with the real Gumroad listing URL once published.
- [ ] Place `assets/hero.gif` above the fold; lazy-load below-fold screenshots.
- [ ] Add the bluedev navbar/footer for brand consistency.
- [ ] Inline a small download counter once Gumroad starts reporting sales (trust signal).
- [ ] Wire up email-capture form ("Notify me when macOS ships") to bluedev's mailing list.
- [ ] Optimize images: serve WebP with PNG fallback; produce 1× and 2× srcset for screenshots.
- [ ] Verify `og:image` displays correctly on Twitter/X, LinkedIn, Slack, Discord (use https://cards-dev.twitter.com/validator).
- [ ] Lighthouse performance ≥ 90, accessibility ≥ 95.
