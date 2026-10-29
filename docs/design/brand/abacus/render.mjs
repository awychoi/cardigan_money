// Renders the abacus-alternatives boards.
//
//   node docs/design/brand/abacus/build.mjs
//   node docs/design/brand/abacus/render.mjs
//
// Uses the Playwright Chromium installed with the repo. Set CHROMIUM_PATH to
// override the binary. Writes preview/board.png and preview/colorways.png.

import { chromium } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const preview = resolve(root, "preview");
const iconDir = resolve(root, "icons");
const dataUri = (file) => `data:image/svg+xml;base64,${readFileSync(file).toString("base64")}`;

const concepts = [
  {
    file: "01-card-abacus.svg",
    n: "01",
    name: "Card abacus",
    line: "The site frame becomes a credit card: chip, two rods, five beads.",
    note: "Closest to the product and the source mark. The safest replacement for the Forest button.",
  },
  {
    file: "02-c-abacus.svg",
    n: "02",
    name: "C abacus",
    line: "A round C holds the rods; the initial does the work.",
    note: "Ties the mark to the name. The clearest of the six at 16 px.",
  },
  {
    file: "03-five-beads.svg",
    n: "03",
    name: "Five beads",
    line: "The abacus distilled: two rods, five beads, a ghost card outline.",
    note: "The most minimal: at 16 px the rods drop away and the five beads carry it. The least explicit about money.",
  },
  {
    file: "04-placket.svg",
    n: "04",
    name: "Placket",
    line: "The abacus turns on its side and becomes a cardigan’s button placket.",
    note: "The strongest story: beads become buttons. The V-neck keeps it from reading as a pill.",
  },
  {
    file: "05-knit-abacus.svg",
    n: "05",
    name: "Knit abacus",
    line: "The site geometry in today’s forest, cream and rust materials.",
    note: "The continuity option: it keeps the installed mark’s cloth while borrowing the abacus.",
  },
  {
    file: "06-ledger-abacus.svg",
    n: "06",
    name: "Ledger abacus",
    line: "A folded page turns the frame into the ledger itself.",
    note: "Quietest and most document-led. Better in a book than in the Dock.",
  },
];

const sizeSet = [64, 32, 24, 16];

function samples(file, tone) {
  return sizeSet
    .map(
      (size) => `<div class="sample">
        <img src="${dataUri(resolve(iconDir, file))}" width="${size}" height="${size}" alt="">
        <span>${size}</span>
      </div>`,
    )
    .join("");
}

function conceptCard(c) {
  return `<section class="card">
    <div class="head"><span class="num">${c.n}</span><h2>${c.name}</h2></div>
    <p class="line">${c.line}</p>
    <div class="big"><img src="${dataUri(resolve(iconDir, c.file))}" width="256" height="256" alt="${c.name}"></div>
    <div class="strips">
      <div class="strip light">${samples(c.file, "light")}</div>
      <div class="strip dark">${samples(c.file, "dark")}</div>
    </div>
    <p class="note">${c.note}</p>
  </section>`;
}

