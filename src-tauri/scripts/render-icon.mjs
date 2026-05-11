// Renders assets/klipo-mark.svg as a 1024×1024 PNG, suitable for feeding to
// `pnpm tauri icon`. We can't reach for ImageMagick/Inkscape/rsvg here, but
// Playwright is already installed for the marketing pipeline — so we use a
// headless Chromium viewport as the SVG renderer.
//
// Usage (from repo root):
//   node src-tauri/scripts/render-icon.mjs
//
// Output: src-tauri/icons/icon-source.png  (then run `pnpm tauri icon` on it)

// Playwright lives in the marketing-pipeline's isolated node_modules tree
// (marketing/gumroad/node_modules) — we don't want a second copy in src-tauri.
// Resolve it with an absolute file URL so this script can run from anywhere.
import { promises as fs } from "node:fs";
import path from "node:path";
import url from "node:url";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const SVG_PATH = path.join(REPO_ROOT, "assets", "klipo-mark.svg");
const OUT_PATH = path.join(REPO_ROOT, "src-tauri", "icons", "icon-source.png");
const PLAYWRIGHT_ENTRY = url.pathToFileURL(
  path.join(REPO_ROOT, "marketing", "gumroad", "node_modules", "playwright", "index.mjs"),
).href;

const { chromium } = await import(PLAYWRIGHT_ENTRY);

const SIZE = 1024;
// Inner padding for the mark inside the colored squircle background. We keep
// roughly 22% inset (per side) so the glyph still reads clearly inside the
// Windows / macOS / iOS icon mask, which trims a few percent off the edges
// of the canvas. With a solid brand-blue background filling the canvas,
// the mark sits in the center as the high-contrast element.
const PAD_RATIO = 0.22;
// Corner radius (as a fraction of the full canvas) for the brand-blue
// "squircle" backplate. ~22% matches Windows 11's rounded-icon visual
// language without going full-circle. Per-OS icon masks override this in
// practice (Android adaptive, macOS squircle, iOS rounded square) but a
// neutral 22% reads correctly under all of them.
const CORNER_RATIO = 0.22;
// Brand tokens — keep in lockstep with marketing/gumroad/style.css.
const BRAND_BLUE = "#015AFF";
const GLYPH_COLOR = "#FFFFFF";

const svgRaw = await fs.readFile(SVG_PATH, "utf8");

// Strip the outer XML declaration + comments so the SVG embeds cleanly inside
// an HTML host page. Keep the actual <svg> tag and its children verbatim.
const svgInner = svgRaw
  .replace(/<\?xml[^?]*\?>\s*/g, "")
  .replace(/<!--[\s\S]*?-->\s*/g, "")
  .trim();

// Composition strategy: a solid bluedev-blue squircle fills the entire
// canvas, then the Klipo mark sits centered in white. This gives the icon
// a guaranteed-high-contrast read on every taskbar / dock background a
// user might have (dark Windows 11, light macOS, OS-themed Linux). The
// previous transparent-bg + brand-blue-glyph approach was readable on
// light surfaces but vanished into dark Windows taskbars — confirmed by
// user feedback on v0.1.5.
const html = `<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<style>
  html, body { margin: 0; padding: 0; width: ${SIZE}px; height: ${SIZE}px; background: transparent; }
  body {
    display: flex; align-items: center; justify-content: center;
    position: relative;
  }
  .icon-bg {
    position: absolute;
    inset: 0;
    background: ${BRAND_BLUE};
    border-radius: ${SIZE * CORNER_RATIO}px;
  }
  .icon-wrap {
    position: relative;
    z-index: 1;
    width: ${SIZE - 2 * SIZE * PAD_RATIO}px;
    height: ${SIZE - 2 * SIZE * PAD_RATIO}px;
    color: ${GLYPH_COLOR};
    display: flex; align-items: center; justify-content: center;
  }
  .icon-wrap svg {
    width: 100%; height: 100%;
  }
  /* Force the SVG's filled clip rect (the clipboard "clip" rectangle at the
     top of the mark, which the source SVG paints with currentColor by
     setting fill="currentColor") to inherit white instead of the brand
     accent it falls back to when no parent color is set. */
  .icon-wrap svg [fill="currentColor"] { fill: ${GLYPH_COLOR}; }
</style>
</head>
<body>
  <div class="icon-bg"></div>
  <div class="icon-wrap">${svgInner}</div>
</body>
</html>`;

const browser = await chromium.launch({
  args: ["--font-render-hinting=none", "--force-color-profile=srgb"],
});
try {
  const ctx = await browser.newContext({
    viewport: { width: SIZE, height: SIZE },
    deviceScaleFactor: 1,
  });
  const page = await ctx.newPage();
  await page.setContent(html, { waitUntil: "load" });
  await page.waitForTimeout(120); // settle paint
  // omitBackground:true so the rounded-corner edges of the squircle stay
  // transparent (no white halo around the bluedev-blue tile). The fill of
  // the squircle itself is opaque BRAND_BLUE, painted in CSS above.
  await page.screenshot({ path: OUT_PATH, omitBackground: true, type: "png" });
  console.log(`✓ wrote ${OUT_PATH} (${SIZE}×${SIZE}, ${BRAND_BLUE} squircle + white glyph)`);
} finally {
  await browser.close();
}
