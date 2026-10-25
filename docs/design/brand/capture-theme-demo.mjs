// Captures the four C marks in the running app shell across themes and modes.
// Requires the mock dev server:
//   VITE_MOCK_BRIDGE=1 VITE_MOCK_SCENARIO=demo pnpm dev
// Then run:
//   CHROMIUM_PATH=... node docs/design/brand/capture-theme-demo.mjs
// Writes preview/theme-demo.png and temporary crops under /tmp/theme-demo-raw.

import { chromium } from "@playwright/test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const rawDir = "/tmp/theme-demo-raw";
mkdirSync(rawDir, { recursive: true });

const buttons = [
  { id: "original", name: "Original", note: "Graphite tile", file: "03-button-coin.svg" },
  { id: "forest", name: "Forest", note: "Forest green", file: "19-c-green-forest.svg" },
  { id: "deep", name: "Deep", note: "Deep green", file: "20-c-green-deep.svg" },
  { id: "sage", name: "Sage", note: "Sage green", file: "21-c-green-sage.svg" },
];
const themes = [
  { id: "ledger", name: "Ledger", note: "Warm paper · green", swatches: ["#efeae0", "#181c22"] },
  { id: "graphite", name: "Graphite", note: "Cool neutrals · blue", swatches: ["#e9edf1", "#15181d"] },
  { id: "editorial", name: "Editorial", note: "Cream paper · terracotta", swatches: ["#f2ece0", "#1d1814"] },
];
const modes = ["light", "dark"];

const iconData = Object.fromEntries(
  buttons.map((b) => [b.id, readFileSync(resolve(root, "icons", b.file)).toString("base64")]),
);

const exe = process.env.CHROMIUM_PATH;
const browser = await chromium.launch(exe ? { executablePath: exe } : {});
const page = await browser.newPage({ viewport: { width: 1440, height: 960 }, deviceScaleFactor: 3 });
page.on("console", (m) => {
  if (m.type() === "error") console.log("console error:", m.text());
});

await page.goto("http://127.0.0.1:1420/", { waitUntil: "networkidle", timeout: 30000 });
await page.getByRole("button", { name: /Open demo/i }).click();
await page.getByRole("heading", { name: "Overview" }).waitFor();
await page.getByRole("button", { name: "Settings", exact: true }).click();
await page.locator(".theme-card").first().waitFor();

async function setTheme(theme, mode) {
  await page.locator(".theme-card", { hasText: theme.name }).click();
  await page.waitForFunction((id) => document.documentElement.dataset.skin === id, theme.id);
  await page.locator(".mode-seg button", { hasText: mode === "light" ? "Light" : "Dark" }).click();
  await page.waitForFunction((m) => document.documentElement.dataset.theme === m, mode);
  await page.waitForTimeout(320);
  await page.locator(".brand .status-pill").evaluateAll((els) => {
    for (const el of els) el.style.display = "none";
  });
}

async function setMark(id) {
  await page.locator(".brand-mark").evaluate((el, src) => {
    el.textContent = "";
    el.style.flex = "0 0 26px";
    el.style.width = "26px";
    el.style.height = "26px";
    el.style.backgroundColor = "transparent";
    el.style.backgroundImage = `url("data:image/svg+xml;base64,${src}")`;
    el.style.backgroundSize = "32.3px 32.3px";
    el.style.backgroundPosition = "center";
    el.style.backgroundRepeat = "no-repeat";
  }, iconData[id]);
  await page.waitForTimeout(140);
}

for (const theme of themes) {
  for (const mode of modes) {
    await setTheme(theme, mode);
    for (const button of buttons) {
      await setMark(button.id);
      await page.screenshot({
        path: resolve(rawDir, `${theme.id}-${mode}-${button.id}.png`),
        clip: { x: 0, y: 0, width: 224, height: 54 },
      });
      console.log("captured", theme.id, mode, button.id);
    }
  }
}

await browser.close();

/* ------------------------------- board ---------------------------------- */

const uri = (name) => `data:image/png;base64,${readFileSync(resolve(rawDir, name)).toString("base64")}`;
const modeLabel = (m) => (m === "light" ? "Light" : "Dark");

const headerCells = themes
  .map(
    (t) => `
    <div class="theme-head">
      <div class="theme-title"><h2>${t.name}</h2><span class="dots">${t.swatches.map((s) => `<i style="background:${s}"></i>`).join("")}</span></div>
      <p>${t.note}</p>
    </div>`,
  )
  .join("");

