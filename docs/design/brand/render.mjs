// Regenerates the brand preview PNGs in docs/design/brand/preview.
//
//   node docs/design/brand/render.mjs
//
// Uses the Playwright Chromium installed with the repo. If the library and the
// local browser build disagree, point CHROMIUM_PATH at a Chromium/Chrome binary.
import { chromium } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const preview = resolve(root, "preview");
const chromiumPath = process.env.CHROMIUM_PATH;
const browser = await chromium.launch(chromiumPath ? { executablePath: chromiumPath } : {});

async function screenshotHtml(html, path, width, height, fullPage = false) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 2 });
  await page.setContent(html, { waitUntil: "networkidle" });
  await page.waitForFunction(() =>
    [...document.images].every((image) => image.complete && image.naturalWidth > 0),
  );
  await page.waitForTimeout(250);
  if (fullPage) {
    const contentHeight = await page.evaluate(() => document.body.scrollHeight);
    await page.setViewportSize({ width, height: Math.ceil(contentHeight) });
    await page.waitForTimeout(100);
  }
  await page.screenshot({ path, fullPage });
  await page.close();
  console.log("wrote", path.replace(`${root}/`, ""));
}

/* ------------------------------- icons ---------------------------------- */

const iconBase = resolve(root, "icons");
const dataUri = (file) =>
  `data:image/svg+xml;base64,${readFileSync(file).toString("base64")}`;

const icons = [
  ["01-cable-c.svg", "01", "Cable C", "A cabled knit “c” on warm paper"],
  ["02-pocket-card.svg", "02", "Pocket card", "A card tucked into a ribbed pocket"],
  ["03-button-coin.svg", "03", "Button coin", "A cardigan button that reads as a coin"],
  ["04-cm-weave.svg", "04", "cm, woven", "The current monogram, softened"],
];

const cards = icons
  .map(
    ([file, number, name, description]) => `
    <section class="card">
      <div class="big"><img src="${dataUri(resolve(iconBase, file))}" width="256" height="256"></div>
      <div class="meta"><span class="num">${number}</span><div><h2>${name}</h2><p>${description}</p></div></div>
      <div class="strip">
        <img src="${dataUri(resolve(iconBase, file))}" width="64" height="64">
        <img src="${dataUri(resolve(iconBase, file))}" width="48" height="48">
        <img src="${dataUri(resolve(iconBase, file))}" width="32" height="32">
        <img src="${dataUri(resolve(iconBase, file))}" width="16" height="16">
      </div>
      <div class="size-labels"><span>64</span><span>48</span><span>32</span><span>16</span></div>
    </section>`,
  )
  .join("");

await screenshotHtml(
  `<!doctype html><html><head><meta charset="utf-8">
  <style>
    *{box-sizing:border-box;margin:0;padding:0}
    body{background:#f7f5ef;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",sans-serif;color:#161d26;padding:56px 56px 48px;width:1600px}
    header{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:1px solid #d9d2c2;padding-bottom:22px;margin-bottom:30px}
    .eyebrow{font:600 12px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.18em;color:#2c6659;text-transform:uppercase}
    h1{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:46px;letter-spacing:-.02em;margin-top:10px}
    .lede{max-width:440px;color:#5f6a78;font-size:14px;line-height:1.55;text-align:right}
    .grid{display:grid;grid-template-columns:repeat(4,1fr);gap:18px}
    .card{background:#fffdf8;border:1px solid #e3ddce;border-radius:18px;padding:24px 22px 18px;display:flex;flex-direction:column;gap:18px}
    .big{display:grid;place-items:center;padding:8px 0 4px;background:radial-gradient(circle at 25% 20%,#fff 0,transparent 55%),#f1ede3;border-radius:14px;border:1px solid #ece6d8}
    .big img{filter:drop-shadow(0 14px 22px rgba(22,29,38,.18))}
    .meta{display:flex;gap:12px;align-items:flex-start}
    .num{font:600 11px/1.6 ui-monospace,"SF Mono",Menlo,monospace;background:#efeae0;border:1px solid #e3ddce;border-radius:6px;padding:2px 7px;color:#2c6659}
    h2{font-size:17px;letter-spacing:-.015em;font-weight:650}
    .meta p{font-size:12.5px;color:#5f6a78;line-height:1.4;margin-top:3px;max-width:20ch}
    .strip{display:flex;align-items:flex-end;gap:14px;border-top:1px solid #ece6d8;padding-top:16px}
    .strip img{filter:drop-shadow(0 2px 3px rgba(22,29,38,.16))}
    .size-labels{display:flex;gap:14px;font:500 9px/1 ui-monospace,Menlo,monospace;color:#9c917d;margin-top:-8px}
    footer{display:flex;justify-content:space-between;margin-top:26px;font:500 11px/1.5 ui-monospace,Menlo,monospace;color:#8a8172}
  </style></head><body>
  <header><div><div class="eyebrow">cardigan_money · brand concepts</div><h1>Four directions for the app icon</h1></div>
  <p class="lede">Warm paper, ledger green and rust. Shown at 256 px, then 64, 48, 32 and 16 px — the size that decides whether an icon survives the Dock and the menu bar.</p></header>
  <div class="grid">${cards}</div>
  <footer><span>drafts · not implemented</span><span>vector source · 1024 × 1024 · macOS squircle grid</span></footer>
  </body></html>`,
  resolve(preview, "icons.png"),
  1600,
  900,
  true,
);


/* ---------------------------- icons, round two -------------------------- */