const board = `<!doctype html>
<html><head><meta charset="utf-8"><style>
  * { box-sizing: border-box; }
  body { margin: 0; background: #efece4; color: #11141b;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif; -webkit-font-smoothing: antialiased; }
  header { background: #07090d; color: #f4f7ff; padding: 54px 64px 46px; display: flex; align-items: flex-end; justify-content: space-between; gap: 40px; }
  .eyebrow { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 12px; letter-spacing: .14em; text-transform: uppercase; color: #8fa0bc; margin: 0 0 16px; }
  h1 { font-family: "Hoefler Text", "Iowan Old Style", Palatino, Georgia, serif; font-weight: 500; font-size: 58px; line-height: 1; letter-spacing: -.035em; margin: 0; }
  .sub { margin: 16px 0 0; color: #c6d2e6; font-size: 17px; line-height: 1.5; max-width: 720px; }
  .source { display: flex; align-items: center; gap: 16px; flex: none; }
  .source .badge { display: block; background: #ffffff; padding: 7px; border-radius: 22px; }
  .source img { width: 84px; height: 84px; border-radius: 15px; display: block; }
  .source div { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 11px; line-height: 1.5; letter-spacing: .08em; text-transform: uppercase; color: #8fa0bc; }
  .source strong { display: block; color: #f4f7ff; font-weight: 500; }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 26px; padding: 34px 64px 0; }
  .card { background: #fff; border: 1px solid #ddd6c7; border-radius: 18px; padding: 24px 24px 20px; display: flex; flex-direction: column; }
  .head { display: flex; align-items: baseline; gap: 10px; }
  .num { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 12px; color: #8a94a4; }
  h2 { font-size: 24px; font-weight: 600; letter-spacing: -.025em; margin: 0; }
  .line { margin: 7px 0 0; color: #4c5666; font-size: 14.5px; line-height: 1.45; min-height: 42px; }
  .big { display: flex; justify-content: center; padding: 14px 0 12px; }
  .big img { width: 256px; height: 256px; display: block; }
  .strips { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .strip { display: flex; align-items: flex-end; justify-content: space-between; gap: 8px; padding: 12px 12px 8px; border-radius: 12px; }
  .strip.light { background: #f5f2ea; border: 1px solid #e6dfd1; }
  .strip.dark { background: #07090d; border: 1px solid #07090d; }
  .sample { display: flex; flex-direction: column; align-items: center; gap: 5px; }
  .sample img { display: block; }
  .sample span { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 9px; letter-spacing: .04em; color: #93a0b2; }
  .strip.light .sample span { color: #8b8171; }
  .note { margin: 14px 0 0; padding-top: 12px; border-top: 1px solid #eee8dc; color: #6a7484; font-size: 12.5px; line-height: 1.45; }
  footer { margin: 30px 64px 0; padding: 24px 0 34px; border-top: 1px solid #d8d0c0; display: flex; align-items: flex-start; justify-content: space-between; gap: 40px; }
  .picks { font-size: 14px; line-height: 1.6; color: #3f4958; max-width: 900px; }
  .picks strong { color: #11141b; }
  .fine { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 10.5px; line-height: 1.6; letter-spacing: .04em; color: #8a8171; text-align: right; }
</style></head>
<body>
  <header>
    <div>
      <p class="eyebrow">cardigan_money · brand exploration · 28 Oct 2026</p>
      <h1>Six ways to carry the abacus.</h1>
      <p class="sub">Each direction carries the personal mark’s DNA — a rounded frame or two rails,
      beads in a three-over-two rhythm, one accented bead — into the app. The placket trades the
      frame for a cardigan and five beads for three buttons. Shown in the site’s own black, white
      and blue, except 05, which carries the installed forest, cream and rust.</p>
    </div>
    <div class="source">
      <span class="badge"><img src="${dataUri(resolve(root, "source/wychoi-abacus.svg"))}" alt="Wychoi.com abacus logo"></span>
      <div><strong>Source</strong>wychoi.com<br>#07090d · #fff<br>#5b8dff</div>
    </div>
  </header>

  <main class="grid">
    ${concepts.map(conceptCard).join("\n")}
  </main>

  <footer>
    <p class="picks"><strong>If I had to pick one:</strong> <strong>02 C abacus</strong> is the
    strongest stand-alone mark — the name is in it, the abacus is in it, and it is the clearest
    at 16 px. <strong>01 Card abacus</strong> is the best product mark
    when the icon has room to explain itself; <strong>04 Placket</strong> is the one people will
    remember; <strong>03 Five beads</strong> is the quiet, modern alternative.</p>
    <p class="fine">1024 × 1024 SVGs<br>824 px tile · 184 px radius<br>flat, path-only</p>
  </footer>
</body></html>`;

/* ------------------------------- colorways ------------------------------- */

const colorwaySets = [
  {
    id: "site",
    label: "Site",
    note: "Black, white, #5b8dff",
    chips: ["#07090d", "#ffffff", "#5b8dff"],
    c: "c-site.svg",
    card: "card-site.svg",
    placket: "placket-site.svg",
  },
  {
    id: "cardigan",
    label: "Cardigan",
    note: "Forest, cream, rust",
    chips: ["#17453a", "#f2e7d5", "#b26a4c"],
    c: "c-cardigan.svg",
    card: "card-cardigan.svg",
    placket: "placket-cardigan.svg",
  },
  {
    id: "blueprint",
    label: "Blueprint",
    note: "Navy, paper, electric blue",
    chips: ["#081120", "#f4f7ff", "#2f5bff"],
    c: "c-blueprint.svg",
    card: "card-blueprint.svg",
    placket: "placket-blueprint.svg",
  },
];

