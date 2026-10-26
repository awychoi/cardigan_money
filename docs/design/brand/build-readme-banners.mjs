// Builds README banner candidates with a prominent mark and wordmark.
//
//   PLAYWRIGHT_BROWSERS_PATH="$PWD/.cache/ms-playwright" \
//     node docs/design/brand/build-readme-banners.mjs
//
// Writes vector sources to hero/options and 2x PNG previews to preview/.
import { chromium } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const heroDir = resolve(root, "hero/options");
const previewDir = resolve(root, "preview");
const mark = `data:image/svg+xml;base64,${readFileSync(resolve(root, "../../../src/assets/brand-mark.svg")).toString("base64")}`;

const W = 1600;
const H = 480;
const MINT = "#7fc2b0";
const MINT_SOFT = "#a8d9cc";
const CREAM = "#f2e7d5";
const RUST = "#d79b7d";

function wordmark(x, y, size, fill, underline = MINT, anchor = "start") {
  return `<text x="${x}" y="${y}" text-anchor="${anchor}" font-family="'SF Mono', ui-monospace, Menlo, monospace" font-size="${size}" font-weight="500" letter-spacing="-${(size * 0.028).toFixed(2)}" fill="${fill}">cardigan<tspan fill="${underline}">_</tspan>money</text>`;
}

function icon(x, y, size, filter = "url(#icon-shadow)") {
  return `<image href="${mark}" x="${x}" y="${y}" width="${size}" height="${size}" filter="${filter}"/>`;
}

function tagline(x, y, text, fill = MINT, anchor = "start", size = 11.5, spacing = 2.4) {
  return `<text x="${x}" y="${y}" text-anchor="${anchor}" font-family="'SF Mono', ui-monospace, Menlo, monospace" font-size="${size}" font-weight="500" letter-spacing="${spacing}" fill="${fill}">${text}</text>`;
}

function fact(x, y, text, fill = "#97a2b1") {
  return `<circle cx="${x}" cy="${y - 3.5}" r="3" fill="${MINT}" opacity=".8"/><text x="${x + 12}" y="${y}" font-family="'SF Mono', ui-monospace, Menlo, monospace" font-size="10.5" font-weight="500" letter-spacing=".4" fill="${fill}">${text}</text>`;
}

/* ------------------------------ Option A -------------------------------- */

const aBars = [34, 52, 44, 78, 64, 96, 110, 136];
const aChart = aBars
  .map((height, index) => {
    const x = 1112 + index * 48;
    const isLast = index === aBars.length - 1;
    return isLast
      ? `<rect x="${x}" y="${338 - height}" width="30" height="${height}" rx="6" fill="url(#a-hatch)" stroke="${RUST}" stroke-width="1.4" opacity=".9"/>`
      : `<rect x="${x}" y="${338 - height}" width="30" height="${height}" rx="6" fill="url(#a-bar)" opacity="${0.62 + index * 0.045}"/>`;
  })
  .join("");

const optionA = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-label="cardigan_money banner — name first, with a small spending chart">
  <defs>
    <linearGradient id="a-bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#101419"/><stop offset=".6" stop-color="#141a20"/><stop offset="1" stop-color="#0e1317"/>
    </linearGradient>
    <radialGradient id="a-glow" cx=".16" cy=".18" r=".85">
      <stop offset="0" stop-color="#2d6a5b" stop-opacity=".34"/><stop offset="1" stop-color="#2d6a5b" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="a-bar" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#9bd9c8"/><stop offset="1" stop-color="#5aa894"/>
    </linearGradient>
    <pattern id="a-hatch" width="6" height="6" patternTransform="rotate(45)" patternUnits="userSpaceOnUse">
      <line x1="0" y1="0" x2="0" y2="6" stroke="${RUST}" stroke-width="2.2" opacity=".55"/>
    </pattern>
    <filter id="icon-shadow" x="-40%" y="-40%" width="180%" height="180%">
      <feDropShadow dx="0" dy="18" stdDeviation="22" flood-color="#000000" flood-opacity=".5"/>
    </filter>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#a-bg)"/>
  <rect width="${W}" height="${H}" fill="url(#a-glow)"/>
  <g fill="none" stroke-linecap="round" opacity=".55">
    <path d="M 1588 76 A 310 310 0 1 0 1588 474" stroke="#1b2622" stroke-width="120"/>
    <path d="M 1588 76 A 310 310 0 1 0 1588 474" stroke="#354941" stroke-width="2" opacity=".55" stroke-dasharray="18 26"/>
  </g>
  <g stroke="#ffffff" stroke-width="1" opacity=".028">
    <path d="M 1538 0 V ${H} M 1554 0 V ${H} M 1570 0 V ${H} M 1586 0 V ${H}"/>
  </g>

  ${icon(84, 72, 152)}
  ${wordmark(270, 178, 68, CREAM)}
  ${tagline(274, 218, "LOCAL-FIRST FINANCE FOR MACOS", MINT, "start", 11.5, 2.2)}
  <line x1="84" y1="286" x2="794" y2="286" stroke="#2a333c" stroke-width="1"/>
  ${fact(88, 330, "SQLCipher encrypted")}
  ${fact(286, 330, "No account")}
  ${fact(424, 330, "No telemetry")}

  ${aChart}
  <line x1="1112" y1="339" x2="1512" y2="339" stroke="#2a333c" stroke-width="1"/>
