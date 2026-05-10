# Klipo Demo Video — Shotlist

> Shot-by-shot recording guide for the ~2-minute Gumroad / bluedev.dev demo.
> Pairs with [`docs/demo-video-script.md`](./demo-video-script.md) — same
> timestamps. Recorder uses this; voice actor uses the script.
>
> **Recording tools:** OBS Studio (free) or Camtasia. 1920×1080 @ 30 fps,
> 30 Mbps minimum. Record system audio off (voiceover layered in post).
>
> **Pre-flight setup:** clean desktop, only the apps in the script open,
> taskbar set to auto-hide, notifications off (Focus Assist on), Klipo
> popup theme set to **dark** (looks better on demos), DB pre-populated
> with the sample clips listed below.

---

## Pre-recording — DB seed

Before recording, copy these clips in order so they appear in the popup
during the demo (Klipo will rank them by recency):

```
1.  https://stripe.com/docs/api
2.  Aydoğan
3.  https://github.com/aydogandagidir/klipo
4.  function calculatePrice(qty: number) { return qty * 29; }
5.  C:\Users\demo\Documents\report-q4.pdf
6.  Hello, this is the meeting note from yesterday's standup...
7.  npm install @tanstack/react-virtual
8.  sk-proj-FAKEEXAMPLE12345abcdef-NOT_A_REAL_KEY_zzz999  (will be flagged)
9.  10.0.0.42
10. (an image — paste a screenshot from Snipping Tool)
```

The fake `sk-proj-` key triggers the sensitive-content guard and is the
hero of the 0:50–1:10 segment.

---

## 0:00–0:10 — Hook

| Shot | Duration | Visual | Notes |
|------|----------|--------|-------|
| 1 | 0:00–0:03 | **Black screen** with a single line of white text: "How many things did you copy today?" | Set the question. Single line, generous padding. |
| 2 | 0:03–0:07 | **Cut to user's desktop** — Notepad open, browser open with a code-snippet article, a Slack window. Cursor selects text in the article and `Ctrl+C`. | Real-feel, slightly fast cuts. |
| 3 | 0:07–0:10 | Switch to Notepad, `Ctrl+V` — but a *different* string appears (because they copied something else after). User pulls a confused face (cursor zigzags). | The "I lost it!" beat. |

---

## 0:10–0:30 — Capture demo

| Shot | Duration | Visual | Notes |
|------|----------|--------|-------|
| 4 | 0:10–0:14 | Klipo tray icon zoom-in (magnified circle highlight) | Establishes Klipo is *already running*. |
| 5 | 0:14–0:22 | Rapid sequence: copy text from VS Code → switch to browser → copy URL → switch to Slack → copy a chat message → switch to File Explorer → copy a PDF file. **Each `Ctrl+C` triggers a brief flash on the tray icon.** | 4 captures in 8 seconds. Fast but not frantic. |
| 6 | 0:22–0:26 | Slow camera-pan to the SQLite database file on disk: `%APPDATA%\app.klipo.desktop\klipo.db` | "Locally. In an encrypted SQLite database." Pair with VO. |
| 7 | 0:26–0:30 | Cross-out animation over a stylized "cloud" icon | "Nothing goes to the cloud." Visual confirmation of privacy claim. |

---

## 0:30–0:50 — Search demo

| Shot | Duration | Visual | Notes |
|------|----------|--------|-------|
| 8 | 0:30–0:33 | User in Notepad. Press `Ctrl+Alt+V`. **Klipo popup fades in over Notepad** (transparent, on top). | Use Klipo's actual `popup-in` animation (120 ms ease-out). |
| 9 | 0:33–0:40 | Type "stripe" letter-by-letter. Results filter in real time, narrowing from 10 clips → 3 → 1. | Show the FTS5 ranking visibly. |
| 10 | 0:40–0:45 | Highlight the result row (selected state, primary border). Press `Enter`. | The pivotal beat. |
| 11 | 0:45–0:50 | Klipo popup vanishes. Notepad now has the Stripe URL pasted. Cursor blinks at end of pasted text. | The "it just works" moment. |

---

## 0:50–1:10 — Sensitive content protection

