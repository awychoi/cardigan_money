// Builds the woven C/D variants and their comparison board.
//   CHROMIUM_PATH=... node docs/design/brand/build-woven.mjs
import { chromium } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const icons = resolve(root, "icons");
const preview = resolve(root, "preview");

// ---------------------------------------------------------------------------
// Palettes

const palettes = [
  {
    id: "paper-terracotta",
    name: "Paper + Terracotta",
    note: "Light, warm",
    bg1: "#fbf7ee", bg2: "#ece2d1", border: "#d7ccb8", glow: "#b26a4c", ring: "#b26a4c",
    weaveBase: "#8e4c34", weaveMid: "#c07a58", weaveLight: "#e1a788",
    rimBase: "#a85f42", rimLight: "#f6e8d4", rimDark: "#6e3826",
    thread: "#f7f3e8", hole: "#2b1a14", accent: "#2c6659",
    bgWeaveBase: "#f3e9da", bgWeaveMid: "#e2b29c", bgWeaveLight: "#fff6e9",
    medallion: "#1f4c42", label: "#1f4c42", labelInk: "#f7f3e8", labelStitch: "#e7d8c0",
    rim: "#f1e3cb", rimAlt: "#123a33",
  },
  {
    id: "forest-butter",
    name: "Forest + Butter",
    note: "Deep green, gold",
    bg1: "#2f6d5f", bg2: "#123a33", border: "#0d2e28", glow: "#e4c07a", ring: "#e4c07a",
    weaveBase: "#8a6423", weaveMid: "#c9a45a", weaveLight: "#eacf8f",
    rimBase: "#c9a45a", rimLight: "#fbf4e3", rimDark: "#5d451c",
    thread: "#fbf4e3", hole: "#14352e", accent: "#b26a4c",
    bgWeaveBase: "#1f4c42", bgWeaveMid: "#c9a45a", bgWeaveLight: "#2f6d5f",
    medallion: "#f7f3e8", label: "#f7f3e8", labelInk: "#1f4c42", labelStitch: "#c9a45a",
    rim: "#c9a45a", rimAlt: "#1f4c42",
  },
  {
    id: "graphite-poppy",
    name: "Graphite + Poppy",
    note: "Dark, hot",
    bg1: "#232d35", bg2: "#0d1216", border: "#070b0e", glow: "#e07a66", ring: "#e07a66",
    weaveBase: "#8c3d33", weaveMid: "#c05a4a", weaveLight: "#e68c78",
    rimBase: "#a34a3d", rimLight: "#f6e8d4", rimDark: "#54231d",
    thread: "#f7f3e8", hole: "#12171c", accent: "#7fc2b0",
    bgWeaveBase: "#161d23", bgWeaveMid: "#a84a3d", bgWeaveLight: "#e07a66",
    medallion: "#f7f3e8", label: "#f7f3e8", labelInk: "#161d26", labelStitch: "#e07a66",
    rim: "#e07a66", rimAlt: "#161d26",
  },
  {
    id: "paper-cobalt",
    name: "Paper + Cobalt",
    note: "Light, cool",
    bg1: "#f8f5ee", bg2: "#e9e1d2", border: "#d3c9b6", glow: "#4f6db0", ring: "#4f6db0",
    weaveBase: "#2b3f6b", weaveMid: "#4f6db0", weaveLight: "#8fa7d2",
    rimBase: "#3b5688", rimLight: "#f2e7d5", rimDark: "#1f2c4a",
    thread: "#f2e7d5", hole: "#151f33", accent: "#b26a4c",
    bgWeaveBase: "#f1ebdd", bgWeaveMid: "#4f6db0", bgWeaveLight: "#dde5f4",
    medallion: "#1f2e4d", label: "#1f2e4d", labelInk: "#f7f3e8", labelStitch: "#8fa7d2",
    rim: "#f2e7d5", rimAlt: "#4f6db0",
  },
];

// ---------------------------------------------------------------------------
// C — woven button

