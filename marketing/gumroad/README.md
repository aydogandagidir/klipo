# Gumroad Marketing Assets — Klipo

This directory contains the source pipeline + final assets for the
**Klipo** Gumroad listing. Klipo is a Windows clipboard manager by
**bluedev** — `$29` lifetime license, 14-day free trial.

## Final assets (in `out/`)

| File | Size | Purpose |
|---|---|---|
| `01-cover.png` | 1280×720 | Gumroad cover image |
| `02-thumbnail.png` | 600×600 | Custom square thumbnail |
| `03-hero.png` | 1280×720 | Gallery: 3-up superpowers showcase |
| `04-comparison.png` | 1280×720 | Gallery: Trial vs Pro pricing card |
| `klipo-demo.mp4` | 1280×720 · 30fps · ~80 s · H.264 | Demo video (no audio) |

These files are **committed to the repo** as historical snapshots of
what was uploaded to Gumroad. Regenerate them by running the pipeline
(see below).

## Source files

- `style.css` — shared brand system (colors, type, animations) — bluedev brand kit
- `cover.html`, `thumbnail.html`, `hero.html`, `comparison.html` — 4 static graphic sources
- `scenes/01-brand-intro.html` … `scenes/08-cta.html` — 8 video scenes (CSS-animated, auto-play)
- `capture.mjs` — Playwright + ffmpeg pipeline runner

## Regenerating the assets

Prerequisites: Node 22+, `npx playwright install chromium` once, ffmpeg
available either on `PATH` or via `FFMPEG_PATH` env var.

```bash
# From the repo root:
FFMPEG_PATH="C:/Users/adagidir/ffmpeg/ffmpeg-8.0.1-essentials_build/bin/ffmpeg.exe" \
  node marketing/gumroad/capture.mjs
```

Output lands in `marketing/gumroad/out/`. Total runtime ≈ 100 seconds
(8 video scenes × ~10 s + ffmpeg encode + 4 PNGs).

## Editing tips

- Want a different price (e.g. $19 or $39)? Search & replace `$29` in
  `cover.html`, `thumbnail.html`, `comparison.html`, `scenes/07-pricing.html`,
  and `scenes/08-cta.html`. Then re-run the pipeline.
- Want a different domain in the CTA? Edit the URL in
  `scenes/08-cta.html` (currently `bluedev.dev/products/klipo`).
- Want to swap a screenshot for a real captured one? Drop it under
  `assets/screenshots/` in the repo root and reference it from a scene
  via `<img src="../../../assets/screenshots/yourfile.png">`.
- Want to change the trial length (14 days)? Search for "14-day" /
  "14 days" across the directory.

## Storyboard (video)

| # | Scene | Duration |
|---|---|---|
| 01 | Brand intro — bluedev → Klipo wordmark | 4.5 s |
| 02 | Problem — "what did I copy 5 minutes ago?" | 7.0 s |
| 03 | Solution reveal — Klipo popup + 3 pillars | 6.5 s |
| 04 | Capture demo — text · image · file · code piling up | 11.0 s |
| 05 | Search demo — type "stripe", filter, paste back | 11.0 s |
| 06 | Sensitive demo — sk-proj key flagged with red border | 11.0 s |
| 07 | Pricing — $29 lifetime + 14-day trial | 11.0 s |
| 08 | CTA — bluedev.dev/products/klipo + Gumroad button | 6.5 s |
| **Total** | | **~78.5 s** |

## Notes

- `out/_scenes/` is gitignored — those are intermediate WebM/MP4 parts
  produced during the ffmpeg concat step (~30 MB). Pipeline regenerates
  them; no need to keep them in git.
- The 4 PNGs and the final MP4 are committed because they're what got
  uploaded to Gumroad. If you re-record, replace them and commit the
  new snapshot — diffs in PRs let reviewers see visual changes.
- Klipo screenshots (`popup.png`, `popup-sensitive.png`, `settings.png`)
  live in the repo root at `assets/screenshots/`. The hero composition
  references them by relative path; if you move them, update the
  `<img src="…">` tags in `hero.html` and the relevant scene files.
