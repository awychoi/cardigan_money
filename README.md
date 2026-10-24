# cardigan_money

A local-first macOS app for one person's credit-card balances and calendar-month spending. Tauri 2, React, TypeScript, Rust, and a SQLCipher-encrypted ledger.

> **Status: Phase 10 local release candidate (offline/demo/CSV scope, Plaid Sandbox verified).** Encrypted vault, dashboard, account management, edits/rules/manual metadata, bounded CSV import/export, self-contained encrypted backups/restore, portable exports, crypto-erase, sync engine/scheduler, and hardening are implemented. Plaid **Sandbox** linking/sync is verified against Sandbox Items; **Production/live institutions remain disabled and unverified**. This is an ad-hoc-signed local build, not distribution-ready.

## What it is

- Calendar months organize spending. Statement cycles do not.
- Current balances and upcoming obligations are computed independently of the selected spending month.
- Pending, posted, refunds, payments, fees, and transfers are distinct.
- Missing, stale, unsupported, and explicitly zero are different states.
- The ledger is encrypted at rest and unlocks only with the owner's passphrase. The passphrase is entered in a native macOS sheet and never enters the webview.
- Demo data and CSV work without any bank connection.

## Security warning (read this)

- **There is no passphrase recovery.** If you lose the passphrase and the passphrase for every backup, the data is unrecoverable. No one can reset it.
- `vault.header` is required. Losing every copy makes the ledger unreadable even if the database file survives. Self-contained backups mitigate this.
- Do not store the passphrase in the repository, an environment file, chat, or a note kept with the backups.
- The app locks on launch, manual Lock Now, screen lock, sleep, fast user switching, logout, quit, and idle timeout. Hiding the window is not a lock.
- This is an ad-hoc-signed local build. There is no auto-update or notarization. Verify source and artifact hash before running a rebuild.
- See [SECURITY.md](SECURITY.md) for the incident runbook and the full residual-risk statement.

## Prerequisites

- macOS on Apple Silicon (developed/tested on an M1 Max)
- Xcode command-line tools (`xcode-select --install`)
- Rust stable (the repo pins `stable` via `rust-toolchain.toml`)
- Node.js 22.12+ and pnpm 12 (the repo pins `packageManager`)
- No system SQLCipher installation is required; it is built from the vendored crate

## Development commands

```bash
# install frontend dependencies (lockfile is committed)
pnpm install --frozen-lockfile

# native desktop app (builds Rust + starts Vite on 127.0.0.1:1420)
pnpm tauri dev

# browser-only mock (no Rust) for UI work
VITE_MOCK_BRIDGE=1 pnpm dev

# tests
pnpm test                 # Vitest + React Testing Library
pnpm test:e2e             # Playwright against the mock bridge
pnpm typecheck
pnpm lint
pnpm theme:check          # validate every theme: schema + contrast
pnpm theme:schema         # regenerate the editor JSON Schema

# Rust
cd src-tauri
cargo test
cargo audit
cargo deny check
cargo test --test perf -- --ignored --nocapture   # 50k dataset measurement
```

## Theming

The UI is driven by a theme document: one JSON file per theme in `src/themes/`, validated against
`src/themes/theme.schema.json`. A theme sets design tokens (color, type, radii, spacing, motion,
card art) and four component variants: sidebar or top navigation, card faces or data tiles,
comfortable or compact density, full or calm motion. Themes are data — adding one needs no code
change, and the picker in **Settings → Appearance** is generated from the files.

- Full field reference, recipes, and limits: [docs/design/theming.md](docs/design/theming.md)
- Add a theme: copy a file in `src/themes/`, set `id` to match the file name, run `pnpm theme:check`
- Dark/light follows the existing appearance setting; every theme ships both palettes

This checkout was prepared in a sandbox that allows writes only inside the repository, so `scripts/dev-env.sh` points `CARGO_HOME`, `RUSTUP_HOME`, the pnpm store, and the Playwright browser cache at `.toolchain/` and `.cache/`. Source it if your environment has the same restriction:

```bash
. scripts/dev-env.sh
CARDIGAN_MONEY_APP_DATA_DIR="$PWD/.cache/appdata" pnpm tauri dev
```

`CARDIGAN_MONEY_APP_DATA_DIR` is a **debug-build-only** test hook. Release builds ignore it and use the standard `~/Library/Application Support/com.awychoi.cardiganmoney` directory.

## Build (Apple Silicon)

```bash
pnpm tauri build
# artifact: src-tauri/target/release/bundle/macos/cardigan_money.app
codesign --force --deep --sign - "src-tauri/target/release/bundle/macos/cardigan_money.app"
codesign --verify --deep --strict "src-tauri/target/release/bundle/macos/cardigan_money.app"
shasum -a 256 "src-tauri/target/release/bundle/macos/cardigan_money.app/Contents/MacOS/cardigan_money"
```

The build is ad-hoc signed for local use only. It does not use a Developer ID, hardened runtime, notarization, or App Sandbox, and must not be represented as distribution-ready.

## Backups, passphrases, and demos

Implemented and covered by service-level tests; native dialogs remain owner smoke tests. See [backup/recovery](docs/operations/backup-recovery.md) and [native smoke](docs/operations/native-smoke.md). The rules:

- Automatic backups are self-contained encrypted containers; restore always asks for the passphrase.
- Changing the passphrase rotates the data encryption key. Older backups require the old passphrase and are labeled; they are never silently rewritten or deleted.
- Portable exports use a separate passphrase and contain a full financial history. A plaintext CSV export is unprotected; the app warns before writing it.
- Demo mode is a separate database with synthetic data and a fixed app-managed key. It cannot open the real ledger, import real CSVs, link a provider, create backups, or export real data.

## Documentation

Start at [docs/README.md](docs/README.md) — the map of plans, security, operations, and reference docs.

- [Current plan: UI overhaul](docs/plans/ui-overhaul-2026-10.md)
- [Implementation status](docs/reference/implementation-status.md) — what is built and what is still unverified
- [Security policy](SECURITY.md) · [architecture](docs/security/architecture.md) · [threat model](docs/security/threat-model.md)
- [Theming](docs/design/theming.md) · [UI direction](docs/design/proposal.md)
- [Decision log](docs/reference/decisions.md)

## Current limitations and unverified items

- Live Plaid linking, Keychain ACLs, native file dialogs/sheets, screen-capture protection, screen-lock/sleep auto-lock, and VoiceOver behavior are **not verified** in this environment; see [native smoke](docs/operations/native-smoke.md) and the blocker in [Plaid feasibility](docs/integrations/plaid-feasibility.md).
- Restore requires an existing unlocked vault; fresh-install restore is unsupported.
- The scheduled sync is mock-provider tested; no live network call was made.
- The release is ad-hoc signed, not notarized or App-Sandboxed.

## License

Private, all rights reserved (`LicenseRef-Proprietary`). No license is granted for redistribution.