function cWoven(p) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img" aria-label="Woven button coin, ${p.name}">
  <defs>
    <linearGradient id="cw-bg" x1="0" y1="0" x2="0.9" y2="1">
      <stop offset="0" stop-color="${p.bg1}"/>
      <stop offset="0.62" stop-color="${p.bg2}"/>
      <stop offset="1" stop-color="${p.bg2}"/>
    </linearGradient>
    <radialGradient id="cw-glow" cx="0.4" cy="0.25" r="0.7">
      <stop offset="0" stop-color="${p.glow}" stop-opacity="0.22"/>
      <stop offset="1" stop-color="${p.glow}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="cw-shade" cx="0.42" cy="0.34" r="0.7">
      <stop offset="0.45" stop-color="#000000" stop-opacity="0"/>
      <stop offset="1" stop-color="#000000" stop-opacity="0.26"/>
    </radialGradient>
    <pattern id="cw-weave" width="120" height="120" patternUnits="userSpaceOnUse">
      <rect width="120" height="120" fill="${p.weaveBase}"/>
      <g fill="${p.weaveMid}">
        <rect x="0" y="7" width="56" height="18" rx="5"/>
        <rect x="0" y="31" width="56" height="18" rx="5"/>
        <rect x="64" y="67" width="56" height="18" rx="5"/>
        <rect x="64" y="91" width="56" height="18" rx="5"/>
      </g>
      <g fill="${p.weaveLight}">
        <rect x="70" y="0" width="18" height="56" rx="5"/>
        <rect x="94" y="0" width="18" height="56" rx="5"/>
        <rect x="6" y="64" width="18" height="56" rx="5"/>
        <rect x="30" y="64" width="18" height="56" rx="5"/>
      </g>
    </pattern>
    <filter id="cw-shadow" x="-30%" y="-30%" width="170%" height="180%">
      <feDropShadow dx="0" dy="20" stdDeviation="24" flood-color="#000000" flood-opacity="0.45"/>
    </filter>
    <filter id="cw-threadshadow" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="5" stdDeviation="5" flood-color="#000000" flood-opacity="0.35"/>
    </filter>
    <clipPath id="cw-tile">
      <rect x="100" y="100" width="824" height="824" rx="184"/>
    </clipPath>
  </defs>

  <g clip-path="url(#cw-tile)">
    <rect x="100" y="100" width="824" height="824" fill="url(#cw-bg)"/>
    <rect x="100" y="100" width="824" height="824" fill="url(#cw-glow)"/>
    <circle cx="512" cy="512" r="362" fill="none" stroke="${p.ring}" stroke-width="2" stroke-dasharray="2 16" opacity="0.3"/>
    <circle cx="512" cy="512" r="392" fill="none" stroke="${p.ring}" stroke-width="1" opacity="0.12"/>
  </g>

  <g filter="url(#cw-shadow)">
    <circle cx="512" cy="512" r="324" fill="${p.rimDark}"/>
    <circle cx="512" cy="512" r="292" fill="url(#cw-weave)"/>
    <circle cx="512" cy="512" r="292" fill="url(#cw-shade)"/>
    <ellipse cx="430" cy="372" rx="190" ry="105" fill="#ffffff" opacity="0.072"/>

    <circle cx="512" cy="512" r="308" fill="none" stroke="${p.rimBase}" stroke-width="32"/>
    <circle cx="512" cy="512" r="308" fill="none" stroke="${p.rimLight}" stroke-width="32" pathLength="100" stroke-dasharray="2 2" stroke-linecap="butt"/>
    <circle cx="512" cy="512" r="308" fill="none" stroke="${p.rimDark}" stroke-width="32" pathLength="100" stroke-dasharray="2 2" stroke-dashoffset="2" stroke-linecap="butt"/>
    <circle cx="512" cy="512" r="324" fill="none" stroke="${p.rimDark}" stroke-width="3" opacity="0.8"/>
    <circle cx="512" cy="512" r="292" fill="none" stroke="${p.rimDark}" stroke-width="4" opacity="0.55"/>

    <g fill="none" stroke="${p.thread}" stroke-width="24" stroke-linecap="round" filter="url(#cw-threadshadow)">
      <path d="M 436 436 C 480 478 548 546 588 588"/>
      <path d="M 436 588 C 480 546 548 478 588 436"/>
    </g>
    <path d="M 604 604 C 646 654 666 690 690 704" fill="none" stroke="${p.thread}" stroke-width="18" stroke-linecap="round" filter="url(#cw-threadshadow)"/>
    <circle cx="699" cy="709" r="10" fill="${p.thread}"/>

    <g fill="${p.hole}">
      <circle cx="436" cy="436" r="35"/>
      <circle cx="588" cy="436" r="35"/>
      <circle cx="436" cy="588" r="35"/>
      <circle cx="588" cy="588" r="35"/>
    </g>
    <g fill="none" stroke="${p.weaveLight}" stroke-width="3" opacity="0.25">
      <path d="M 410 450 A 30 30 0 0 0 462 450"/>
      <path d="M 562 450 A 30 30 0 0 0 614 450"/>
      <path d="M 410 602 A 30 30 0 0 0 462 602"/>
      <path d="M 562 602 A 30 30 0 0 0 614 602"/>
    </g>
    <circle cx="699" cy="709" r="4" fill="${p.accent}"/>
  </g>

  <rect x="101" y="101" width="822" height="822" rx="183" fill="none" stroke="${p.border}" stroke-width="2" opacity="0.7"/>
  <rect x="112" y="112" width="800" height="800" rx="174" fill="none" stroke="#ffffff" stroke-width="2" opacity="${p.bg1.startsWith('#f') ? 0.35 : 0.07}"/>
