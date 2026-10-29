// Builds the abacus-derived logo candidates in docs/design/brand/abacus/icons.
//
//   node docs/design/brand/abacus/build.mjs
//
// Every icon is a 1024x1024 SVG, path-only, on the same 824 px rounded tile as
// the rest of the brand. The geometry is derived from wychoi.com's abacus mark:
// two rods, five beads (three over two), one accented bead.

import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const iconsDir = resolve(root, "icons");
const colorDir = resolve(iconsDir, "colorways");
mkdirSync(colorDir, { recursive: true });

/* ------------------------------- palettes -------------------------------- */

const PALETTES = {
  site: {
    tile: "#07090d",
    ink: "#ffffff",
    accent: "#5b8dff",
    rod: "#515f77",
    fill: "#10151f",
    fold: "#c3cee0",
    rule: "#d7dfeb",
  },
  cardigan: {
    tile: "#17453a",
    ink: "#f2e7d5",
    accent: "#b26a4c",
    rod: "#7fc2b0",
    fill: "#1b5346",
    fold: "#cfbda0",
    rule: "#d8c8ac",
  },
  blueprint: {
    tile: "#081120",
    ink: "#f4f7ff",
    accent: "#2f5bff",
    rod: "#5c6e8c",
    fill: "#0f2138",
    fold: "#b9c8de",
    rule: "#d5deec",
  },
};

/* -------------------------------- helpers -------------------------------- */

