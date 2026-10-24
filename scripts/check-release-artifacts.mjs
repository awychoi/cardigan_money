#!/usr/bin/env node
// Release-bundle checks. Run after `pnpm build`.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const dist = join(root, "dist");
const failures = [];

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

let files;
try {
  files = walk(dist);
} catch {
  console.error("dist/ not found; run `pnpm build` first");
  process.exit(1);
}

for (const file of files) {
  if (!file.endsWith(".js") && !file.endsWith(".html") && !file.endsWith(".css")) continue;
  const source = readFileSync(file, "utf8");
  const relative = file.slice(root.length);
  if (source.includes("mock-bridge-sentinel-4c1f")) {
    failures.push(`${relative}: dev mock bridge shipped in production`);
  }
  if (source.includes("sourceMappingURL")) {
    failures.push(`${relative}: production source map reference found`);
  }
}

// Note: vendor bundles legitimately contain documentation/error URLs as
// strings. They are not fetch targets and are blocked by the release CSP; the
// browser test asserts that zero external requests are made.

if (failures.length) {
  console.error("release artifact checks failed:");
  for (const failure of failures) console.error(` - ${failure}`);
  process.exit(1);
}
console.log(`release artifact checks passed (${files.length} files)`);