</svg>
`;
}

// ---------------------------------------------------------------------------
// D — woven label

const source = readFileSync(resolve(icons, "04-cm-weave.svg"), "utf8");
const cmGroup = source.match(/<g filter="url\(#cm-drop\)" fill="url\(#cm-ink\)">([\s\S]*?)<\/g>/);
if (!cmGroup) throw new Error("cm paths not found");
const cmPaths = [...cmGroup[1].matchAll(/<path d="([^"]+)"\/>/g)].map((m) => m[1]);
if (cmPaths.length !== 2) throw new Error("expected two cm paths");

function dWoven(p) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img" aria-label="Woven cm label, ${p.name}">
  <defs>
    <pattern id="dw-weave" width="160" height="160" patternUnits="userSpaceOnUse">
      <rect width="160" height="160" fill="${p.bgWeaveBase}"/>
      <g fill="${p.bgWeaveMid}">
        <rect x="0" y="10" width="74" height="22" rx="6"/>
        <rect x="0" y="44" width="74" height="22" rx="6"/>
        <rect x="86" y="86" width="74" height="22" rx="6"/>
        <rect x="86" y="120" width="74" height="22" rx="6"/>
      </g>
      <g fill="${p.bgWeaveLight}">
        <rect x="92" y="0" width="22" height="74" rx="6"/>
        <rect x="126" y="0" width="22" height="74" rx="6"/>
        <rect x="10" y="86" width="22" height="74" rx="6"/>
        <rect x="44" y="86" width="22" height="74" rx="6"/>
      </g>
    </pattern>
    <radialGradient id="dw-vig" cx="0.5" cy="0.45" r="0.72">
      <stop offset="0.5" stop-color="#000000" stop-opacity="0"/>
      <stop offset="1" stop-color="#000000" stop-opacity="0.24"/>
    </radialGradient>
    <linearGradient id="dw-label" x1="0" y1="0" x2="0.8" y2="1">
      <stop offset="0" stop-color="${p.label}"/>
      <stop offset="1" stop-color="${p.label}"/>
    </linearGradient>
    <filter id="dw-labelshadow" x="-30%" y="-30%" width="170%" height="180%">
      <feDropShadow dx="0" dy="20" stdDeviation="22" flood-color="#000000" flood-opacity="0.4"/>
    </filter>
    <clipPath id="dw-tile">
      <rect x="100" y="100" width="824" height="824" rx="184"/>
    </clipPath>
  </defs>

  <g clip-path="url(#dw-tile)">
    <rect x="100" y="100" width="824" height="824" fill="url(#dw-weave)"/>
    <rect x="100" y="100" width="824" height="824" fill="url(#dw-vig)"/>
    <rect x="100" y="100" width="824" height="824" fill="none" stroke="#000000" stroke-width="3" opacity="0.2"/>
  </g>

  <g transform="rotate(-4 512 512)" filter="url(#dw-labelshadow)">
    <rect x="206" y="337" width="612" height="350" rx="34" fill="${p.label}"/>
    <rect x="206" y="337" width="612" height="350" rx="34" fill="none" stroke="${p.rimAlt}" stroke-width="2" opacity="0.35"/>
    <rect x="226" y="357" width="572" height="310" rx="24" fill="none" stroke="${p.labelStitch}" stroke-width="3" stroke-dasharray="3 12" opacity="0.7"/>
    <g transform="translate(512 512) scale(0.96) translate(-515.5 -552.5)" fill="${p.labelInk}">
      ${cmPaths.map((d) => `<path d="${d}"/>`).join("\n      ")}
      <circle cx="792" cy="626" r="20" fill="${p.accent}"/>
    </g>
    <circle cx="252" cy="382" r="13" fill="none" stroke="${p.labelStitch}" stroke-width="4"/>
    <path d="M 238 372 C 222 354 210 356 196 370" fill="none" stroke="${p.labelStitch}" stroke-width="4" stroke-linecap="round" opacity="0.9"/>
  </g>

  <rect x="101" y="101" width="822" height="822" rx="183" fill="none" stroke="${p.border}" stroke-width="2" opacity="0.65"/>
  <rect x="112" y="112" width="800" height="800" rx="174" fill="none" stroke="#ffffff" stroke-width="2" opacity="0.07"/>
</svg>
`;
}

