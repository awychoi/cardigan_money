// Generates the cardigan_money placket mark for every bundled theme.
//
//   node scripts/build-brand-marks.mjs
//
// Writes src/assets/brand/placket-<theme>.svg (the tile-cropped app mark) and
// placket-blueprint-icon.svg (the 1024 px icon source used by `pnpm tauri icon`).
// The geometry is the chosen 04 Placket from docs/design/brand/abacus.
//
// Blueprint is the fixed default: it is the Dock icon and the fallback for any
// theme without a colorway. Keep its palette in sync with the Blueprint theme.

import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const outDir = resolve(root, "src/assets/brand");
mkdirSync(outDir, { recursive: true });

const PLACKETS = {
  blueprint: {
    tile: "#081120",
    placket: "#f4f7ff",
    button: "#081120",
    accent: "#2f5bff",
    thread: "#c45a18",
  },
  ledger: {
    tile: "#17453a",
    placket: "#f7f1e3",
    button: "#1f4c42",
    accent: "#b26a4c",
    thread: "#5a9d8c",
  },
  editorial: {
    tile: "#1b1712",
    placket: "#faf6ee",
    button: "#1b1712",
    accent: "#a0472b",
    thread: "#c97c55",
  },
  graphite: {
    tile: "#101216",
    placket: "#e9edf1",
    button: "#15191e",
    accent: "#33628f",
    thread: "#7fb0e0",
  },
};

/** The 04 Placket geometry, in the 1024 px brand grid. */
function placket({ tile, placket, button, accent, thread }, fullCanvas = false) {
  const viewBox = fullCanvas ? "0 0 1024 1024" : "100 100 824 824";
  const size = fullCanvas ? 1024 : 824;
  const holes = (cy) =>
    `<g fill="${placket}"><circle cx="491" cy="${cy - 21}" r="9"/><circle cx="533" cy="${cy - 21}" r="9"/><circle cx="491" cy="${cy + 21}" r="9"/><circle cx="533" cy="${cy + 21}" r="9"/></g>`;
  const body = [
    `<rect x="100" y="100" width="824" height="824" rx="184" fill="${tile}"/>`,
    `<path d="M 376 316 Q 376 300 392 300 L 492 348 Q 512 356 532 348 L 632 300 Q 648 300 648 316 V 620 Q 648 756 512 756 Q 376 756 376 620 Z" fill="${placket}"/>`,
    `<path d="M 404 330 L 494 362 Q 512 370 530 362 L 620 330" fill="none" stroke="${thread}" stroke-width="9" stroke-linecap="round"/>`,
    `<circle cx="512" cy="440" r="62" fill="${button}"/>`,
    holes(440),
    `<circle cx="512" cy="560" r="62" fill="${accent}"/>`,
    holes(560),
    `<circle cx="512" cy="680" r="62" fill="${button}"/>`,
    holes(680),
  ].join("\n");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="${viewBox}">\n${body}\n</svg>\n`;
}

const themeIds = readdirSync(resolve(root, "src/themes"))
  .filter((file) => file.endsWith(".theme.json"))
  .map((file) => file.replace(/\.theme\.json$/, ""))
  .sort();

const missing = themeIds.filter((id) => !PLACKETS[id]);
if (missing.length > 0) {
  console.error(`No placket colorway for: ${missing.join(", ")}`);
  console.error("Add palettes to scripts/build-brand-marks.mjs before shipping the theme.");
  process.exit(1);
}

const extra = Object.keys(PLACKETS).filter((id) => !themeIds.includes(id));
if (extra.length > 0) {
  console.warn(`Colorways without a theme file: ${extra.join(", ")}`);
}

for (const id of themeIds) {
  const file = resolve(outDir, `placket-${id}.svg`);
  writeFileSync(file, placket(PLACKETS[id]));
  console.log("wrote src/assets/brand/placket-" + id + ".svg");
}

writeFileSync(resolve(outDir, "placket-blueprint-icon.svg"), placket(PLACKETS.blueprint, true));
console.log("wrote src/assets/brand/placket-blueprint-icon.svg");
