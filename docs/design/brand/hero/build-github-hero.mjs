// Builds the GitHub social-preview hero (1280x640) for cardigan_money.
//
//   node docs/design/brand/hero/build-github-hero.mjs
//
// Writes github-social-preview.svg, github-social-preview.png (1280x640) and
// github-social-preview@2x.png (2560x1280). Upload the 1x PNG in the GitHub
// repository settings (Social preview).

import { chromium } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = import.meta.dirname;
const repo = resolve(root, "../../../..");
const appMark = readFileSync(resolve(repo, "src/assets/brand/placket-blueprint.svg")).toString("base64");
const makerMark = readFileSync(resolve(repo, "src/assets/wychoi-abacus.svg")).toString("base64");
const appMarkUri = `data:image/svg+xml;base64,${appMark}`;
const makerMarkUri = `data:image/svg+xml;base64,${makerMark}`;

const MONO = "'SF Mono', ui-monospace, Menlo, monospace";
const SANS = "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif";
const SERIF = "'Hoefler Text', 'Iowan Old Style', Palatino, Georgia, serif";

const chips = [
  ["Tauri 2", 64, 92],
  ["Rust", 168, 64],
  ["React", 244, 72],
  ["TypeScript", 328, 106],
  ["SQLCipher", 446, 104],
];

