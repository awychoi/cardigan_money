// Builds original C with three green tile backgrounds, plus a comparison board.
//   CHROMIUM_PATH=... node docs/design/brand/build-green.mjs
import { chromium } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const icons = resolve(root, "icons");
const preview = resolve(root, "preview");
const base = readFileSync(resolve(icons, "03-button-coin.svg"), "utf8");

const greens = [
  {
    id: "forest",
    name: "Forest",
    note: "Deep brand green",
    stops: ["#3b7a6c", "#20564b", "#0d2b25"],
    deco: "#d7f2e8",
    decoOp: "0.05",
    glow: "#7fc2b0",
    glowOp: "0.06",
    border: "#08211c",
    borderOp: "0.65",
    innerOp: "0.1",
    swatches: ["#20564b", "#7fc2b0", "#b26a4c"],
  },
  {
    id: "deep",
    name: "Deep",
    note: "Closest to Knit Graph",
    stops: ["#2a6155", "#153f36", "#081f1b"],
    deco: "#bfe8dc",
    decoOp: "0.04",
    glow: "#7fc2b0",
    glowOp: "0.05",
    border: "#051713",
    borderOp: "0.7",
    innerOp: "0.08",
    swatches: ["#153f36", "#7fc2b0", "#b26a4c"],
  },
  {
    id: "sage",
    name: "Sage",
    note: "Lightest, biggest contrast",
    stops: ["#a7ddcf", "#73bca9", "#3a8576"],
    deco: "#0e463b",
    decoOp: "0.05",
    glow: "#b26a4c",
    glowOp: "0.05",
    border: "#2b6558",
    borderOp: "0.55",
    innerOp: "0.3",
    swatches: ["#73bca9", "#f2e7d5", "#b26a4c"],
  },
];

const files = new Map(
  greens.map((g, i) => [g.id, `${19 + i}-c-green-${g.id}.svg`]),
);

for (const g of greens) {
  let svg = base
    .replace(
      /<radialGradient id="bc-bg"[\s\S]*?<\/radialGradient>/,
      `<radialGradient id="bc-bg" cx="0.28" cy="0.18" r="0.95">
      <stop offset="0" stop-color="${g.stops[0]}"/>
      <stop offset="0.55" stop-color="${g.stops[1]}"/>
      <stop offset="1" stop-color="${g.stops[2]}"/>
    </radialGradient>`,
    )
    .replace(
      '<g stroke="#ffffff" stroke-width="2" opacity="0.035">',
      `<g stroke="${g.deco}" stroke-width="2" opacity="${g.decoOp}">`,
    )
    .replace(
      '<circle cx="512" cy="500" r="360" fill="#b26a4c" opacity="0.05"/>',
      `<circle cx="512" cy="500" r="360" fill="${g.glow}" opacity="${g.glowOp}"/>`,
    )
    .replace(
      '<rect x="101" y="101" width="822" height="822" rx="183" fill="none" stroke="#000000" stroke-width="2" opacity="0.45"/>',
      `<rect x="101" y="101" width="822" height="822" rx="183" fill="none" stroke="${g.border}" stroke-width="2" opacity="${g.borderOp}"/>`,
    )
    .replace(
      '<rect x="112" y="112" width="800" height="800" rx="174" fill="none" stroke="#ffffff" stroke-width="2" opacity="0.08"/>',
      `<rect x="112" y="112" width="800" height="800" rx="174" fill="none" stroke="#ffffff" stroke-width="2" opacity="${g.innerOp}"/>`,
    )
    .replace(
      'aria-label="Button coin app icon concept"',
      `aria-label="Button coin app icon, ${g.name} green background"`,
    );

  if (!svg.includes(g.stops[0])) throw new Error(`background not replaced for ${g.id}`);
  const file = files.get(g.id);
  writeFileSync(resolve(icons, file), svg);
  console.log("wrote", file);
}

const uri = (f) => `data:image/svg+xml;base64,${readFileSync(resolve(icons, f)).toString("base64")}`;

const cells = [
  {
    file: "03-button-coin.svg",
    name: "Original",
    note: "Graphite · baseline",
    kind: "baseline",
    swatches: ["#232c33", "#b26a4c", "#f2e7d5"],
  },
  ...greens.map((g) => ({
    file: files.get(g.id),
    name: g.name,
    note: g.note,
    kind: "green",
    swatches: g.swatches,
  })),
];