</svg>`;

/* ------------------------------ Option B -------------------------------- */

const bBars = [8, 14, 11, 20, 16, 26, 22, 32, 40];
const bChart = bBars
  .map((height, index) => {
    const x = 694 + index * 24;
    const y = 414 - height;
    const isLast = index === bBars.length - 1;
    return isLast
      ? `<rect x="${x}" y="${y}" width="14" height="${height}" rx="4" fill="url(#b-hatch)" stroke="${RUST}" stroke-width="1" opacity=".85"/>`
      : `<rect x="${x}" y="${y}" width="14" height="${height}" rx="4" fill="url(#b-bar)" opacity="${0.48 + index * 0.04}"/>`;
  })
  .join("");

const optionB = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-label="cardigan_money banner — centered mark and oversized wordmark">
  <defs>
    <linearGradient id="b-bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#11161b"/><stop offset=".52" stop-color="#161c22"/><stop offset="1" stop-color="#0d1216"/>
    </linearGradient>
    <radialGradient id="b-glow" cx=".5" cy=".36" r=".62">
      <stop offset="0" stop-color="#2e6e5f" stop-opacity=".3"/><stop offset="1" stop-color="#2e6e5f" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="b-bar" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#9bd9c8"/><stop offset="1" stop-color="#5aa894"/>
    </linearGradient>
    <pattern id="b-hatch" width="5" height="5" patternTransform="rotate(45)" patternUnits="userSpaceOnUse">
      <line x1="0" y1="0" x2="0" y2="5" stroke="${RUST}" stroke-width="1.8" opacity=".6"/>
    </pattern>
    <filter id="icon-shadow" x="-45%" y="-45%" width="190%" height="190%">
      <feDropShadow dx="0" dy="20" stdDeviation="26" flood-color="#000000" flood-opacity=".55"/>
    </filter>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#b-bg)"/>
  <rect width="${W}" height="${H}" fill="url(#b-glow)"/>
  <g fill="none" stroke-linecap="round" opacity=".5">
    <path d="M -40 372 A 330 330 0 1 0 610 372" stroke="#1b2622" stroke-width="116"/>
    <path d="M -40 372 A 330 330 0 1 0 610 372" stroke="#354941" stroke-width="2" opacity=".5" stroke-dasharray="18 28"/>
    <path d="M 1640 372 A 330 330 0 1 1 990 372" stroke="#1b2622" stroke-width="116"/>
    <path d="M 1640 372 A 330 330 0 1 1 990 372" stroke="#354941" stroke-width="2" opacity=".5" stroke-dasharray="18 28"/>
  </g>
  <g stroke="#ffffff" stroke-width="1" opacity=".025">
    <path d="M 30 0 V ${H} M 46 0 V ${H} M 62 0 V ${H} M 1554 0 V ${H} M 1570 0 V ${H} M 1586 0 V ${H}"/>
  </g>

  ${icon(730, 54, 140)}
  ${wordmark(800, 274, 78, CREAM, MINT, "middle")}
  ${tagline(800, 314, "YOUR CARDS, KEPT ON YOUR MAC", MINT_SOFT, "middle", 11.5, 3.2)}
  <line x1="560" y1="346" x2="1040" y2="346" stroke="#2a333c" stroke-width="1"/>
  ${bChart}
  <line x1="686" y1="415" x2="914" y2="415" stroke="#2a333c" stroke-width="1"/>
</svg>`;

