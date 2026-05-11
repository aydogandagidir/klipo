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
// 12% inner padding so the mark doesn't bleed to the canvas edges; matches
// macOS/iOS rounded-rect icon mask insets.
const PAD_RATIO = 0.12;

const svgRaw = await fs.readFile(SVG_PATH, "utf8");

// Strip the outer XML declaration + comments so the SVG embeds cleanly inside
// an HTML host page. Keep the actual <svg> tag and its children verbatim.
const svgInner = svgRaw
  .replace(/<\?xml[^?]*\?>\s*/g, "")
  .replace(/<!--[\s\S]*?-->\s*/g, "")
  .trim();

const html = `<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<style>
  html, body { margin: 0; padding: 0; width: ${SIZE}px; height: ${SIZE}px; background: transparent; }
  body {
    display: flex; align-items: center; justify-content: center;
  }
  .icon-wrap {
    width: ${SIZE - 2 * SIZE * PAD_RATIO}px;
    height: ${SIZE - 2 * SIZE * PAD_RATIO}px;
    color: #015AFF;
    display: flex; align-items: center; justify-content: center;
  }
  .icon-wrap svg {
    width: 100%; height: 100%;
  }
</style>
</head>
<body>
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
  await page.screenshot({ path: OUT_PATH, omitBackground: true, type: "png" });
  console.log(`✓ wrote ${OUT_PATH} (${SIZE}×${SIZE}, transparent bg)`);
} finally {
  await browser.close();
}