const variantFiles = new Map(
  palettes.map((p, i) => [
    p.id,
    {
      c: `${11 + i}-c-woven-${p.id}.svg`,
      d: `${15 + i}-d-woven-${p.id}.svg`,
    },
  ]),
);

for (const p of palettes) {
  const files = variantFiles.get(p.id);
  writeFileSync(resolve(icons, files.c), cWoven(p));
  writeFileSync(resolve(icons, files.d), dWoven(p));
  console.log("wrote", files.c, files.d);
}

// ---------------------------------------------------------------------------
// Comparison board

const cFiles = [
  ["03-button-coin.svg", "Original", "Graphite + rust", "baseline", ["#14171C", "#B26A4C", "#F2E7D5"]],
  ...palettes.map((p) => [variantFiles.get(p.id).c, p.name, p.note, "new", [p.bg2, p.weaveMid, p.thread]]),
];
const dFiles = [
  ["04-cm-weave.svg", "Original", "Forest + cream", "baseline", ["#2C6659", "#F7F3E8", "#B26A4C"]],
  ...palettes.map((p) => [variantFiles.get(p.id).d, p.name, p.note, "new", [p.bgWeaveBase, p.bgWeaveMid, p.labelInk]]),
];

const uri = (f) => `data:image/svg+xml;base64,${readFileSync(resolve(icons, f)).toString("base64")}`;
const row = (title, sub, files) => `
  <div class="row-head"><h2>${title}</h2><p>${sub}</p></div>
  <div class="row">
    ${files.map(([file, name, note, kind, swatches]) => `
      <article class="cell ${kind === "baseline" ? "baseline" : ""}">
        <div class="stage"><img class="icon" src="${uri(file)}" width="200" height="200"></div>
        <div class="label">
          <div class="line"><h3>${name}</h3>${kind === "baseline" ? '<span class="pin">Original</span>' : ""}</div>
          <p>${note}</p>
          <div class="swatches">${swatches.map((c) => `<i style="background:${c}"></i>`).join("")}</div>
        </div>
        <div class="small"><img src="${uri(file)}" width="16" height="16"><span>16 px</span></div>
      </article>
    `).join("")}
  </div>`;