const svg = (label, body, viewBox = "0 0 1024 1024", size = 1024) =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="${viewBox}" role="img" aria-label="${label}">\n${body}\n</svg>\n`;

const tile = (fill) => `<rect x="100" y="100" width="824" height="824" rx="184" fill="${fill}"/>`;

const rod = (y, x1, x2, color, width = 20) =>
  `<line x1="${x1}" y1="${y}" x2="${x2}" y2="${y}" stroke="${color}" stroke-width="${width}" stroke-linecap="round"/>`;

const bead = (cx, cy, r, fill) => `<circle cx="${cx}" cy="${cy}" r="${r}" fill="${fill}"/>`;

/** A bead drawn as a four-hole button. */
function button(cx, cy, r, fill, hole) {
  const o = Math.round(r * 0.35);
  const h = Math.max(6, Math.round(r * 0.13));
  return `<g>${bead(cx, cy, r, fill)}
    <circle cx="${cx - o}" cy="${cy - o}" r="${h}" fill="${hole}"/>
    <circle cx="${cx + o}" cy="${cy - o}" r="${h}" fill="${hole}"/>
    <circle cx="${cx - o}" cy="${cy + o}" r="${h}" fill="${hole}"/>
    <circle cx="${cx + o}" cy="${cy + o}" r="${h}" fill="${hole}"/>
  </g>`;
}

/* ------------------------------- concepts -------------------------------- */

/** 00 · Source — the wychoi.com abacus, redrawn on the 824 px tile. */
function sourceAbacus() {
  return [
    tile("#07090d"),
    `<rect x="228.5" y="299.5" width="566" height="402" rx="68" fill="none" stroke="#ffffff" stroke-width="39"/>`,
    rod(435, 248, 773, "#515f77", 16),
    rod(602, 248, 773, "#515f77", 16),
    bead(356, 435, 55, "#ffffff"),
    bead(510, 435, 55, "#5b8dff"),
    bead(665, 435, 55, "#ffffff"),
    bead(433, 602, 55, "#ffffff"),
    bead(588, 602, 55, "#ffffff"),
  ].join("\n");
}

/** 01 · Card abacus — the site frame becomes a credit card. */
function cardAbacus(p) {
  return [
    tile(p.tile),
    `<rect x="202" y="316" width="620" height="392" rx="64" fill="${p.fill}" stroke="${p.ink}" stroke-width="52"/>`,
    `<rect x="678" y="354" width="92" height="68" rx="14" fill="${p.rod}"/>`,
    `<path d="M 698 354 V 422 M 678 388 H 770" fill="none" stroke="${p.fill}" stroke-width="8"/>`,
    rod(492, 286, 778, p.rod, 28),
    rod(600, 286, 778, p.rod, 28),
    bead(380, 492, 56, p.ink),
    bead(512, 492, 56, p.accent),
    bead(644, 492, 56, p.ink),
    bead(446, 600, 56, p.ink),
    bead(578, 600, 56, p.ink),
  ].join("\n");
}

/** 02 · C abacus — a round C holds the rods and beads. */
function cAbacus(p) {
  return [
    tile(p.tile),
    `<path d="M 718 367 A 252 252 0 1 0 718 657" fill="none" stroke="${p.ink}" stroke-width="84" stroke-linecap="round"/>`,
    rod(448, 330, 744, p.rod, 22),
    rod(576, 330, 744, p.rod, 22),
    bead(392, 448, 50, p.ink),
    bead(512, 448, 50, p.accent),
    bead(632, 448, 50, p.ink),
    bead(452, 576, 50, p.ink),
    bead(572, 576, 50, p.ink),
  ].join("\n");
}

/** 03 · Five beads — the abacus distilled, with a ghost card outline. */
function fiveBeads(p) {
  return [
    tile(p.tile),
    `<rect x="196" y="300" width="632" height="424" rx="76" fill="none" stroke="${p.ink}" stroke-width="30" opacity="0.15"/>`,
    rod(440, 292, 732, p.rod, 28),
    rod(584, 292, 732, p.rod, 28),
    bead(380, 440, 64, p.ink),
    bead(512, 440, 64, p.accent),
    bead(644, 440, 64, p.ink),
    bead(448, 584, 64, p.ink),
    bead(576, 584, 64, p.ink),
  ].join("\n");
}

/** 04 · Placket — the abacus on its side becomes a cardigan placket. */
function placket(p) {
  const holes = (cy) => `<g fill="${p.ink}">
    <circle cx="491" cy="${cy - 21}" r="9"/>
    <circle cx="533" cy="${cy - 21}" r="9"/>
    <circle cx="491" cy="${cy + 21}" r="9"/>
    <circle cx="533" cy="${cy + 21}" r="9"/>
  </g>`;
  return [
    tile(p.tile),
    `<path d="M 376 316 Q 376 300 392 300 L 492 348 Q 512 356 532 348 L 632 300 Q 648 300 648 316 V 620 Q 648 756 512 756 Q 376 756 376 620 Z" fill="${p.ink}"/>`,
    bead(512, 440, 62, p.tile),
    holes(440),
    bead(512, 560, 62, p.accent),
    holes(560),
    bead(512, 680, 62, p.tile),
    holes(680),
  ].join("\n");
}

/** 05 · Knit abacus — the site geometry in cardigan materials. */
function knitAbacus(p) {
  return [
    tile(p.tile),
    `<rect x="232" y="316" width="560" height="392" rx="84" fill="none" stroke="${p.ink}" stroke-width="46"/>`,
    rod(456, 292, 732, p.rod, 22),
    rod(568, 292, 732, p.rod, 22),
    button(384, 456, 52, p.ink, p.tile),
    button(512, 456, 52, p.accent, p.ink),
    button(640, 456, 52, p.ink, p.tile),
    button(448, 568, 52, p.ink, p.tile),
    button(576, 568, 52, p.ink, p.tile),
  ].join("\n");
}

/** 06 · Ledger abacus — the frame becomes a folded ledger page. */
function ledgerAbacus(p) {
  return [
    tile(p.tile),
    `<path d="M 306 252 H 658 L 762 356 V 728 Q 762 772 718 772 H 306 Q 262 772 262 728 V 296 Q 262 252 306 252 Z" fill="${p.ink}"/>`,
    `<path d="M 658 252 L 762 356 H 658 Z" fill="${p.fold}"/>`,
    `<rect x="306" y="300" width="160" height="18" rx="9" fill="${p.rule}"/>`,
    rod(500, 318, 706, p.rule, 18),
    rod(610, 318, 706, p.rule, 18),
    bead(400, 500, 46, p.tile),
    bead(512, 500, 46, p.accent),
    bead(624, 500, 46, p.tile),
    bead(456, 610, 46, p.tile),
    bead(568, 610, 46, p.tile),
    `<rect x="560" y="704" width="146" height="12" rx="6" fill="${p.rule}"/>`,
  ].join("\n");
}

/* -------------------------------- output --------------------------------- */

const site = PALETTES.site;
writeFileSync(
  resolve(root, "source/wychoi-abacus.svg"),
  svg("Source abacus, wychoi.com", sourceAbacus(), "100 100 824 824", 824),
);
console.log("wrote source/wychoi-abacus.svg");

const files = {
  "01-card-abacus.svg": ["Card abacus", cardAbacus(site)],
  "02-c-abacus.svg": ["C abacus", cAbacus(site)],
  "03-five-beads.svg": ["Five beads", fiveBeads(site)],
  "04-placket.svg": ["Cardigan placket", placket(site)],
  "05-knit-abacus.svg": ["Knit abacus", knitAbacus(PALETTES.cardigan)],
  "06-ledger-abacus.svg": ["Ledger abacus", ledgerAbacus(site)],
};

for (const [file, [label, body]] of Object.entries(files)) {
  writeFileSync(resolve(iconsDir, file), svg(label, body));
  console.log("wrote icons/" + file);
}

const colorways = {
  "c-site.svg": ["C abacus, site palette", cAbacus(PALETTES.site)],
  "c-cardigan.svg": ["C abacus, cardigan palette", cAbacus(PALETTES.cardigan)],
  "c-blueprint.svg": ["C abacus, blueprint palette", cAbacus(PALETTES.blueprint)],
  "card-site.svg": ["Card abacus, site palette", cardAbacus(PALETTES.site)],
  "card-cardigan.svg": ["Card abacus, cardigan palette", cardAbacus(PALETTES.cardigan)],
  "card-blueprint.svg": ["Card abacus, blueprint palette", cardAbacus(PALETTES.blueprint)],
  "placket-site.svg": ["Cardigan placket, site palette", placket(PALETTES.site)],
  "placket-cardigan.svg": ["Cardigan placket, cardigan palette", placket(PALETTES.cardigan)],
  "placket-blueprint.svg": ["Cardigan placket, blueprint palette", placket(PALETTES.blueprint)],
};

for (const [file, [label, body]] of Object.entries(colorways)) {
  writeFileSync(resolve(colorDir, file), svg(label, body));
  console.log("wrote icons/colorways/" + file);
}