| Shot | Duration | Visual | Notes |
|------|----------|--------|-------|
| 12 | 0:50–0:54 | Switch to a code editor. Highlight the fake `sk-proj-FAKEEXAMPLE12345abcdef-NOT_A_REAL_KEY_zzz999` string. `Ctrl+C`. | Make sure the key is clearly fake. |
| 13 | 0:54–0:58 | `Ctrl+Alt+V` → popup opens. **Top item is the just-copied key, with red left border + blurred preview + "SENSITIVE" badge in red.** | The product hero shot. Hold this shot for 3 seconds — let viewers process the affordance. |
| 14 | 0:58–1:04 | Hover over the row → blur fades, preview becomes readable for inspection. Then move cursor away → blur returns. | Show the hover-to-reveal mechanic. |
| 15 | 1:04–1:10 | Press `Enter`. **Confirmation dialog**: "Paste sensitive clip?" with Confirm / Cancel buttons. User clicks Cancel. Popup stays. | The double-check that prevents accidents. |

---

## 1:10–1:30 — Settings tour

| Shot | Duration | Visual | Notes |
|------|----------|--------|-------|
| 16 | 1:10–1:14 | Settings opens (gear icon click). Land on **General** tab. | Show window chrome — looks like a normal app. |
| 17 | 1:14–1:18 | Theme picker: click Dark → Light → System. UI colors flip live. History-limit input shows 10,000. | Quick interaction, builds confidence. |
| 18 | 1:18–1:22 | Click hotkey field → press `Ctrl+Shift+Space`. Field updates. | Shows hotkey rebind works. |
| 19 | 1:22–1:30 | Switch to **Privacy** tab. Land on the **Re-scan history** row. Click the button. Toast appears: "Scanned 358 clips: 0 newly flagged, 358 unchanged." | The new v0.1.3 feature spotlight. Hold the toast on screen for 3 seconds. |

---

## 1:30–1:50 — Privacy pitch

| Shot | Duration | Visual | Notes |
|------|----------|--------|-------|
| 20 | 1:30–1:36 | Slow zoom out from the SQLite database file in the user's `%APPDATA%\app.klipo.desktop\` folder. | Reinforce locality. |
| 21 | 1:36–1:42 | Settings → Privacy: Telemetry toggle highlighted, **OFF**. Cursor circles it but doesn't click. | "Telemetry off by default and opt-in only." |
| 22 | 1:42–1:50 | A grid of three icons appears: 🔒 Local · 🚫 No account · 💯 Lifetime. Each icon enters with a quick fade-in, ~200 ms apart. | Reinforce three pillars. |

---

## 1:50–2:00 — CTA close

| Shot | Duration | Visual | Notes |
|------|----------|--------|-------|
| 23 | 1:50–1:54 | **Center-frame**: Klipo logo (left) + "by bluedev" wordmark (right). Off-white background, no other clutter. | Clean brand close. |
| 24 | 1:54–1:58 | **Text overlay** appears below logo: "$29 — lifetime v0.x updates · macOS in v0.2" | Big enough to read mobile. |
| 25 | 1:58–2:00 | Mouse cursor enters frame, hovers over a stylized "Buy on Gumroad" button (the actual button styling matches the bluedev.dev landing page). Cuts to bluedev.dev hero on click. | Soft handoff to the landing page. |

---

## Post-production checklist

- [ ] Cut shots to exact timestamps; allow ±200 ms drift between visuals and VO.
- [ ] Color-grade for consistency (slight desaturation, lift shadows, daily-driver feel).
- [ ] Add subtle keystroke SFX for each `Ctrl+C` / `Ctrl+V` (low volume, -18 dB under VO).
- [ ] Background music ducks under VO automatically (sidechain compression).
- [ ] Master audio at -16 LUFS for web playback.
- [ ] Export 1080p H.264 MP4 (Gumroad + Twitter).
- [ ] Export WebM VP9 (bluedev.dev `<video>` element with MP4 fallback).
- [ ] Captions/subtitles burned-in version + separate `.vtt` file (accessibility + autoplay-muted social previews).

## Asset list to prepare before recording

- [ ] Klipo wordmark SVG (already in `/assets/icons` — check for PNG variant)
- [ ] bluedev wordmark (request from user if not in repo)
- [ ] Gumroad-branded "Buy" button mockup
- [ ] Three pillar icons (lock, no-account, lifetime) — Lucide icons exported as SVG
- [ ] Background music track licensed
- [ ] Voiceover recording (separate WAV file)
- [ ] Closed-caption transcript matching script word-for-word