const roundTwo = [
  ["05-cable-card.svg", "E", "Cable card", "A mint card with a cable-knit stripe. The product, up close."],
  ["06-coin-c.svg", "F", "Coin C", "A mint coin with a cut-in C and one rust bead."],
  ["07-ledger-c.svg", "G", "Ledger C", "A quiet ledger page with one big, inked C."],
  ["08-stitch-chart.svg", "H", "Stitch chart", "Four columns of spend, joined by a cable thread."],
  ["09-argyle.svg", "I", "Argyle", "A cardigan diamond, buttoned at the centre."],
  ["10-woven-cards.svg", "J", "Woven cards", "Two cards woven into one mark, pinned by a button."],
];

const roundTwoCards = roundTwo
  .map(
    ([file, key, name, description]) => `
    <article class="card">
      <div class="stage"><img class="icon" src="${dataUri(resolve(iconBase, file))}" width="206" height="206"></div>
      <div class="meta"><span class="key">${key}</span><div><h2>${name}</h2><p>${description}</p></div></div>
      <div class="sizes">
        <span class="label">Small</span>
        <img src="${dataUri(resolve(iconBase, file))}" width="64" height="64">
        <img src="${dataUri(resolve(iconBase, file))}" width="32" height="32">
        <img src="${dataUri(resolve(iconBase, file))}" width="16" height="16">
      </div>
      <div class="lockup"><img src="${dataUri(resolve(iconBase, file))}" width="28" height="28"><span>cardigan_money</span></div>
    </article>`,
  )
  .join("");

await screenshotHtml(
  `<!doctype html><html><head><meta charset="utf-8"><style>
    *{box-sizing:border-box;margin:0;padding:0}
    body{background:#12171c;color:#edf0f4;width:1600px;padding:54px 54px 40px;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",sans-serif}
    header{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:1px solid #2a323b;padding-bottom:24px;margin-bottom:28px}
    .eyebrow{font:600 10.5px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.19em;color:#7fc2b0;text-transform:uppercase}
    h1{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:46px;line-height:1.08;letter-spacing:-.025em;color:#edf0f4;margin-top:11px}
    h1 em{color:#7fc2b0;font-style:italic}
    .lede{max-width:400px;color:#97a2b1;font-size:13.5px;line-height:1.55;text-align:right}
    .grid{display:grid;grid-template-columns:repeat(3,1fr);gap:16px}
    .card{background:#181f25;border:1px solid #2a333c;border-radius:18px;padding:16px 16px 14px;display:flex;flex-direction:column;min-width:0}
    .stage{background:radial-gradient(circle at 25% 18%,#26333b 0,#1b242b 55%,#151b21 100%);border:1px solid #2b353e;border-radius:13px;display:grid;place-items:center;padding:18px 8px}
    .stage .icon{filter:drop-shadow(0 18px 20px rgba(0,0,0,.4))}
    .meta{display:flex;gap:10px;align-items:flex-start;margin-top:16px}
    .key{font:600 10px/1.6 ui-monospace,"SF Mono",Menlo,monospace;background:#22312d;border:1px solid #31463f;border-radius:6px;padding:2px 7px;color:#7fc2b0}
    h2{font-size:16px;letter-spacing:-.015em;font-weight:650;color:#edf0f4}
    .meta p{font-size:12.5px;color:#97a2b1;line-height:1.45;margin-top:3px;max-width:28ch}
    .sizes{display:flex;align-items:flex-end;gap:13px;border-top:1px solid #2a323b;padding-top:13px;margin-top:15px}
    .sizes .label{font:500 8px/1.4 ui-monospace,Menlo,monospace;letter-spacing:.14em;text-transform:uppercase;color:#6f7a88;margin-right:2px;align-self:center}
    .sizes img{filter:drop-shadow(0 3px 4px rgba(0,0,0,.45))}
    .lockup{display:flex;align-items:center;gap:9px;margin-top:13px;padding-top:12px;border-top:1px solid #242c34}
    .lockup span{font:500 10px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.06em;color:#aeb8c5}
    footer{display:flex;justify-content:space-between;margin-top:24px;font:500 10px/1.5 ui-monospace,Menlo,monospace;color:#6f7a88;letter-spacing:.06em}
  </style></head><body>
  <header><div><div class="eyebrow">cardigan_money · round two</div><h1>Six more marks, <em>built for Knit Graph.</em></h1></div>
  <p class="lede">The same graphite, mint and rust as hero 2. A–D are still in play; these are six new directions to compare.</p></header>
  <div class="grid">${roundTwoCards}</div>
  <footer><span>drafts · not implemented</span><span>vector source · 1024 × 1024 · macOS squircle grid</span></footer>
  </body></html>`,
  resolve(preview, "icons-round2.png"),
  1600,
  1150,
  true,
);

/* ------------------------------- heroes --------------------------------- */

const heroes = [
  ["h1-quiet-ledger.svg", "h1-quiet-ledger.png"],
  ["h2-knit-graph.svg", "h2-knit-graph.png"],
  ["h3-wallet.svg", "h3-wallet.png"],
];

for (const [source, target] of heroes) {
  const svg = readFileSync(resolve(root, "hero", source)).toString("base64");
  const page = await browser.newPage({ viewport: { width: 1600, height: 560 }, deviceScaleFactor: 2 });
  await page.setContent(
    `<!doctype html><html><head><style>html,body{margin:0;background:#fff}img{display:block;width:1600px;height:560px}</style></head>
     <body><img id="hero" src="data:image/svg+xml;base64,${svg}"></body></html>`,
    { waitUntil: "networkidle" },
  );
  await page.waitForFunction(() => {
    const image = document.getElementById("hero");
    return image && image.complete && image.naturalWidth > 0;
  });
  await page.waitForTimeout(200);
  await page.locator("#hero").screenshot({ path: resolve(preview, target) });
  await page.close();
  console.log("wrote", `preview/${target}`);
}

await browser.close();
