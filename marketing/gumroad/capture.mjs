// Gumroad marketing capture pipeline for Klipo.
//
// Stage 1 — screenshot every static graphic (cover, thumbnail, comparison, hero).
// Stage 2 — record each video scene as WebM at its declared duration.
// Stage 3 — concat WebM scenes into a single MP4 (H.264) via ffmpeg.
//
// Usage: node marketing/gumroad/capture.mjs
//
// Outputs end up in marketing/gumroad/out/.

import { chromium } from "playwright";
import { promises as fs } from "node:fs";
import path from "node:path";
import url from "node:url";
import { spawn } from "node:child_process";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const HERE = __dirname;
const OUT  = path.join(HERE, "out");
const SCENES_DIR = path.join(HERE, "scenes");

// Allow overriding ffmpeg path via env (Windows users often have it outside PATH).
const FFMPEG = process.env.FFMPEG_PATH || "ffmpeg";

// 1280×720 — Gumroad cover ratio (also our video frame).
const SIZE_WIDE = { width: 1280, height: 720 };
const SIZE_SQ   = { width: 600,  height: 600 };

const STATIC_TARGETS = [
  { name: "01-cover.png",      file: "cover.html",      size: SIZE_WIDE, settle: 1500 },
  { name: "02-thumbnail.png",  file: "thumbnail.html",  size: SIZE_SQ,   settle: 1200 },
  { name: "03-hero.png",       file: "hero.html",       size: SIZE_WIDE, settle: 1500 },
  { name: "04-comparison.png", file: "comparison.html", size: SIZE_WIDE, settle: 1200 },
];

const VIDEO_SCENES = [
  "01-brand-intro.html",
  "02-problem.html",
  "03-solution-reveal.html",
  "04-capture-demo.html",
  "05-search-demo.html",
  "06-sensitive-demo.html",
  "07-pricing.html",
  "08-cta.html",
];

async function ensureDir(p) { await fs.mkdir(p, { recursive: true }); }

function fileUrl(p) {
  return url.pathToFileURL(p).href;
}

function log(...args) { console.log(...args); }

async function captureScreenshots(browser) {
  log("\n[1/3] Capturing static graphics…");
  const ctx = await browser.newContext({
    viewport: SIZE_WIDE,
    deviceScaleFactor: 2,            // crisp on retina-ish displays
  });
  const page = await ctx.newPage();
  for (const t of STATIC_TARGETS) {
    await page.setViewportSize(t.size);
    const filePath = path.join(HERE, t.file);
    await page.goto(fileUrl(filePath), { waitUntil: "networkidle", timeout: 30_000 });
    // Wait for fonts to actually paint, then settle for animations.
    await page.evaluate(() => document.fonts ? document.fonts.ready : Promise.resolve());
    await page.waitForTimeout(t.settle);
    const out = path.join(OUT, t.name);
    await page.screenshot({ path: out, omitBackground: false });
    log(`   ✓ ${t.name}  (${t.size.width}×${t.size.height})`);
  }
  await ctx.close();
}

async function readSceneDuration(page) {
  return page.evaluate(() => {
    const el = document.body;
    const v  = el && el.dataset && el.dataset.duration;
    return v ? parseInt(v, 10) : 6000;
  });
}