/* ------------------------------ Option C -------------------------------- */

const cBars = [70, 104, 86, 150, 124, 190];
const cChart = cBars
  .map((height, index) => {
    const x = 1136 + index * 50;
    const y = 330 - height;
    const isLast = index === cBars.length - 1;
    return isLast
      ? `<rect x="${x}" y="${y}" width="34" height="${height}" rx="7" fill="url(#c-hatch)" stroke="${RUST}" stroke-width="1.2" opacity=".9"/>`
      : `<rect x="${x}" y="${y}" width="34" height="${height}" rx="7" fill="url(#c-bar)" opacity="${0.55 + index * 0.06}"/>`;
  })
  .join("");

const optionC = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-label="cardigan_money banner — warm paper lockup with a compact ledger panel">
  <defs>
    <linearGradient id="c-paper" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#fbf9f3"/><stop offset=".62" stop-color="#f7f5ef"/><stop offset="1" stop-color="#f1ece0"/>
    </linearGradient>
    <radialGradient id="c-glow" cx=".18" cy=".2" r=".75">
      <stop offset="0" stop-color="#7fc2b0" stop-opacity=".18"/><stop offset="1" stop-color="#7fc2b0" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="c-panel" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#1b4a40"/><stop offset="1" stop-color="#102f28"/>
    </linearGradient>
    <linearGradient id="c-bar" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#9bd9c8"/><stop offset="1" stop-color="#5aa894"/>
    </linearGradient>
    <pattern id="c-hatch" width="6" height="6" patternTransform="rotate(45)" patternUnits="userSpaceOnUse">
      <line x1="0" y1="0" x2="0" y2="6" stroke="${RUST}" stroke-width="2.2" opacity=".6"/>
    </pattern>
    <filter id="icon-shadow" x="-40%" y="-40%" width="180%" height="180%">
      <feDropShadow dx="0" dy="16" stdDeviation="20" flood-color="#293c33" flood-opacity=".32"/>
    </filter>
    <filter id="panel-shadow" x="-20%" y="-20%" width="140%" height="150%">
      <feDropShadow dx="0" dy="18" stdDeviation="22" flood-color="#293c33" flood-opacity=".22"/>
    </filter>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#c-paper)"/>
  <rect width="${W}" height="${H}" fill="url(#c-glow)"/>
  <g fill="none" stroke-linecap="round" opacity=".55">
    <path d="M 690 356 A 250 250 0 1 0 690 156" stroke="#e4ebdf" stroke-width="86"/>
    <path d="M 690 356 A 250 250 0 1 0 690 156" stroke="#c7d8cb" stroke-width="1.5" opacity=".55" stroke-dasharray="14 22"/>
  </g>
  <g stroke="#e3ddce" stroke-width="1" opacity=".7">
    <path d="M 1538 0 V ${H} M 1554 0 V ${H} M 1570 0 V ${H} M 1586 0 V ${H}"/>
  </g>

  ${icon(84, 72, 152)}
  ${wordmark(270, 178, 66, "#18211f", "#2c6659")}
  ${tagline(274, 218, "LOCAL-FIRST FINANCE FOR MACOS", "#2c6659", "start", 11.5, 2.2)}
  <line x1="84" y1="286" x2="1040" y2="286" stroke="#ddd6c7" stroke-width="1"/>
  ${fact(88, 330, "SQLCipher encrypted", "#5f6a78")}
  ${fact(286, 330, "No account", "#5f6a78")}
  ${fact(424, 330, "No telemetry", "#5f6a78")}

  <g filter="url(#panel-shadow)">
    <rect x="1100" y="58" width="416" height="364" rx="26" fill="#163e35"/>
    <rect x="1112" y="70" width="392" height="340" rx="20" fill="url(#c-panel)" stroke="#2f5a50" stroke-width="1"/>
    ${cChart}
    <line x1="1136" y1="331" x2="1480" y2="331" stroke="#2f5a50" stroke-width="1"/>
  </g>