const board = `<!doctype html><html><head><meta charset="utf-8"><style>
  *{box-sizing:border-box;margin:0;padding:0}
  body{width:1600px;background:#12171c;color:#edf0f4;padding:52px 56px 38px;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",sans-serif}
  header{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:1px solid #2a323b;padding-bottom:24px;margin-bottom:26px}
  .eyebrow{font:600 10.5px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.19em;color:#7fc2b0;text-transform:uppercase}
  h1{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:48px;line-height:1.05;letter-spacing:-.028em;margin-top:11px}
  h1 em{color:#7fc2b0;font-style:italic}
  .lede{max-width:410px;text-align:right;color:#97a2b1;font-size:13px;line-height:1.55}
  .lede strong{color:#edf0f4;font-weight:600}
  .palette{display:flex;gap:7px;justify-content:flex-end;margin-top:10px}
  .palette span{font:500 8px/1 ui-monospace,Menlo,monospace;letter-spacing:.08em;color:#6f7a88;display:flex;align-items:center;gap:5px;text-transform:uppercase}
  .palette i{width:8px;height:8px;border-radius:2px;display:inline-block}
  .row-head{display:flex;align-items:baseline;justify-content:space-between;margin:30px 0 12px;padding-top:22px;border-top:1px solid #2a323b}
  .row-head h2{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:27px;letter-spacing:-.02em}
  .row-head p{font:500 9.5px/1.5 ui-monospace,Menlo,monospace;letter-spacing:.09em;color:#6f7a88;text-transform:uppercase}
  .row{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:13px}
  .cell{background:#181f25;border:1px solid #2a333c;border-radius:16px;padding:12px 12px 11px;display:flex;flex-direction:column}
  .cell.baseline{background:#151b21;border-color:#242c34}
  .stage{background:radial-gradient(circle at 28% 18%,#27343c 0,#1b242b 58%,#151b21 100%);border:1px solid #2b353e;border-radius:11px;display:grid;place-items:center;padding:15px 6px;min-height:201px}
  .stage .icon{filter:drop-shadow(0 16px 18px rgba(0,0,0,.45))}
  .label{margin-top:11px}
  .line{display:flex;align-items:center;justify-content:space-between;gap:8px}
  h3{font-size:13px;font-weight:650;letter-spacing:-.01em;color:#edf0f4}
  .pin{font:600 7.5px/1 ui-monospace,Menlo,monospace;letter-spacing:.12em;text-transform:uppercase;color:#97a2b1;border:1px solid #303a44;border-radius:999px;padding:4px 6px}
  .label p{font-size:11px;color:#8b96a5;margin-top:3px}
  .swatches{display:flex;gap:4px;margin-top:8px}
  .swatches i{width:12px;height:12px;border-radius:3px;border:1px solid rgba(255,255,255,.12)}
  .small{display:flex;align-items:center;gap:7px;margin-top:10px;padding-top:9px;border-top:1px solid #262e36}
  .small span{font:500 8px/1 ui-monospace,Menlo,monospace;letter-spacing:.08em;color:#6f7a88;text-transform:uppercase}
  footer{display:flex;justify-content:space-between;margin-top:24px;font:500 9.5px/1.5 ui-monospace,Menlo,monospace;letter-spacing:.06em;color:#6f7a88}
</style></head><body>
<header>
  <div><div class="eyebrow">cardigan_money · woven tests</div><h1>C + D, with more <em>weave.</em></h1></div>
  <div><p class="lede"><strong>Knit Graph is green and dark.</strong> C (woven button) and D (woven label) are shown in four colorways that push against it. The originals are on the left.</p>
    <div class="palette"><span><i style="background:#14171c"></i>graphite</span><span><i style="background:#7fc2b0"></i>mint</span><span><i style="background:#d79b7d"></i>rust</span></div>
  </div>
</header>
${row("C · Button coin", "Woven face · segmented rim", cFiles)}
${row("D · cm, woven", "Woven tile · stitched label", dFiles)}
<footer><span>drafts · 16 px shown at actual size</span><span>vector source · 1024 × 1024 · four colorways each</span></footer>
</body></html>`;

const exe = process.env.CHROMIUM_PATH;
const browser = await chromium.launch(exe ? { executablePath: exe } : {});
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
await page.setContent(board, { waitUntil: "networkidle" });
await page.waitForFunction(() => [...document.images].every((i) => i.complete && i.naturalWidth > 0));
await page.waitForTimeout(300);
const height = await page.evaluate(() => document.body.scrollHeight);
await page.setViewportSize({ width: 1600, height: Math.ceil(height) });
await page.screenshot({ path: resolve(preview, "woven-variants.png"), fullPage: true });
await browser.close();
console.log("wrote preview/woven-variants.png");