const rows = buttons
  .map(
    (b) => `
    <div class="row-label">
      <div class="row-icon"><img src="data:image/svg+xml;base64,${readFileSync(resolve(root, "icons", b.file)).toString("base64")}" width="48" height="48"></div>
      <div><h3>${b.name}</h3><p>${b.note}</p></div>
    </div>
    ${themes
      .map(
        (t) => `
      <div class="theme-cell">
        ${modes
          .map(
            (m) => `
          <div class="shot">
            <span class="mode">${modeLabel(m)}</span>
            <img src="${uri(`${t.id}-${m}-${b.id}.png`)}" alt="${b.name} button on ${t.name} ${m}">
          </div>`,
          )
          .join("")}
      </div>`,
      )
      .join("")}
  `,
  )
  .join("");

const board = `<!doctype html><html><head><meta charset="utf-8"><style>
  *{box-sizing:border-box;margin:0;padding:0}
  body{width:1600px;background:#12171c;color:#edf0f4;padding:52px 54px 38px;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",sans-serif}
  header{display:flex;align-items:flex-end;justify-content:space-between;border-bottom:1px solid #2a323b;padding-bottom:24px;margin-bottom:26px}
  .eyebrow{font:600 10.5px/1 ui-monospace,"SF Mono",Menlo,monospace;letter-spacing:.19em;color:#7fc2b0;text-transform:uppercase}
  h1{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:46px;line-height:1.06;letter-spacing:-.028em;margin-top:11px}
  h1 em{color:#7fc2b0;font-style:italic}
  .lede{max-width:440px;text-align:right;color:#97a2b1;font-size:12.5px;line-height:1.55}
  .lede strong{color:#edf0f4;font-weight:600}
  .matrix{display:grid;grid-template-columns:150px repeat(3,minmax(0,1fr));gap:14px}
  .theme-head{padding:0 4px 8px}
  .theme-title{display:flex;align-items:center;justify-content:space-between;gap:10px}
  .theme-head h2{font-family:"New York","Iowan Old Style",Georgia,serif;font-weight:500;font-size:23px;letter-spacing:-.02em}
  .dots{display:flex;gap:4px}
  .dots i{width:10px;height:10px;border-radius:3px;border:1px solid rgba(255,255,255,.16)}
  .theme-head p{font:500 8px/1.5 ui-monospace,Menlo,monospace;letter-spacing:.08em;color:#6f7a88;text-transform:uppercase;margin-top:3px}
  .row-label{display:flex;align-items:flex-start;gap:10px;padding:14px 6px 0}
  .row-icon{width:48px;height:48px;flex:none;filter:drop-shadow(0 5px 8px rgba(0,0,0,.35))}
  .row-label h3{font-size:13.5px;font-weight:650;letter-spacing:-.01em}
  .row-label p{font:500 8px/1.5 ui-monospace,Menlo,monospace;letter-spacing:.08em;color:#6f7a88;text-transform:uppercase;margin-top:4px}
  .theme-cell{background:#181f25;border:1px solid #2a333c;border-radius:14px;padding:9px;display:flex;flex-direction:column;gap:8px}
  .shot{display:grid;grid-template-columns:32px minmax(0,1fr);align-items:center;gap:8px}
  .mode{font:600 7px/1.4 ui-monospace,Menlo,monospace;letter-spacing:.1em;text-transform:uppercase;color:#8b96a5}
  .shot img{display:block;width:100%;height:auto;border-radius:7px;border:1px solid #2b353e}
  footer{display:flex;justify-content:space-between;margin-top:22px;font:500 9px/1.5 ui-monospace,Menlo,monospace;letter-spacing:.06em;color:#6f7a88}
</style></head><body>
<header>
  <div><div class="eyebrow">cardigan_money · brand mark demo</div><h1>Four C buttons, <em>three themes.</em></h1></div>
  <p class="lede"><strong>Captured from the running app.</strong> Each button replaces the 26 px brand mark. Light and dark for every current theme; only the mark changed. The Demo pill is hidden so the mark keeps its normal size.</p>
</header>
<div class="matrix">
  <div></div>${headerCells}
  ${rows}
</div>
<footer><span>mock bridge · 3× capture · only the brand mark changed</span><span>Ledger · Graphite · Editorial</span></footer>
</body></html>`;

const boardBrowser = await chromium.launch(exe ? { executablePath: exe } : {});
const boardPage = await boardBrowser.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
await boardPage.setContent(board, { waitUntil: "networkidle" });
await boardPage.waitForFunction(() => [...document.images].every((i) => i.complete && i.naturalWidth > 0));
await boardPage.waitForTimeout(300);
const height = await boardPage.evaluate(() => document.body.scrollHeight);
await boardPage.setViewportSize({ width: 1600, height: Math.ceil(height) });
await boardPage.screenshot({ path: resolve(root, "preview", "theme-demo.png"), fullPage: true });
await boardBrowser.close();
console.log("wrote preview/theme-demo.png");