const chipMarkup = chips
  .map(
    ([label, x, w], index) => `<g>
      <rect x="${x}" y="462" width="${w}" height="34" rx="17" fill="${index === 4 ? "#1f1a18" : "#0e2040"}" stroke="${index === 4 ? "#8a5a2a" : "#29476f"}"/>
      <text x="${x + w / 2}" y="484" text-anchor="middle" font-family="${MONO}" font-size="11" font-weight="500" letter-spacing=".2" fill="${index === 4 ? "#ffb27a" : "#dce6fb"}">${label}</text>
    </g>`,
  )
  .join("\n");

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="640" viewBox="0 0 1280 640" role="img" aria-label="cardigan_money — local-first, encrypted personal finance for macOS">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#070d1a"/>
      <stop offset=".55" stop-color="#081426"/>
      <stop offset="1" stop-color="#0a1730"/>
    </linearGradient>
    <radialGradient id="blue-glow" cx=".16" cy=".18" r=".55">
      <stop offset="0" stop-color="#2f5bff" stop-opacity=".30"/>
      <stop offset="1" stop-color="#2f5bff" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="ember" cx=".06" cy=".96" r=".5">
      <stop offset="0" stop-color="#ff9a5a" stop-opacity=".10"/>
      <stop offset="1" stop-color="#ff9a5a" stop-opacity="0"/>
    </radialGradient>
    <filter id="icon-shadow" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="18" stdDeviation="24" flood-color="#000000" flood-opacity=".55"/>
    </filter>
    <filter id="card-shadow" x="-30%" y="-30%" width="160%" height="160%">
      <feDropShadow dx="0" dy="16" stdDeviation="26" flood-color="#000000" flood-opacity=".45"/>
    </filter>
  </defs>

  <rect width="1280" height="640" fill="url(#bg)"/>
  <rect width="1280" height="640" fill="url(#blue-glow)"/>
  <rect width="1280" height="640" fill="url(#ember)"/>

  <g stroke="#7fb0e0" stroke-width="1" opacity=".035">
    ${Array.from({ length: 20 }, (_, i) => `<path d="M ${i * 64} 0 V 640"/>`).join("")}
    ${Array.from({ length: 10 }, (_, i) => `<path d="M 0 ${i * 64} H 1280"/>`).join("")}
  </g>
  <circle cx="1088" cy="320" r="330" fill="none" stroke="#29476f" stroke-width="1" opacity=".45"/>
  <circle cx="1088" cy="320" r="400" fill="none" stroke="#29476f" stroke-width="1" opacity=".2"/>
  <path d="M 64 600 H 1216" stroke="#29476f" stroke-width="1" opacity=".55"/>
  <path d="M 968 64 V 576" stroke="#29476f" stroke-width="1" opacity=".65"/>

  <!-- App mark: Blueprint placket on a dark plate -->
  <g filter="url(#icon-shadow)">
    <rect x="64" y="64" width="240" height="240" rx="56" fill="#0e2040" stroke="#29476f" stroke-width="1"/>
  </g>
  <rect x="64" y="64" width="240" height="240" rx="56" fill="none" stroke="#5b8dff" opacity=".16"/>
  <image href="${appMarkUri}" x="84" y="84" width="200" height="200"/>

  <!-- Name and promise -->
  <text x="336" y="154" font-family="${MONO}" font-size="56" font-weight="500" letter-spacing="-2.2" fill="#f4f7ff">cardigan<tspan fill="#5b8dff">_</tspan>money</text>
  <text x="338" y="196" font-family="${SANS}" font-size="20" fill="#c6d2e6">Local-first, encrypted personal finance for macOS.</text>
  <path d="M 338 226 H 900" stroke="#29476f" stroke-width="1"/>

  <text x="338" y="282" font-family="${SERIF}" font-size="27" letter-spacing="-.4" fill="#f4f7ff">Private by design. <tspan fill="#5b8dff">Local by default</tspan><tspan fill="#ff9a5a">.</tspan></text>
  <text x="338" y="314" font-family="${SANS}" font-size="13.5" fill="#a9b9d8">Your cards, balances and spending stay on this Mac.</text>
  <text x="338" y="338" font-family="${SANS}" font-size="13.5" fill="#a9b9d8">No cloud · No account · No telemetry.</text>
  <text x="338" y="374" font-family="${MONO}" font-size="10" letter-spacing="1.7" fill="#7fb0e0">SQLCIPHER-ENCRYPTED · ARGON2ID · LOCAL-ONLY</text>

  <!-- Stack -->
  <text x="64" y="446" font-family="${MONO}" font-size="10" letter-spacing="2.2" fill="#7fb0e0">BUILT WITH</text>
  ${chipMarkup}
  <text x="64" y="574" font-family="${MONO}" font-size="10.5" fill="#6f7c90">github.com/awychoi/cardigan_money</text>

  <!-- Maker card -->
  <g filter="url(#card-shadow)">
    <rect x="1000" y="64" width="216" height="512" rx="24" fill="#f4f7ff"/>
  </g>
  <image href="${makerMarkUri}" x="1056" y="96" width="104" height="104"/>
  <rect x="1056" y="232" width="48" height="3" rx="1.5" fill="#c45a18"/>
  <text x="1056" y="270" font-family="${MONO}" font-size="10" letter-spacing="2.4" fill="#2f5bff">MADE BY</text>
  <text x="1056" y="302" font-family="${SERIF}" font-size="24" letter-spacing="-.3" fill="#081120">Wai Yin Choi</text>
  <text x="1056" y="330" font-family="${MONO}" font-size="12" fill="#2447d8">wychoi.com</text>
  <path d="M 1056 336 H 1130" stroke="#2447d8" stroke-width="1" opacity=".5"/>
  <text x="1056" y="358" font-family="${MONO}" font-size="9.5" letter-spacing=".4" fill="#53627a">NEW YORK</text>
  <path d="M 1056 392 H 1160" stroke="#d5deec" stroke-width="1"/>
  <text x="1056" y="420" font-family="${SANS}" font-size="11.5" fill="#53627a">A personal project.</text>
  <text x="1056" y="440" font-family="${SANS}" font-size="11.5" fill="#53627a">Privacy is architecture.</text>

  <text x="1056" y="548" font-family="${MONO}" font-size="9" letter-spacing="1.2" fill="#53627a">MADE ON A MAC</text>
</svg>
`;

writeFileSync(resolve(root, "github-social-preview.svg"), svg);
console.log("wrote github-social-preview.svg");

const browser = await chromium.launch(
  process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {},
);
for (const [scale, name] of [
  [1, "github-social-preview.png"],
  [2, "github-social-preview@2x.png"],
]) {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 640 },
    deviceScaleFactor: scale,
  });
  await page.setContent(
    `<!doctype html><html><head><style>html,body{margin:0;padding:0;background:#070d1a;overflow:hidden}svg{display:block;width:1280px;height:640px}</style></head><body>${svg}</body></html>`,
    { waitUntil: "networkidle" },
  );
  await page.waitForFunction(() => [...document.images].every((image) => image.complete));
  await page.waitForTimeout(200);
  await page.screenshot({ path: resolve(root, name) });
  await page.close();
  console.log("wrote", name);
}
await browser.close();