const board = `<!doctype html><html><head><meta charset="utf-8"><style>
  *{box-sizing:border-box;margin:0;padding:0}
  body{width:1600px;background:#12171c;color:#edf0f4;padding:54px 56px 40px;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",sans-serif}
  header{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:1px solid #2a323b;padding-bottom:24px;margin-bottom:28px}
  .eyebrow{font:600 10.5px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.19em;color:#7fc2b0;text-transform:uppercase}
  h1{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:48px;line-height:1.05;letter-spacing:-.028em;margin-top:11px}
  h1 em{color:#7fc2b0;font-style:italic}
  .lede{max-width:410px;text-align:right;color:#97a2b1;font-size:13px;line-height:1.55}
  .lede strong{color:#edf0f4;font-weight:600}
  .grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:15px}
  .cell{background:#181f25;border:1px solid #2a333c;border-radius:18px;padding:13px 13px 12px;display:flex;flex-direction:column}
  .cell.baseline{background:#151b21;border-color:#242c34}
  .stage{background:radial-gradient(circle at 28% 18%,#29363f 0,#1b242b 58%,#151b21 100%);border:1px solid #2b353e;border-radius:12px;display:grid;place-items:center;padding:18px 8px;min-height:252px}
  .stage .icon{filter:drop-shadow(0 18px 20px rgba(0,0,0,.45))}
  .meta{margin-top:13px}
  .line{display:flex;align-items:center;justify-content:space-between;gap:8px}
  h2{font-size:14px;font-weight:650;letter-spacing:-.01em;color:#edf0f4}
  .pin{font:600 7.5px/1 ui-monospace,Menlo,monospace;letter-spacing:.12em;text-transform:uppercase;color:#97a2b1;border:1px solid #303a44;border-radius:999px;padding:4px 6px}
  .meta p{font-size:11.5px;color:#8b96a5;margin-top:4px}
  .swatches{display:flex;gap:5px;margin-top:9px}
  .swatches i{width:13px;height:13px;border-radius:3px;border:1px solid rgba(255,255,255,.12)}
  .small{display:flex;align-items:center;gap:8px;margin-top:11px;padding-top:10px;border-top:1px solid #262e36}
  .small span{font:500 8px/1 ui-monospace,Menlo,monospace;letter-spacing:.09em;color:#6f7a88;text-transform:uppercase}
  .lockup-strip{display:flex;align-items:center;gap:24px;flex-wrap:wrap;margin-top:18px;padding:15px 18px;background:#1a2229;border:1px solid #2a333c;border-radius:16px}
  .strip-label{font:600 8.5px/1.4 ui-monospace,Menlo,monospace;letter-spacing:.15em;text-transform:uppercase;color:#7fc2b0;margin-right:4px}
  .lockup{display:flex;align-items:center;gap:8px}
  .lockup img{filter:drop-shadow(0 3px 4px rgba(0,0,0,.4))}
  .lockup span{font:500 11px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.04em;color:#aeb8c5}
  footer{display:flex;justify-content:space-between;margin-top:22px;font:500 9.5px/1.5 ui-monospace,Menlo,monospace;letter-spacing:.06em;color:#6f7a88}
</style></head><body>
<header>
  <div><div class="eyebrow">cardigan_money · C color test</div><h1>Original C, <em>three greens.</em></h1></div>
  <div><p class="lede"><strong>The button, rim and thread are untouched.</strong> Only the tile changes. Original graphite is on the left; all four are shown at 16 px.</p></div>
</header>
<div class="grid">
  ${cells.map((c) => `
    <article class="cell ${c.kind === "baseline" ? "baseline" : ""}">
      <div class="stage"><img class="icon" src="${uri(c.file)}" width="240" height="240"></div>
      <div class="meta">
        <div class="line"><h2>${c.name}</h2>${c.kind === "baseline" ? '<span class="pin">Original</span>' : ""}</div>
        <p>${c.note}</p>
        <div class="swatches">${c.swatches.map((s) => `<i style="background:${s}"></i>`).join("")}</div>
      </div>
      <div class="small"><img src="${uri(c.file)}" width="16" height="16"><span>16 px</span></div>
    </article>`).join("")}
</div>
<div class="lockup-strip">
  <span class="strip-label">In the Knit Graph lockup</span>
  ${cells.map((c) => `<div class="lockup"><img src="${uri(c.file)}" width="30" height="30"><span>cardigan_money</span></div>`).join("")}
</div>
<footer><span>drafts · 16 px shown at actual size</span><span>Knit Graph · graphite #14171C · mint #7FC2B0 · rust #D79B7D</span></footer>
</body></html>`;

const browser = await chromium.launch(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {});
const page = await browser.newPage({ viewport: { width: 1600, height: 800 }, deviceScaleFactor: 2 });
await page.setContent(board, { waitUntil: "networkidle" });
await page.waitForFunction(() => [...document.images].every((i) => i.complete && i.naturalWidth > 0));
await page.waitForTimeout(300);
const height = await page.evaluate(() => document.body.scrollHeight);
await page.setViewportSize({ width: 1600, height: Math.ceil(height) });
await page.screenshot({ path: resolve(preview, "c-green.png"), fullPage: true });
await browser.close();
console.log("wrote preview/c-green.png");