const colorwayPanel = (set) => `<section class="cw">
  <div class="cw-head"><h2>${set.label}</h2><span>${set.note}</span></div>
  <div class="cw-row">
    <figure><img src="${dataUri(resolve(iconDir, "colorways", set.c))}" width="142" height="142" alt="C abacus, ${set.label}"><figcaption>02 C</figcaption></figure>
    <figure><img src="${dataUri(resolve(iconDir, "colorways", set.card))}" width="142" height="142" alt="Card abacus, ${set.label}"><figcaption>01 Card</figcaption></figure>
    <figure><img src="${dataUri(resolve(iconDir, "colorways", set.placket))}" width="142" height="142" alt="Placket, ${set.label}"><figcaption>04 Placket</figcaption></figure>
  </div>
  <div class="chips">${set.chips.map((c) => `<span><i style="background:${c}"></i>${c}</span>`).join("")}</div>
</section>`;

const colorways = `<!doctype html>
<html><head><meta charset="utf-8"><style>
  * { box-sizing: border-box; }
  body { margin: 0; background: #efece4; color: #11141b;
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif; -webkit-font-smoothing: antialiased; }
  header { background: #07090d; color: #f4f7ff; padding: 50px 64px 42px; }
  .eyebrow { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 12px; letter-spacing: .14em; text-transform: uppercase; color: #8fa0bc; margin: 0 0 14px; }
  h1 { font-family: "Hoefler Text", "Iowan Old Style", Palatino, Georgia, serif; font-weight: 500; font-size: 50px; line-height: 1; letter-spacing: -.035em; margin: 0; }
  .sub { margin: 14px 0 0; color: #c6d2e6; font-size: 16px; line-height: 1.5; max-width: 900px; }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 26px; padding: 34px 64px 0; }
  .cw { background: #fff; border: 1px solid #ddd6c7; border-radius: 18px; padding: 22px; }
  .cw-head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; border-bottom: 1px solid #eee8dc; padding-bottom: 12px; }
  .cw-head h2 { margin: 0; font-size: 22px; letter-spacing: -.02em; }
  .cw-head span { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 10.5px; color: #8a94a4; text-align: right; }
  .cw-row { display: flex; justify-content: space-between; gap: 8px; padding: 16px 0 10px; }
  figure { margin: 0; display: flex; flex-direction: column; align-items: center; gap: 8px; }
  figure img { width: 142px; height: 142px; display: block; }
  figcaption { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 10px; letter-spacing: .05em; text-transform: uppercase; color: #6a7484; }
  .chips { display: flex; gap: 8px; flex-wrap: wrap; border-top: 1px solid #eee8dc; padding-top: 12px; }
  .chips span { display: inline-flex; align-items: center; gap: 6px; font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 10px; color: #6a7484; }
  .chips i { width: 12px; height: 12px; border-radius: 3px; border: 1px solid #00000022; }
  footer { margin: 26px 64px 0; padding: 20px 0 32px; border-top: 1px solid #d8d0c0; color: #3f4958; font-size: 13.5px; line-height: 1.6; }
</style></head>
<body>
  <header>
    <p class="eyebrow">cardigan_money · abacus alternatives · colorways</p>
    <h1>The final three, in three palettes.</h1>
    <p class="sub">C is the stand-alone mark, Card is the product mark, Placket is the story. The site palette
    is the literal inheritance; Cardigan keeps the installed app’s cloth; Blueprint matches the new theme.
    The geometry does not change — only the frame, the beads and the one accent.</p>
  </header>
  <main class="grid">${colorwaySets.map(colorwayPanel).join("\n")}</main>
  <footer><strong>C</strong> is the safest at 16 px. <strong>Card</strong> explains the product best
  once the icon has room. <strong>Placket</strong> is the most distinctive. If the app mark should
  survive a theme switch, either pin one palette or keep the frame constant and let the accent follow
  the theme.</footer>
</body></html>`;

/* -------------------------------- render --------------------------------- */

const exe = process.env.CHROMIUM_PATH;
const browser = await chromium.launch(exe ? { executablePath: exe } : {});

async function shoot(html, file, width) {
  const page = await browser.newPage({ viewport: { width, height: 900 }, deviceScaleFactor: 2 });
  await page.setContent(html, { waitUntil: "networkidle" });
  await page.waitForFunction(() =>
    [...document.images].every((image) => image.complete && image.naturalWidth > 0),
  );
  await page.waitForTimeout(250);
  const height = await page.evaluate(() => document.body.scrollHeight);
  await page.setViewportSize({ width, height: Math.ceil(height) });
  await page.waitForTimeout(120);
  await page.screenshot({ path: resolve(preview, file), fullPage: true });
  await page.close();
  console.log("wrote preview/" + file);
}

await shoot(board, "board.png", 1680);
await shoot(colorways, "colorways.png", 1680);
await browser.close();
