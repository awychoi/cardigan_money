# GitHub repository metadata

The repository **About** description, homepage and **topics** are GitHub metadata, not files in the
git tree. A plain `git push` does not update them. This repo uses the hosted
[Settings app](https://github.com/apps/settings) to sync them from
[`.github/settings.yml`](../../.github/settings.yml) whenever that file is pushed to the default
branch.

## Setup

1. Install the Settings app on this repository:
   <https://github.com/apps/settings/installations/new>
2. Edit [`.github/settings.yml`](../../.github/settings.yml) and push to `main`.
3. The app applies `repository.description`, `repository.homepage` and replaces the full topic set.

Verify from a machine with the GitHub CLI:

```bash
gh repo view --json description,homepageUrl,repositoryTopics
```

No token or workflow secret is needed in this repo: auth lives in the installed GitHub App.

## Format notes

- `topics` must be a comma-separated **string**. The app splits it on commas; a YAML list will not
  work.
- The app replaces the topic set, so removing an entry from the file removes it from GitHub.
- The file is applied from the **default branch**; pushes to other branches do not sync.
- Other repository settings are untouched unless added under `repository:`.

## Security

The Settings app is a third-party GitHub App and needs admin access to the repository. Anyone with
push access can change repository settings by editing this file. For a solo repository that is
acceptable; for shared repositories, add a `CODEOWNERS` entry for `.github/settings.yml` and require
code-owner review on the default branch before enabling the app.

## What is still manual

- **Social preview image:** GitHub has no public API for it. Upload
  [`hero/github-social-preview.png`](../design/brand/hero/github-social-preview.png) in
  **Settings → Social preview**.
- **Pinned repositories, Discussions settings, and similar:** use the GitHub UI.

## Alternative without a third-party app

A GitHub Actions workflow can run the same API calls, but the default `GITHUB_TOKEN` cannot edit
repository settings. It requires a PAT secret with **Administration: write** (or a GitHub App
installation token). The Settings app avoids storing a long-lived token at the cost of installing a
third-party app.
