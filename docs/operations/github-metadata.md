# GitHub repository metadata

The repository **About** description, homepage and **topics** are GitHub metadata. They are not
files in the git tree, and `git push` does not update them. This repo keeps the desired values in
[`.github/repo-metadata.json`](../../.github/repo-metadata.json) and applies them with a local
script.

## Apply locally

```bash
brew install gh          # once, if the GitHub CLI is not installed
gh auth login            # once per machine
pnpm github:metadata     # applies description, homepage and the full topic set
pnpm github:metadata -- --dry-run   # preview without calling GitHub
```

Alternatively, use a token instead of the CLI:

```bash
GITHUB_TOKEN=... pnpm github:metadata
```

A classic token needs the `repo` scope. A fine-grained token needs **Administration: read and
write** for this repository. The script reads the token from the environment and never prints it.

Verify:

```bash
gh repo view --json description,homepageUrl,repositoryTopics
```

## Rules the script enforces

- Description: one line, 1–350 characters.
- Homepage: optional absolute `http`/`https` URL, 255 characters or fewer.
- Topics: at most 20, unique, lowercase letters/digits/dashes, 50 characters each. The script
  **replaces** the topic set, so removing an entry from the JSON removes it from GitHub.

## What cannot be managed this way

- **Social preview image:** GitHub has no public API for it. Upload
  [`hero/github-social-preview.png`](../design/brand/hero/github-social-preview.png) in
  **Settings → Social preview**.
- **Pinned repositories, Discussions settings, and other repository settings:** use the GitHub UI
  or a broader settings tool.

## Automation

The default GitHub Actions `GITHUB_TOKEN` cannot edit repository settings. To apply this file on
every push automatically, add a PAT secret with **Administration: write** (for example
`REPO_METADATA_TOKEN`) and a workflow that runs `pnpm github:metadata` on changes to
`.github/repo-metadata.json`. It is deliberately not enabled by default: it requires a long-lived
PAT, while the local command is one step and has no stored secret.
