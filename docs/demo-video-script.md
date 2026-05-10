# Klipo Demo Video — Script (English, ~2 min)

> Voice-over script for the Gumroad listing + bluedev.dev landing page hero
> video. Tone: professional but conversational, human-like, not corporate.
> Pace: ~145 words/min for natural delivery; total ~290 words / 2:00.
>
> **Pairs with** [`docs/demo-video-shotlist.md`](./demo-video-shotlist.md) —
> the shotlist tells the recorder *what to show*, this tells the voice
> actor *what to say*. Timestamps match.

---

## 0:00–0:10 — Hook

> *(black screen → fade in to user's desktop with multiple tabs / app windows open)*

**Voice-over:**
"Ever copied something — a code snippet, a link, an address — and lost it
the next time you hit Ctrl+C? Of course you have. Everyone has. Here's how
to never lose another."

---

## 0:10–0:30 — Capture demo

> *(screen recording: rapid copy actions across apps; small toast or visual
> beat each time something is captured)*

**Voice-over:**
"This is Klipo. It runs quietly in your Windows tray. Every time you copy
*anything* — text, code, an image, a file, even rich text from your inbox —
Klipo remembers it. Locally. In an encrypted SQLite database on your
machine. Nothing goes to the cloud."

---

## 0:30–0:50 — Search demo

> *(press Ctrl+Alt+V → popup opens → type "stripe" → instant results → Enter)*

**Voice-over:**
"Press Ctrl+Alt+V from any app, and Klipo's popup floats in. Type a few
characters — Klipo searches your entire history with full-text indexing,
ranked by relevance, in under fifty milliseconds. Hit Enter. The clip
pastes back into the app you were just using. Every action has a
shortcut. Mouse is optional."

---

## 0:50–1:10 — Sensitive content protection

> *(copy a fake `sk-proj-…` API key string → popup shows red border + SENSITIVE
> badge + blurred preview)*

**Voice-over:**
"Klipo also notices when you've copied something sensitive — API keys,
credit cards, JWTs, private keys. Thirteen patterns out of the box. The
clip gets a red border, a blurred preview, and a confirm-before-paste
dialog. So you don't accidentally drop a production token into the wrong
chat window."

---

## 1:10–1:30 — Settings tour

> *(open Settings → General → theme picker, hotkey rebind; jump to
> Privacy → Re-scan history button click → toast "Scanned 358 clips:
> 0 newly flagged, 358 unchanged.")*

**Voice-over:**
"Customize the hotkey, the theme, the history limit. Bind your own chord.
Run at login. And if Klipo updates its detection rules, one click
re-scans your existing history — without ever moving or deleting a
clip."

---

## 1:30–1:50 — Privacy pitch

> *(slow camera over: SQLite database file in the user's AppData folder, then
> the Settings → Privacy tab with "Telemetry: Off" highlighted)*

**Voice-over:**
"Klipo is local-first by design. Your clipboard data stays on your machine.
Telemetry is off by default and opt-in only. There's no account to create.
No subscription. No upsell modals. Just a clipboard manager that respects
your time and your data."

---

## 1:50–2:00 — CTA close

> *(Klipo logo + bluedev wordmark center frame; text overlay: "$29 — lifetime
> v0.x updates · macOS coming in v0.2"; mouse cursor moves to a "Buy on
> Gumroad" button)*

**Voice-over:**
"Klipo. By bluedev. Twenty-nine dollars, with lifetime updates in the
0.x series. macOS arrives in v0.2 — included free. Get it at
bluedev.dev/products/klipo."

> *(fade to bluedev.dev landing page hero)*

---

## Script-only word count: ~290 words

## Recording notes for the voice-over

- **Microphone:** condenser mic minimum (Blue Yeti / Shure MV7 / Rode NT-USB).
  Not a laptop mic.
- **Pace:** target 145 wpm. Slightly slower than conversation. Pause for half
  a beat between sentences ending in a period.
- **Tone reference:** Stripe documentation videos, Linear product tour,
  early Tailwind UI promo videos. *Not* enterprise SaaS narration.
- **Pronunciation:** "Klipo" rhymes with "tempo." "bluedev" is one word,
  lower-case b, second syllable stressed (`blue-DEV`).
- **Voice options:**
  - **Self-recorded** — most authentic, recommended for indie launch. Add
    light compression + de-noise in post.
  - **AI TTS** — ElevenLabs "Adam" or "Daniel" voices give professional-grade
    output if self-recording isn't an option. Disclose this in the launch
    post if asked.

## Music + sound design (optional)

- **Background music:** instrumental, low-mid tempo, ~70 BPM. Avoid drum-heavy
  tracks (they fight the voice). License-free options: Epidemic Sound
  "Productive" pack, Artlist "Soft Focus" pack.
- **SFX:** subtle keyboard click on each Ctrl+C action; soft chime when the
  popup appears. No loud transitions.
- **Mix:** voice-over -3 dB above music, normalize to -16 LUFS for web.

## Output formats needed

- **Gumroad:** MP4 H.264, 1920×1080, 30 fps, ≤ 50 MB. Direct upload.
- **bluedev.dev:** MP4 (same) + WebM (VP9) for `<video>` element with both sources.
- **Twitter / X:** 16:9 MP4, ≤ 2:20, ≤ 512 MB.
- **YouTube unlisted:** original 4K master if recorded at 4K, otherwise 1080p.
