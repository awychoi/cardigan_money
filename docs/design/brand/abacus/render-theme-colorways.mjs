// Renders the installed placket colorways for the four bundled themes.
//
//   node docs/design/brand/abacus/render-theme-colorways.mjs
//
// Reads the shipped SVGs in src/assets/brand, so the board cannot drift from the
// app. Writes preview/theme-colorways.png.

import { chromium } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const repo = resolve(root, "../../../..");
const preview = resolve(root, "preview");

const themes = ["blueprint", "editorial", "graphite", "ledger"].map((id) => {
  const theme = JSON.parse(readFileSync(resolve(repo, "src/themes", `${id}.theme.json`), "utf8"));
  const svg = readFileSync(resolve(repo, "src/assets/brand", `placket-${id}.svg`), "utf8");
  const fill = (pattern) => svg.match(pattern)?.[1] ?? "#000000";
  return {
    id,
    name: theme.name,
    description: theme.description,
    svg,
    uri: `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`,
    colors: {
      tile: fill(/<rect[^>]*fill="([^"]+)"/),
      placket: fill(/<path d="M 376[^>]*fill="([^"]+)"/),
      button: fill(/<circle cx="512" cy="440"[^>]*fill="([^"]+)"/),
      accent: fill(/<circle cx="512" cy="560"[^>]*fill="([^"]+)"/),
      thread: fill(/<path d="M 404[^>]*stroke="([^"]+)"/),
    },
  };
});

const chip = (color) => `<span class="chip"><i style="background:${color}"></i>${color}</span>`;
const column = (theme) => `<section class="col">
  <div class="head"><h2>${theme.name}</h2></div>
  <p class="desc">${theme.description ?? ""}</p>
  <div class="chips">${Object.values(theme.colors).map(chip).join("")}</div>
  <img class="big" src="${theme.uri}" width="264" height="264" alt="${theme.name} placket">
  <div class="lockup"><img src="${theme.uri}" width="26" height="26" alt=""><span>cardigan_money</span></div>
  <div class="sizes">
    ${[64, 32, 16].map((size) => `<div><img src="${theme.uri}" width="${size}" height="${size}" alt=""><span>${size}</span></div>`).join("")}
  </div>
  <div class="picker">
    <div class="pv"><span class="pv-side" style="background:${theme.colors.tile}"></span><span class="pv-body"><i style="background:${theme.colors.placket};width:44%"></i><i style="background:${theme.colors.thread};width:64%"></i><i style="background:${theme.colors.button};width:76%"></i></span></div>
    <div class="pname"><img src="${theme.uri}" width="20" height="20" alt=""><b>${theme.name}</b></div>
  </div>
</section>`;

const html = `<!doctype html><html><head><meta charset="utf-8"><style>
  *{box-sizing:border-box}
  body{margin:0;background:#efece4;color:#11141b;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text",sans-serif;padding:38px 46px 32px}
  h1{font-family:"Hoefler Text","Iowan Old Style",Palatino,Georgia,serif;font-weight:500;font-size:40px;letter-spacing:-.03em;margin:0 0 6px}
  .sub{margin:0 0 26px;color:#5b6678;font-size:14.5px;max-width:1000px;line-height:1.5}
  .grid{display:grid;grid-template-columns:repeat(4,1fr);gap:18px}
  .col{background:#fff;border:1px solid #ddd6c7;border-radius:16px;padding:16px}
  .head{border-bottom:1px solid #eee8dc;padding-bottom:8px}
  h2{font-size:20px;margin:0;letter-spacing:-.02em}
  .desc{margin:7px 0 8px;color:#6a7484;font-size:11.5px;line-height:1.4;min-height:32px}
  .chips{display:flex;gap:4px;flex-wrap:wrap;min-height:14px}
  .chip{display:inline-flex;align-items:center;gap:3px;font-family:ui-monospace,monospace;font-size:8px;color:#7a8494}
  .chip i{width:8px;height:8px;border-radius:2px;border:1px solid #0002}
  .big{display:block;width:264px;height:264px;margin:12px auto 12px}
  .lockup{display:flex;align-items:center;gap:8px;background:#07090d;border-radius:9px;padding:8px 10px;margin-bottom:12px}
  .lockup img{border-radius:6px}
  .lockup span{font-family:ui-monospace,monospace;font-size:12px;color:#eef2f8;letter-spacing:.02em}
  .sizes{display:flex;align-items:flex-end;gap:18px;padding:2px 4px 12px;border-bottom:1px solid #eee8dc}
  .sizes div{display:flex;flex-direction:column;align-items:center;gap:4px}
  .sizes span{font-family:ui-monospace,monospace;font-size:8px;color:#98a2b2}
  .picker{padding-top:10px}
  .pv{display:flex;height:44px;border:1px solid #ddd;border-radius:6px;overflow:hidden}
  .pv-side{width:26%}
  .pv-body{flex:1;display:grid;align-content:center;gap:3px;padding:6px 8px}
  .pv-body i{display:block;height:3px;border-radius:2px;opacity:.85}
  .pname{display:flex;align-items:center;gap:6px;margin-top:7px}
  .pname img{border-radius:4px}
  .pname b{font-size:12px}
  footer{margin-top:24px;color:#6a7481;font-size:12px;border-top:1px solid #d8d0c0;padding-top:14px}
</style></head><body>
  <h1>One placket, four themes.</h1>
  <p class="sub">The tile, placket, buttons, accent button and neckline thread take each theme's signature colors.
  <strong>Blueprint is the fixed default</strong> for the Dock icon and any theme without a colorway.
  The board is rendered from the shipped <code>src/assets/brand/placket-*.svg</code> files.</p>
  <div class="grid">${themes.map(column).join("")}</div>
  <footer>26 px header mark · 20 px theme-picker mark · 16/32/64 px Dock checks. Geometry: the 04 Placket, 824 px tile, rx 184.</footer>
</body></html>`;

const browser = await chromium.launch(
  process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {},
);
const page = await browser.newPage({
  viewport: { width: 1600, height: 900 },
  deviceScaleFactor: 2,
});
await page.setContent(html, { waitUntil: "networkidle" });
await page.waitForFunction(() => [...document.images].every((image) => image.complete));
await page.waitForTimeout(200);
const height = await page.evaluate(() => document.body.scrollHeight);
await page.setViewportSize({ width: 1600, height: Math.ceil(height) });
await page.screenshot({ path: resolve(preview, "theme-colorways.png"), fullPage: true });
await browser.close();
console.log("wrote preview/theme-colorways.png");