async function captureVideoScenes(browser) {
  log("\n[2/3] Recording video scenes…");
  const tmpRoot = path.join(OUT, "_scenes");
  await ensureDir(tmpRoot);
  const sceneFiles = [];

  for (let i = 0; i < VIDEO_SCENES.length; i++) {
    const file = VIDEO_SCENES[i];
    const outDir = path.join(tmpRoot, `s${String(i + 1).padStart(2, "0")}`);
    await ensureDir(outDir);

    const ctx = await browser.newContext({
      viewport: SIZE_WIDE,
      recordVideo: { dir: outDir, size: SIZE_WIDE },
      deviceScaleFactor: 1,            // 1:1 for predictable video pixels
    });
    const page = await ctx.newPage();
    const filePath = path.join(SCENES_DIR, file);
    const t0 = Date.now();
    await page.goto(fileUrl(filePath), { waitUntil: "networkidle", timeout: 30_000 });
    await page.evaluate(() => document.fonts ? document.fonts.ready : Promise.resolve());
    const duration = await readSceneDuration(page);
    // Total wall-clock = duration of the scene.
    const elapsed = Date.now() - t0;
    const remaining = Math.max(0, duration - elapsed);
    await page.waitForTimeout(remaining);
    await page.close();
    await ctx.close();
    // Find the recorded WebM (Playwright writes a single .webm in the dir).
    const list = await fs.readdir(outDir);
    const webm = list.find((f) => f.endsWith(".webm"));
    if (!webm) throw new Error(`No webm recorded for ${file} in ${outDir}`);
    const webmPath = path.join(outDir, webm);
    sceneFiles.push({ file, webmPath, duration });
    log(`   ✓ scene ${String(i + 1).padStart(2, "0")} · ${file}  (${duration}ms)`);
  }
  return sceneFiles;
}

function runFfmpeg(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(FFMPEG, args, { stdio: ["ignore", "pipe", "pipe"] });
    let err = "";
    child.stderr.on("data", (d) => { err += d.toString(); });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`ffmpeg exited ${code}\n${err.slice(-2000)}`));
    });
  });
}

async function buildFinalMp4(sceneFiles) {
  log("\n[3/3] Stitching scenes with ffmpeg…");

  // Per-scene: re-encode WebM → MP4 (H.264) at exact duration. This normalizes
  // codec/fps so the concat demuxer can stitch without artefacts.
  const mp4Parts = [];
  for (let i = 0; i < sceneFiles.length; i++) {
    const { webmPath, duration } = sceneFiles[i];
    const mp4Path = path.join(OUT, "_scenes", `s${String(i + 1).padStart(2, "0")}.mp4`);
    const args = [
      "-y",
      "-i", webmPath,
      "-t", (duration / 1000).toFixed(3),
      // White letterbox if a scene is ever shorter than 1280×720 — matches our
      // white-first canvas. (Was 0x0A1628 navy, which leaked navy edges on
      // scenes whose Playwright recording didn't perfectly fill the frame.)
      "-vf", `fps=30,scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2:color=0xFFFFFF,setsar=1`,
      "-c:v", "libx264",
      "-preset", "medium",
      "-crf", "20",
      "-pix_fmt", "yuv420p",
      "-movflags", "+faststart",
      "-an",                            // no audio track
      mp4Path,
    ];
    await runFfmpeg(args);
    mp4Parts.push(mp4Path);
    log(`   ✓ part ${String(i + 1).padStart(2, "0")} encoded`);
  }

  // Concat list (paths are forward-slashed for ffmpeg cross-platform).
  const listPath = path.join(OUT, "_scenes", "concat.txt");
  const listBody = mp4Parts.map((p) => `file '${p.replace(/\\/g, "/")}'`).join("\n");
  await fs.writeFile(listPath, listBody, "utf8");

  // Final concat — copy streams (already encoded uniformly above).
  const finalPath = path.join(OUT, "klipo-demo.mp4");
  await runFfmpeg([
    "-y",
    "-f", "concat",
    "-safe", "0",
    "-i", listPath,
    "-c", "copy",
    "-movflags", "+faststart",
    finalPath,
  ]);
  log(`   ✓ final video: ${finalPath}`);
  return finalPath;
}

(async () => {
  await ensureDir(OUT);
  log("Launching headless Chromium…");
  const browser = await chromium.launch({
    args: [
      "--font-render-hinting=none",
      "--force-color-profile=srgb",
      "--disable-web-security",
    ],
  });
  try {
    await captureScreenshots(browser);
    const scenes = await captureVideoScenes(browser);
    const out = await buildFinalMp4(scenes);
    log(`\n✓ Done. See ${OUT}`);
  } finally {
    await browser.close();
  }
})().catch((err) => {
  console.error("\n✗ Capture pipeline failed:", err);
  process.exit(1);
});
