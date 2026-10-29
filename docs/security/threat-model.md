# Threat model: cardigan_money

Status: Phase 0. Derived from [personal-cards-build-spec.md](../README.md#retired-documents) §13.1 and the pre-implementation review in [security-review.md](design-review-2026-10-23.md). This document describes the intended design and the control that is supposed to enforce it. Implementation status is tracked separately in [implementation-status.md](../reference/implementation-status.md); do not read this model as a claim that every control has already been tested.

## 1. What the app is

A single-user, local-first macOS desktop application for one person's credit-card balances and calendar-month spending. Tauri 2 shell, React renderer, Rust services, SQLCipher ledger. No server, no accounts service, no telemetry, no inbound listener, no multi-user boundary.

## 2. Assets, in priority order

| Asset | Why it matters | Where it may exist |
| --- | --- | --- |
| Full ledger history, balances, account masks | Complete financial profile useful for fraud and social engineering | SQLCipher database, encrypted WAL, encrypted backups, plaintext CSV exports, renderer memory while unlocked |
| Vault passphrase | Unlocks the ledger and all backups created before a rotation; cannot be revoked out of old backups | Owner's memory; transiently in native AppKit input and Rust memory |
| DEK / KEK | The cryptographic crown jewels | Rust memory only, while unlocked; DEK wrapped in `vault.header` |
| Plaid access tokens | Persistent read access to linked accounts | macOS Keychain only |
| Plaid client secret | Mints link tokens and exchanges public tokens | macOS Keychain only |
| Hosted Link URL and link token | Short-lived bearer material during linking | Rust memory; the URL is passed to the system browser and may remain in browser history |
| CSV exports and backup containers | Copies outside the app's control | Owner-chosen filesystem locations |
| Security event history | Detect repeated unlock failures, unexpected exports, integrity failures | Encrypted database |

## 3. Adversaries

| Adversary | Capability | In scope? |
| --- | --- | --- |
| Opportunistic local user / shoulder surfer | Uses an unlocked or hidden window, copies files, screenshots | Yes |
| Thief with the disk or a backup only | Offline brute force against the vault/backup | Yes; passphrase strength and Argon2id are the defense |
| Malicious website or local server | Attacks the webview, navigation, dev server, links | Yes |
| Malicious imported file or provider payload | Malformed CSV/JSON, formula injection, parser DoS, HTML injection | Yes |
| Malicious or compromised dependency | Runs at build time or in the renderer; attempts to capture secrets | Yes; supply-chain controls |
| Network attacker | Intercepts or redirects provider traffic | Yes; TLS, host allowlist, no redirects |
| OAuth/callback hijacker | Races or replays a callback | Yes; the selected Plaid flow has no app callback |
| Second app instance | Opens the database concurrently or races key changes | Yes; single-instance lock |
| Malware running as the same macOS user | Reads process memory, Keychain prompts, files; can wait for unlock | **Partially.** Cannot be fully defended; documented residual risk |
| Root/kernel attacker, physical DRAM attack, coerced owner | Reads anything | **No** — out of scope, not claimed |
| Compromised Plaid or bank | Upstream breach | **No** — trust boundary; minimize held data |

## 4. Trust boundaries

```text
UNTRUSTED                         VALIDATED BOUNDARY                  TRUSTED (minimized)
┌──────────────────────┐         ┌───────────────────────────┐       ┌─────────────────────────────┐
│ React renderer       │  IPC    │ Tauri command gateway     │       │ Rust services               │
│ - displays data      │────────>│ - single main window      │──────>│ - domain rules              │
│ - never sees secrets │         │ - typed commands, bounds  │       │ - vault session (DEK)       │
│ - no network         │<────────│ - rate limits, safe errors│       │ - SQLCipher DB              │
│ - no filesystem      │  data   │ - no generic SQL/file/URL │       │ - Keychain (provider only)  │
└──────────────────────┘         └───────────────────────────┘       │ - network egress allowlist  │
       ^                                                             │ - native dialogs + I/O      │
       │ native secure sheet (passphrase never crosses this line)    └─────────────────────────────┘
   AppKit NSSecureTextField
```

- The renderer is untrusted even though it is locally bundled. npm dependencies, HTML injection, and future features run there.
- The passphrase, backup/export passwords, and Plaid client secret are collected by a native AppKit sheet built in Rust. The renderer receives only `unlocked`, `failed`, or `cancelled`.
- Native file dialogs and file I/O are opened from Rust. The renderer receives opaque job IDs, counts, and sanitized errors — never a path.
- All network calls are Rust-only with a fixed host allowlist. The release CSP has no HTTPS source at all. The only other outbound action is the fixed maker link `https://wychoi.com`, opened in the system browser by a Rust command that accepts no renderer URL and sends no app data.
- The Keychain holds provider tokens and the Plaid client secret only. It never holds the passphrase, DEK, or KEK.
- Demo mode is a separate database and code path with a fixed synthetic key; it cannot open the real ledger.

## 5. Assumptions

- macOS is current and patched; FileVault is enabled; the owner's login account is not already compromised at root level.
- The app is single-user and local; there is no multi-tenant boundary.
- Plaid responses are authenticated by TLS but still parsed and bounded as untrusted input.
- macOS Keychain and code-signing APIs behave as documented.
- The build is local, ad-hoc signed, and not distribution-ready.

## 6. Controls mapped to assets

| Threat | Primary controls |
| --- | --- |
| Stolen disk/backup | SQLCipher raw-key DB with Argon2id-wrapped DEK; authenticated header; encrypted backups; strong-passphrase policy |
| Passphrase captured by renderer compromise | Native secure input; passphrase never in renderer, IPC, logs, URL, localStorage, or Keychain |
| Token/secret leak | Keychain only (`WhenUnlockedThisDeviceOnly`, non-synchronizable); Rust-only network; redaction; secret-sentinel tests |
| Restored backup binding local provider tokens | Tokens never enter backups; restored references are random UUIDs; cross-vault reuse is an explicit choice and only binds tokens already on this Mac; a missing or revoked token falls back to reconnect or a new link |
| Plaintext ledger by misconfiguration | Fail-closed `cipher_version`/`cipher_provider`/`cipher_status`, key-first pragma, plaintext-header rejection, plain-SQLite negative test |
| Renderer exfiltration / XSS | CSP with no HTTPS source, one capability, command allowlist, no remote assets, no HTML injection, blocked navigation/new windows |
| Malicious import/export | Native dialogs, size/row/field caps, UTF-8-only streaming, duplicate review, formula-injection escaping, 0600 exports, no auto-open |
| OS lifecycle exposure | Lock on screen lock/sleep/fast-user-switch/logout/quit/manual plus idle timeout; content protection; state restoration disabled |
| Second instance corruption | Single-instance plugin before any database open |
| Backup tampering | AEAD-authenticated header, DB length + SHA-256 bound as AAD, vault-ID/schema checks, read-only validation, atomic replacement |
| Supply-chain compromise | Lockfiles/frozen installs, pinned toolchains, audits, explicit postinstall allowlist, official plugins only |
| Logs/crash/state restoration | Redacting logger off by default, no financial payloads, core dumps disabled, state restoration disabled, no third-party crash reporting |
| Plaid linking flow | Hosted Link + `/link/token/get` polling; no scheme/loopback/listener; Sandbox verified; Trial/Production wired with separate credentials and host but unverified with a real Item |

## 7. Out of scope / non-goals

The design does **not** claim protection against a compromised kernel, a root attacker, physical DRAM probing, a coerced owner, a compromised Plaid/bank, screen cameras or a compromised window server, or a malicious macOS/WebKit zero-day. It does claim to protect data at rest, backups, exports, the renderer boundary, provider credentials, and accidental disclosure through logs, screenshots, state restoration, and crash artifacts.

See [../security-review.md](design-review-2026-10-23.md) §10 and [../SECURITY.md](../../SECURITY.md) §7 for the full residual-risk list.

## 8. Review triggers

The threat model must be revisited if: the app gains a server, webhook, inbound listener, custom URL scheme, multi-user support, auto-update, cloud sync, analytics, a second window, or a change to the key hierarchy. Any of those invalidates parts of this model.