</svg>`;

const options = [
  ["r1-name-first", optionA],
  ["r2-center-stage", optionB],
  ["r3-paper-label", optionC],
];

for (const [name, svg] of options) {
  writeFileSync(resolve(heroDir, `${name}.svg`), svg);
  console.log("wrote", `hero/options/${name}.svg`);
}

const browser = await chromium.launch(
  process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {},
);

async function renderSvg(svg, target) {
  const page = await browser.newPage({
    viewport: { width: W, height: H },
    deviceScaleFactor: 2,
  });
  await page.setContent(
    `<!doctype html><html><head><style>html,body{margin:0;background:#fff}img{display:block;width:${W}px;height:${H}px}</style></head>
     <body><img id="banner" src="data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}"></body></html>`,
    { waitUntil: "networkidle" },
  );
  await page.waitForFunction(() => {
    const image = document.getElementById("banner");
    return image && image.complete && image.naturalWidth > 0;
  });
  await page.waitForTimeout(200);
  await page.locator("#banner").screenshot({ path: target });
  await page.close();
  console.log("wrote", `preview/${target.split("/").pop()}`);
}

for (const [name, svg] of options) {
  await renderSvg(svg, resolve(previewDir, `${name}.png`));
}

/* --------------------------- contact sheet ------------------------------- */

const order = [
  [
    "r1-name-first",
    "01",
    "Name first",
    "Large mark and wordmark left; the graph becomes a small supporting strip.",
  ],
  [
    "r2-center-stage",
    "02",
    "Center stage",
    "Centered mark and oversized wordmark; a tiny stitch chart sits underneath.",
  ],
  [
    "r3-paper-label",
    "03",
    "Paper label",
    "Warm paper version with the graph contained in a compact ledger panel.",
  ],
];

const sheet = `<!doctype html><html><head><meta charset="utf-8"><style>
  *{box-sizing:border-box;margin:0;padding:0}
  body{width:1600px;background:#0e1216;color:#edf0f4;padding:48px 56px 42px;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",sans-serif}
  header{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:1px solid #2a333c;padding-bottom:22px;margin-bottom:28px}
  .eyebrow{font:600 10px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.22em;color:#7fc2b0;text-transform:uppercase}
  h1{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:44px;letter-spacing:-.03em;margin-top:10px}
  .lede{max-width:410px;text-align:right;color:#97a2b1;font-size:13px;line-height:1.55}
  .option{margin-bottom:24px}
  .meta{display:flex;align-items:baseline;gap:10px;margin-bottom:10px}
  .num{font:600 10px/1.6 ui-monospace,"SF Mono",Menlo,monospace;background:#22312d;border:1px solid #31463f;border-radius:6px;padding:2px 7px;color:#7fc2b0}
  h2{font-size:16px;font-weight:650;letter-spacing:-.01em}
  .meta p{color:#8d99a7;font-size:12px;margin-left:2px}
  img{display:block;width:1488px;height:446px;border:1px solid #2a333c;border-radius:10px}
  footer{display:flex;justify-content:space-between;border-top:1px solid #2a333c;margin-top:26px;padding-top:16px;font:500 9.5px/1.5 ui-monospace,Menlo,monospace;letter-spacing:.07em;color:#6f7a88}
</style></head><body>
<header><div><div class="eyebrow">cardigan_money · README banner candidates</div><h1>Three ways to put the name first.</h1></div>
<p class="lede">1600 × 480. The mark and wordmark are enlarged; the chart is now a supporting motif. Shown here at 1488 px wide, close to GitHub README width.</p></header>
${order
  .map(
    ([name, num, title, note]) => `<section class="option">
      <div class="meta"><span class="num">${num}</span><h2>${title}</h2><p>${note}</p></div>
      <img src="data:image/svg+xml;base64,${Buffer.from(options.find(([n]) => n === name)[1]).toString("base64")}">
    </section>`,
  )
  .join("")}
<footer><span>drafts · pick one before replacing the README hero</span><span>Graphite #14171C · mint #7FC2B0 · cream #F2E7D5 · rust #D79B7D</span></footer>
</body></html>`;

const page = await browser.newPage({
  viewport: { width: 1600, height: 1400 },
  deviceScaleFactor: 1,
});
await page.setContent(sheet, { waitUntil: "networkidle" });
await page.waitForFunction(() =>
  [...document.images].every((image) => image.complete && image.naturalWidth > 0),
);
await page.waitForTimeout(300);
const height = await page.evaluate(() => document.body.scrollHeight);
await page.setViewportSize({ width: 1600, height: Math.ceil(height) });
await page.screenshot({ path: resolve(previewDir, "readme-banner-options.png"), fullPage: true });
await page.close();
console.log("wrote preview/readme-banner-options.png");

await browser.close();
