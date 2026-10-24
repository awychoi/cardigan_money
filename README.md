# cardigan_money

A local-first macOS app for one person's credit-card balances and calendar-month spending. Tauri 2, React, TypeScript, Rust, and a SQLCipher-encrypted ledger.

> **Status: active build, not complete.** The secure shell and encrypted vault foundation are implemented. Account management, the dashboard, CSV import/export, backups, and sync are built in the phases tracked in [docs/implementation-status.md](docs/implementation-status.md). There is no live Plaid connection and no verified institution coverage. This is not distribution-ready.

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

# Rust
cd src-tauri
cargo test
cargo audit
cargo deny check
```

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

These are implemented in later phases; see [docs/implementation-status.md](docs/implementation-status.md) for what actually works today. The intended rules:

- Automatic backups are self-contained encrypted containers; restore always asks for the passphrase.
- Changing the passphrase rotates the data encryption key. Older backups require the old passphrase and are labeled; they are never silently rewritten or deleted.
- Portable exports use a separate passphrase and contain a full financial history. A plaintext CSV export is unprotected; the app warns before writing it.
- Demo mode is a separate database with synthetic data and a fixed app-managed key. It cannot open the real ledger, import real CSVs, link a provider, create backups, or export real data.

## Documentation

- [Implementation plan](implementation-plan.md)
- [Build specification](personal-cards-build-spec.md)
- [Security review](security-review.md)
- [Security policy and incident runbook](SECURITY.md)
- [Threat model](docs/threat-model.md)
- [Security architecture](docs/security-architecture.md)
- [Plaid feasibility](docs/plaid-feasibility.md)
- [Test plan](docs/test-plan.md)
- [Decision log](docs/decisions.md)
- [Implementation status](docs/implementation-status.md)

## License

Private, all rights reserved (`LicenseRef-Proprietary`). No license is granted for redistribution.
