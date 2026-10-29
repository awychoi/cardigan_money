// Applies .github/repo-metadata.json to the GitHub repository About box and topics.
//
//   pnpm github:metadata --dry-run
//   gh auth login
//   pnpm github:metadata
//
// GitHub stores the About description, homepage and topics as repository metadata,
// not as files in git. A plain `git push` does not update them; this script does.
//
// Auth: either an authenticated `gh` CLI, or GITHUB_TOKEN/GH_TOKEN with
// repository Administration write access. The token is read from the environment
// and never printed.

import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const metadataPath = resolve(root, ".github/repo-metadata.json");
const dryRun = process.argv.includes("--dry-run");

function fail(message) {
  console.error(`github-metadata: ${message}`);
  process.exit(1);
}

function readMetadata() {
  let raw;
  try {
    raw = JSON.parse(readFileSync(metadataPath, "utf8"));
  } catch (error) {
    fail(`cannot read ${metadataPath}: ${error.message}`);
  }

  const description = typeof raw.description === "string" ? raw.description.trim() : "";
  if (description.length === 0 || description.length > 350) {
    fail("description must be 1-350 characters");
  }
  if (/[\r\n]/.test(description)) {
    fail("description must be a single line");
  }

  const homepage = typeof raw.homepage === "string" ? raw.homepage.trim() : "";
  if (homepage.length > 0) {
    let url;
    try {
      url = new URL(homepage);
    } catch {
      fail("homepage must be an absolute URL");
    }
    if (!["http:", "https:"].includes(url.protocol)) {
      fail("homepage must use http or https");
    }
    if (homepage.length > 255) {
      fail("homepage must be 255 characters or fewer");
    }
  }

  if (!Array.isArray(raw.topics) || raw.topics.length > 20) {
    fail("topics must be an array of at most 20 entries");
  }
  const topics = raw.topics.map((topic) => {
    if (typeof topic !== "string" || !/^[a-z0-9][a-z0-9-]{0,49}$/.test(topic)) {
      fail(`invalid topic ${JSON.stringify(topic)}; use lowercase letters, digits and dashes`);
    }
    return topic;
  });
  if (new Set(topics).size !== topics.length) {
    fail("topics must be unique");
  }

  return { description, homepage, topics };
}

function repository() {
  const explicit = process.env.GITHUB_REPOSITORY?.trim();
  if (explicit) return explicit;

  let remote;
  try {
    remote = execFileSync("git", ["remote", "get-url", "origin"], {
      cwd: root,
      encoding: "utf8",
    }).trim();
  } catch {
    fail("cannot read the origin remote; set GITHUB_REPOSITORY=owner/repo");
  }
  const match = remote
    .replace(/\.git$/, "")
    .replace(/\/$/, "")
    .match(/github\.com[:/]([^/]+\/[^/]+)$/);
  if (!match) fail(`origin is not a GitHub repository: ${remote}`);
  return match[1];
}

const metadata = readMetadata();
const repo = repository();

if (dryRun) {
  console.log(`Repository: ${repo}`);
  console.log(`Description: ${metadata.description}`);
  console.log(`Homepage:    ${metadata.homepage || "(none)"}`);
  console.log(`Topics (${metadata.topics.length}): ${metadata.topics.join(", ")}`);
  console.log("Dry run: nothing sent to GitHub.");
  process.exit(0);
}

const token = process.env.GITHUB_TOKEN?.trim() || process.env.GH_TOKEN?.trim();
const hasGh = spawnSync("gh", ["--version"], { stdio: "ignore" }).status === 0;

if (!token && !hasGh) {
  fail(
    "needs an authenticated GitHub CLI or GITHUB_TOKEN/GH_TOKEN. " +
      "Install gh (brew install gh), run gh auth login, or set a token with repository Administration write.",
  );
}

const headers = {
  Accept: "application/vnd.github+json",
  "X-GitHub-Api-Version": "2022-11-28",
  "User-Agent": "cardigan-money-metadata",
  ...(token ? { Authorization: `Bearer ${token}` } : {}),
};

async function rest(path, method, body) {
  const response = await fetch(`https://api.github.com/repos/${repo}${path}`, {
    method,
    headers: { ...headers, "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    const detail = (await response.text()).slice(0, 300);
    fail(`GitHub ${method} ${path} failed (${response.status}): ${detail}`);
  }
}

function ghApi(method, path, body) {
  const result = spawnSync(
    "gh",
    ["api", "--method", method, `repos/${repo}${path}`, "--input", "-"],
    { input: JSON.stringify(body), encoding: "utf8", stdio: ["pipe", "ignore", "pipe"] },
  );
  if (result.status !== 0) {
    fail(`gh api ${method} ${path} failed: ${(result.stderr || result.stdout).trim()}`);
  }
}

try {
  if (token) {
    await rest("", "PATCH", { description: metadata.description, homepage: metadata.homepage });
    await rest("/topics", "PUT", { names: metadata.topics });
  } else {
    ghApi("PATCH", "", { description: metadata.description, homepage: metadata.homepage });
    ghApi("PUT", "/topics", { names: metadata.topics });
  }
} catch (error) {
  fail(error.message);
}

console.log(`Updated ${repo}: description, homepage and ${metadata.topics.length} topics.`);
