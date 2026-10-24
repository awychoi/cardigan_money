# Security review: cardigan_money

Review date: 2026-10-23
Reviewer role: cyber-security engineer (architecture and threat review)
Scope: [personal-cards-build-spec.md](personal-cards-build-spec.md) and [implementation-plan.md](implementation-plan.md). No application code exists yet, so this is a pre-implementation design review.
Status: findings recorded; both documents amended. The specification remains the source of truth; this review records why the amendments exist and what "done" means for security.

Note on tense: the findings quote the **pre-amendment** documents. Where a finding says "the plan says" or "the spec says," read it as the version reviewed. The amendments are summarized in §7 of this review and are normative in the current specification and plan.

---

## 1. Verdict

The plan is substantially better than a typical local-first finance prototype. It already commits to an encrypted ledger, an in-memory key, Keychain-only provider tokens, no telemetry, least-privilege Tauri capabilities, encrypted backups, no destructive sync behavior, and honest reporting of unverified provider coverage. Those are the right instincts.

It is **not watertight as written**. There is one critical fail-open risk, seven high-severity gaps, and a set of medium-severity operational gaps. The most important are:

1. **Encryption can fail open silently.** Nothing in the plan proves at runtime that the linked library is actually SQLCipher. A misconfigured `rusqlite` build can make `PRAGMA key` a no-op and create a plaintext database while the UI says "encrypted." The current "spike" is not a release gate.
2. **The passphrase is collected in the webview.** The plan treats the renderer as trusted for the one secret that unlocks the entire ledger and every backup. One XSS, one malicious npm dependency, or one unsafe HTML render converts a renderer bug into permanent disclosure of all historical financial data.
3. **The key hierarchy is underspecified and partly circular.** `vault_metadata` is described as an encrypted-table home for KDF parameters, but KDF parameters are needed before the encrypted database can be opened. Implemented literally, this cannot work; teams under pressure improvise (hard-coded keys, passphrase retention, plaintext sidecars).
4. **Automatic backups contradict "no extra prompt."** "Encrypt every automatic backup with the owner's current passphrase" and "do not prompt" can only be true if the passphrase is retained indefinitely or the backup key is reused without a restore story. Neither is specified.
5. **The lock lifecycle leaves decrypted data available.** The plan explicitly declines auto-lock and lets "hide" keep the process unlocked and the scheduler running. A hidden, unlocked window is a shoulder-surfing and unattended-access finding.
6. **The Plaid OAuth story is not implementable as implied.** Plaid redirect URIs must be HTTPS; custom URI schemes are unsupported; `localhost` redirects are allowed only in Sandbox. Any plan that assumes a `cardigan_money://` or loopback callback for Production will fail or be redesigned under pressure. The plan needs a verified desktop flow (Hosted Link + polling, or a hosted HTTPS redirect page), or live linking must be explicitly unavailable.
7. **The renderer has no enforced network/privilege boundary.** "Restrictive CSP" and "least-privilege capabilities" are asserted but not specified. Tauri custom commands are allowed to all windows by default; a renderer compromise can invoke every registered command unless the command surface is constrained.
8. **Security is deferred to Phase 10.** Threat modeling, CSP, secret handling, and security tests must be Phase 0–2 concerns, not a hardening pass at the end.

The amendments make the plan enforce the controls rather than merely mention them. "Watertight" is not achievable for a general-purpose macOS app; Section 10 states the residual risks plainly so the owner is not sold a false guarantee.

---

## 2. Method and sources

I reviewed both documents line by line and checked the security-relevant platform claims against current official documentation. No application behavior was tested because no code exists. The review covers architecture, data flow, trust boundaries, cryptographic key management, local storage, IPC, provider integration, build/supply chain, and verification.

Sources used:

- Tauri security model and trust boundaries: <https://v2.tauri.app/security/>
- Tauri CSP behavior: <https://v2.tauri.app/security/csp/>
- Tauri capabilities (including that registered app commands are allowed to all windows by default): <https://v2.tauri.app/security/capabilities/>
- Tauri application lifecycle threats: <https://v2.tauri.app/security/lifecycle/>
- Tauri configuration (`freezePrototype`, `withGlobalTauri`, asset protocol): <https://v2.tauri.app/reference/config/>
- SQLCipher API (`PRAGMA key` raw key, `cipher_memory_security`, `cipher_integrity_check`, `rekey`, KDF): <https://www.zetetic.net/sqlcipher/sqlcipher-api/>
- Plaid OAuth redirect rules (HTTPS only; custom schemes unsupported; localhost only in Sandbox): <https://plaid.com/docs/link/oauth/>
- Plaid Link Webview SDK (deprecated): <https://plaid.com/docs/link/webview/>
- Plaid Hosted Link and `/link/token/get` polling: <https://plaid.com/docs/link/hosted-link/>
- macOS window content protection: <https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.set_content_protected>
- age format: <https://github.com/C2SP/C2SP/blob/main/age.md>
- OWASP ASVS: <https://owasp.org/www-project-application-security-verification-standard/>

---

## 3. Assets, adversaries, and assumptions

### 3.1 Assets (in priority order)

| Asset | Why it matters | Where it lives |
| --- | --- | --- |
| Ledger history, balances, account masks | Complete financial profile; useful for fraud, social engineering, and extortion | SQLCipher DB, backups, exports, renderer memory |
| Vault passphrase | Unlocks the ledger and all backups; cannot be rotated out of old backups | Owner's memory; transient native/Rust memory |
| Data encryption key (DEK) and key-encryption key (KEK) | The actual cryptographic crown jewels | Rust memory only while unlocked |
| Plaid access tokens | Persistent read access to the owner's accounts | macOS Keychain |
| Plaid client secret | Mints link tokens and exchanges public tokens | macOS Keychain |
| Short-lived link/hosted-link artifacts | Bearer material during linking | Rust memory; possibly browser history if placed in a URL |
| Exported CSV / portable backups | Plaintext or independently encrypted copies outside the app's control | User-chosen filesystem locations |
| Security event history | Detect tampering, repeated unlock failures, unexpected exports | Encrypted DB |

### 3.2 Adversaries

| Adversary | Capability | In scope? |
| --- | --- | --- |
| Opportunistic local user / shoulder surfer | Uses an unlocked or hidden window, copies files, takes screenshots | Yes |
| Malware running as the owner (same UID) | Reads process memory, Keychain prompts, files, injects into renderer; can wait for unlock | Partially; cannot be fully defended. Documented residual risk. |
| Thief with the disk/backup only | Offline brute force against the vault or a backup | Yes; KDF and passphrase strength are the defense |
| Malicious website / local server | Attacks the webview, navigation, OAuth callback, dev server | Yes |
| Malicious imported file / provider payload | Malformed CSV/JSON, HTML injection, formula injection, parser DoS | Yes |
| Malicious or compromised dependency | Executes in build or renderer; steals passphrase/tokens | Yes; supply-chain controls |
| Network attacker | Intercepts or redirects provider traffic | Yes; TLS, host allowlist, no redirects |
| OAuth callback hijacker | Registers the app's scheme/port, races the callback | Yes; the chosen Plaid flow must not rely on custom schemes |
| Second app instance | Opens the DB concurrently, causes corruption or key confusion | Yes; single-instance lock |
| Root/kernel attacker, physical DRAM attack | Reads anything | No; explicitly out of scope |
| Plaid or bank compromise | Data breach upstream | No; contract/trust boundary, but minimize data held |

### 3.3 Trust boundaries

```text
UNTRUSTED                         VALIDATED BOUNDARY                 TRUSTED (minimized)
┌──────────────────────┐         ┌──────────────────────────┐       ┌─────────────────────────────┐
│ React renderer       │  IPC    │ Tauri command gateway     │       │ Rust services               │
│ - displays data      │────────>│ - window/origin check     │──────>│ - domain rules              │
│ - never sees secrets │         │ - schema + bounds         │       │ - vault session (DEK)       │
│ - no network         │<────────│ - allowlist + rate limit  │       │ - SQLCipher DB              │
│ - no filesystem      │  data   │ - sanitized errors        │       │ - Keychain (tokens only)    │
└──────────────────────┘         └──────────────────────────┘       │ - network egress allowlist  │
                                                                     └─────────────────────────────┘
```

The renderer is treated as an **untrusted presentation layer**. It is not a security boundary merely because it is bundled locally: npm dependencies, HTML injection, and future features all execute there. The passphrase, DEK, KEK, provider tokens, and client secret never enter it.

### 3.4 Assumptions

- macOS is current and patched; FileVault is enabled; the owner's login account is not already compromised by root-level malware.
- The app is a single-user local application; there is no server, no multi-tenant boundary, and no remote API surface.
- Plaid is trusted to return authentic data, but every Plaid response is still validated as untrusted input.
- macOS Keychain and the OS code-signing/security APIs behave as documented.
- The build is local and unsigned or ad-hoc signed; distribution signing/notarization remains out of scope, with documented consequences.

### 3.5 Non-goals

The design does not claim to protect against a compromised kernel, a root attacker, physical DRAM probing, a coerced owner, a compromised Plaid/bank, or a malicious macOS/WebKit zero-day. It does claim to protect data at rest, backups, exports, the renderer boundary, provider credentials, and accidental disclosure through logs, screenshots, state restoration, and crash artifacts.

---

## 4. Findings

Severity definitions: **Critical** blocks any release because the core security promise can fail silently; **High** must be fixed before the relevant feature ships; **Medium** must be fixed before release or explicitly accepted by the owner with a written rationale; **Low** should be fixed but is not release-blocking.

| ID | Severity | Finding | Required change | Spec location |
| --- | --- | --- | --- | --- |
| SEC-01 | Critical | Encryption can fail open. The plan says "verify SQLCipher" but has no runtime fail-closed check. A plain-SQLite build, a wrong feature flag, or a connection opened before `PRAGMA key` can create a plaintext DB while the app claims encryption. | At startup and on every connection: require non-empty `cipher_version`/`cipher_provider`, `cipher_status = 1`, `PRAGMA key` before any other statement, and a negative test that a plain SQLite client cannot open the file. Refuse to create or open a ledger if the check fails. Record the provider/version/check result in `vault.header` and audit metadata. | §13.2, §16 |
| SEC-02 | High | The passphrase is entered in the webview. The spec says the session key never enters the renderer, but the passphrase does. XSS or a compromised dependency captures it and can decrypt every backup offline, forever. | Collect passphrases, backup/export passwords, and the Plaid client secret through a native secure input sheet (AppKit `NSSecureTextField`). The renderer receives only success/failure. If a native prompt is genuinely impossible, stop and require an explicit owner risk acceptance; do not silently fall back to an HTML input. | §11, §13.3 |
| SEC-03 | High | Key hierarchy is underspecified/circular. KDF parameters are listed as `vault_metadata` table content, but the table is inside the encrypted DB, so it cannot be read before unlock. Automatic-backup key handling is equally undefined. | Adopt envelope encryption: random 256-bit DEK, Argon2id-derived KEK, XChaCha20-Poly1305 wrap, external authenticated `vault.header` (0600) containing KDF params/salt/wrapped DEK/vault ID, SQLCipher opened with the raw DEK. Keep only audit metadata inside the DB. | §4, §5, §13.2 |
| SEC-04 | High | Backup encryption contradicts "no prompt." Automatic backups cannot be encrypted with a passphrase that is never retained without either retaining the passphrase or reusing an unspecified key. Restore/key-rotation semantics are missing. | Use a self-contained snapshot container: SQLCipher DB encrypted with the current DEK + header carrying the wrapped DEK and KDF parameters. Automatic backups need no prompt; restore always asks for the passphrase. Rotate the DEK on passphrase change so old backups require the old passphrase. | §14, §13.8 |
| SEC-05 | High | No mandatory lock lifecycle. "No auto-lock; locks on quit" plus "hide keeps scheduler running" leaves decrypted data accessible to anyone at the Mac, and the renderer may retain it indefinitely. | Mandatory manual Lock Now; mandatory lock on screen lock, sleep, fast user switching, and logout; idle auto-lock default 15 minutes (configurable, with warning if disabled); lock clears renderer caches and stops sync. Hiding is not a security boundary. | §12, §13.3 |
| SEC-06 | High | Plaid OAuth flow is not implementable as implied. Plaid requires HTTPS redirect URIs; custom URI schemes are unsupported; localhost is allowed only in Sandbox. The plan's "state/nonces, validate callbacks, close temporary listeners" implies a callback flow that does not exist for this app. | Verify a desktop flow in Phase 0: Hosted Link with `/link/token/get` polling, or a hosted HTTPS redirect page. Do not use a custom scheme or loopback callback in Production. If neither flow is approved, live linking is unavailable; do not add a server or fake the flow. | §6, §9, §13.6 |
| SEC-07 | High | Renderer privilege/network boundary is not enforced. Tauri registers custom commands for all windows by default; "restrictive CSP" and "least-privilege capabilities" are not specified, and no navigation/new-window blocking is required. | One `main` capability; no fs/shell/http/opener/sql/store/process/deep-link plugins; custom command allowlist with window/origin validation; CSP with no HTTPS sources, only IPC; block navigation/new windows; `freezePrototype`, `withGlobalTauri: false`, asset protocol off, devtools off in release. | §13.4, §13.5 |
| SEC-08 | Medium-High | Keychain handling is underspecified, and the unsigned-build decision destabilizes Keychain ACLs. Tokens could be stored with the wrong accessibility class or synced to iCloud. | `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, `kSecAttrSynchronizable=false`, data-protection Keychain, one item per connection, delete on disconnect; record the code-signing identity and its effect on Keychain; use a stable local signing identity for development. | §4, §13.3, §13.10 |
| SEC-09 | High (process) | Security is deferred to Phase 10. Threat modeling, CSP, secret handling, and security tests are absent from the early phases where they are cheapest to fix. | Put threat model + crypto design + Tauri baseline in Phase 0; native secret input + lock + fail-closed vault in Phase 2; security tests in every phase; independent review before release. | Plan §4–§14 |
| SEC-10 | Medium | No supply-chain controls. No lockfile/frozen-install requirement, no `cargo audit`/`pnpm audit`, no postinstall-script policy, no restriction on unofficial plugins. | Commit lockfiles; `--frozen-lockfile`/`--locked`; `cargo deny` + `cargo audit`; `pnpm audit`; `ignore-scripts` with an allowlist; official Tauri plugins only; pin toolchains; record artifact hashes. | §13.10, Plan §4/§14 |
| SEC-11 | Medium | Logs, crash dumps, and macOS state restoration can leak decrypted data. The plan says "never log" but does not define the logger, redaction, core-dump policy, or state-restoration settings. | Redacting logger with no financial payloads; no persistent logs by default; `RLIMIT_CORE=0`; panic hook; disable window state restoration and enable secure restorable state; no third-party crash reporting; test with sentinel secrets. | §13.7, §16 |
| SEC-12 | Medium | CSV import/export is not bounded or user-warned. Large files can DoS; exported CSV is plaintext; formula injection is mentioned but not fully specified; filesystem paths may cross IPC. | Native file dialogs and file I/O in Rust; size/row/field caps; streaming; UTF-8 only; formula-injection escaping for `= + - @`, tab, CR; 0600 export; explicit "plaintext export" warning; no auto-open; fuzz tests. | §10, §13.9 |
| SEC-13 | Medium | Demo mode uses an app-managed (likely hard-coded) key and could receive real imports. | Demo DB is separate, ephemeral, fixed-key by design, never backed by the real path; disable CSV import, provider linking, backups, and exports in demo; persistent label; tests that demo cannot open the real DB. | §15, §13.11 |
| SEC-14 | Medium | No single-instance/DB lock. Two instances can open the same DB or race key changes, risking corruption or confusing failures. | Single-instance plugin or exclusive file lock; second instance focuses the first and exits; no DB open before the lock is held. | §13.7 |
| SEC-15 | Medium | Restoring an untrusted backup opens a parser attack surface and could execute schema objects. | Restore opens the candidate read-only first with `trusted_schema=OFF`, `SQLITE_DBCONFIG_DEFENSIVE`, `query_only`; validates magic/header/AEAD/DB ID/schema/integrity; caps size; never runs triggers/views from the candidate; keeps a pre-restore backup. | §14, §13.8 |
| SEC-16 | Medium | No security audit trail or incident response. There is no way to see repeated unlock failures, unexpected exports, or a failed integrity check, and no runbook for a leaked token/passphrase. | Encrypted `security_events` table (no secrets); incident runbook covering passphrase compromise, token leak, lost backup, corruption, and dependency compromise; Plaid Item revocation steps. | §7, §13.12, §13.13 |
| SEC-17 | Medium | Unsigned build and no auto-update create tampering and stale-binary risk. | Ad-hoc sign at minimum; record SHA-256; verify the artifact; document that there is no auto-update and that rebuilding is the update path; recommend a stable local signing identity. Do not call it distribution-ready. | §1.2, §13.10 |
| SEC-18 | Medium | Same-user malware can read the unlocked process; the plan does not state this limitation. | Do not claim protection against a compromised account. Use `mlock`/zeroize best-effort, short unlock sessions, and lock lifecycle; document the residual risk plainly. | §13.14 |
| SEC-19 | Low-Medium | No explicit file-permission/symlink/atomic-write requirements. | 0700 dirs / 0600 files; reject symlinks; `O_NOFOLLOW`; atomic temp+fsync+rename; validate app-data path ownership; test. | §13.7 |
| SEC-20 | Low-Medium | IPC inputs are not bounded and errors may leak. | `deny_unknown_fields`, length/range caps, enum validation, no raw SQL/paths/URLs, sanitized error codes, rate limits, fuzz tests. | §13.4 |

---

## 5. Detailed critique of the highest-risk areas

### 5.1 Fail-open encryption is the worst possible failure

The application's central claim is "no plaintext ledger." SQLCipher is a SQLite extension, and the API is deliberately compatible: if the library is plain SQLite, `PRAGMA key` is accepted as an unknown pragma and the database is created in plaintext. A build that accidentally links `libsqlite3` instead of SQLCipher, or a connection that issues `PRAGMA foreign_keys=ON` before `PRAGMA key`, can produce a plaintext file that the app then happily uses. The current plan's "spike" tests one build, not the shipped binary.

The fix is not a documentation note. It is a startup invariant and a test suite:

- At process start, before any window opens, assert `PRAGMA cipher_version` is non-empty and `PRAGMA cipher_provider` is expected.
- On every new connection, set the key first, then `PRAGMA cipher_status` (or `cipher_integrity_check` plus a read) and fail closed on any error.
- Verify the file header is not `SQLite format 3\0` and that a plain `sqlite3` client cannot open it.
- Run a negative build test: intentionally link plain SQLite in a test-only binary and prove the app refuses to start.
- Record the cipher/provider/version in the encrypted DB and in `vault.header` for support and audit.
- Never auto-create a ledger at a path where a file exists but cannot be decrypted.

This is finding SEC-01 and a release blocker.

### 5.2 The passphrase must not be typed into the webview

The spec says the session key never enters the renderer, but the unlock form necessarily hands the passphrase to the renderer before it crosses IPC. React state, devtools (if ever enabled), a malicious dependency, an HTML-injection bug in a future feature, or a compromised `npm` package can read it. Unlike a stolen session token, a stolen passphrase is permanent: it decrypts every backup, including backups the attacker copied months ago. The same is true for backup/export passwords and the Plaid client secret.

A native secure input sheet is the correct boundary:

- Use AppKit `NSSecureTextField` (via `objc2-app-kit`/`objc2-foundation`) in a modal sheet or alert accessory view.
- The passphrase exists in Rust memory as a `Zeroizing<String>`/`SecretString`, is passed directly to the KDF, and is zeroized immediately after the KEK is derived.
- The renderer receives only `unlocked`/`failed`/`cancelled`, never the value.
- Use the same mechanism for passphrase creation/confirmation, passphrase change, restore, portable-export password, and Plaid client-secret entry.
- If the native sheet cannot be built, stop and ask the owner to accept the renderer risk in writing; do not quietly ship an HTML password field.

This is finding SEC-02. It is the single biggest reduction in blast radius available in this design.

### 5.3 The vault header cannot live only inside the encrypted database

The current data model says `vault_metadata` stores "cipher/schema version, KDF parameters, creation and passphrase-rotation timestamps." If that table is inside the SQLCipher database, the app needs the KDF parameters to derive the key that opens the database that contains the KDF parameters. That is circular. The likely improvisations are hard-coded KDF parameters (weaker), storing the passphrase (forbidden), or a plaintext sidecar created ad hoc (unreviewed format).

The correct design is a small external header and envelope encryption:

```text
owner passphrase ──Argon2id(salt, params)──> KEK (32 bytes)
KEK ──XChaCha20-Poly1305 unwrap──> DEK (32 random bytes)
DEK ──SQLCipher raw key (x'...')──> encrypted ledger

vault.header (0600, outside the DB, authenticated, redundant copy):
  format_version, vault_id, kdf=argon2id, m_cost/t_cost/p_cost,
  salt, wrap_nonce, wrapped_dek, generation, created_at, schema_version
```

The header is not secret (the wrapped DEK is useless without the passphrase), but it must be integrity-protected and rollback-detected. It must be backed up redundantly; losing it loses the ledger even if the DB file survives. Backups must embed their own copy of the header so they are self-contained.

This is finding SEC-03. It also resolves the automatic-backup problem: a backup can copy the current header without needing the passphrase, while the DB snapshot is encrypted with the DEK.

### 5.4 "Automatic backups, no prompt" needs a key hierarchy, not a retained passphrase

The spec says automatic backups are encrypted with the owner's current passphrase and "because the vault is unlocked when the app runs, do not prompt for an extra password." That only works if the app retains the passphrase in memory for the whole session, which increases exposure, or if it reuses the SQLCipher key in an undocumented way. The restore path then has no stated way to distinguish an old-passphrase backup from a new-passphrase backup.

The envelope design solves it cleanly:

- The backup DB snapshot is encrypted with the current DEK (no prompt).
- The backup header embeds the current wrapped-DEK header (no prompt).
- Restore asks for the passphrase, derives the KEK with the backup header's salt/params, unwraps the DEK, and opens the snapshot.
- On passphrase change, generate a new DEK, rekey the live DB with the SQLCipher rekey API using the new raw DEK, write a new header, and keep old backups as-is. Old backups still carry the old wrapped DEK and require the old passphrase, exactly as the spec warns.
- Crash safety requires a pre-rekey encrypted backup and a small rotation journal so a crash between rekey and header replacement can be recovered deterministically.

This is finding SEC-04.

### 5.5 The lock lifecycle is part of the threat model, not a UX preference

The spec says "version one unlocks at launch and locks on quit" and "no auto-lock," while close-to-hide keeps the process and scheduler alive. That means a Mac left on a desk, a hidden window, a screen-sharing session, or a family member using the machine can see the full ledger. Locking the Mac screen does not lock the vault in the current design.

The minimum defensible lifecycle:

- Manual **Lock Now** (menu item and keyboard shortcut).
- Automatic lock on macOS screen lock, sleep, fast user switching, logout, and app quit.
- Idle auto-lock, default 15 minutes, configurable 1–240 minutes or disabled with a warning.
- On lock: close DB connections, drop and zeroize the session, clear renderer query caches and sensitive state, show the lock screen, and cancel any in-flight sync at a safe point without advancing the cursor.
- Hiding the window is not a lock; the idle timer continues while hidden. If the owner wants scheduled sync while hidden, they must accept that the vault can remain unlocked until the idle timeout; the setting must explain the tradeoff.
- The scheduler skips when locked and records why. Scheduled syncs are best-effort, as the spec already says.

This is finding SEC-05. It changes an explicit owner decision; it is recorded as a security-review revision in the spec, and the owner can override it only with a written risk acceptance.

### 5.6 The Plaid flow as written cannot be built

The plan says to use state/nonces, validate callbacks, constrain origins, and close temporary listeners. That is sound advice for a web app with a redirect URI, but this is a local macOS app. Plaid's current documented rules are:

- Redirect URIs must use HTTPS.
- Custom URI schemes are not supported in any environment.
- `localhost` redirect URIs are allowed only in Sandbox.
- The Link Webview SDK is deprecated (and requires redirect handling for Chase).

So a `cardigan_money://oauth` deep link or a `127.0.0.1` loopback listener is not a Production Plaid OAuth flow. The plan must choose one of:

1. **Hosted Link + polling.** Create a link token in Rust with `hosted_link: {}`, open the returned `hosted_link_url` in the system browser, and poll `/link/token/get` for `SESSION_FINISHED`/`public_token` (the endpoint exposes session results for six hours). This keeps all API secrets in Rust, avoids a callback into the app, and avoids hosting a redirect page. It does place a bearer URL in browser history for its short lifetime; treat it as a secret and never log it. Whether Plaid approves this for a desktop app must be verified, not assumed.
2. **Hosted HTTPS redirect page.** Host a static page that Plaid redirects to and that hands control back to the app. That page is external infrastructure and must be documented as "what leaves the Mac." It must not receive or store access tokens. The app still needs to retrieve the public token; a redirect page without a server generally cannot do that securely unless it is paired with Hosted Link or a webhook.
3. **No live linking.** Ship the full offline/CSV app and show live linking as unavailable with the concrete blocker, as the spec already permits.

What must not happen: inventing a custom scheme, binding a local HTTP listener for Production, embedding the Plaid secret in the renderer, or putting a `link_token`/access token in a URL that is logged or persisted. This is finding SEC-06.

### 5.7 The renderer is not a trust boundary, and Tauri does not make it one

Tauri's own documentation is explicit: Rust code is trusted; WebView code is not, and the IPC layer is the boundary. The plan says "least-privilege capabilities" but does not enumerate the dangerous plugins; it says "restrictive CSP" but does not give a policy; it says "validate IPC" but not the command surface.

The required baseline:

- One capability for the `main` window. No `fs`, `shell`, `http`, `opener`, `sql`, `store`, `process`, `updater`, `deep-link`, `websocket`, `upload`, `clipboard`, or `notification` permissions. Use custom Rust commands for file dialogs and network I/O so the renderer never receives arbitrary paths or URLs.
- A command inventory with typed request/response structs, `deny_unknown_fields`, length/range bounds, and a window-label/origin guard. No generic `query`, `read_file`, `write_file`, `open_url`, `http_request`, or `execute` command.
- File import/export commands open the native dialog in Rust and return an opaque job ID, not a path. The renderer gets counts and sanitized errors.
- CSP with no HTTPS source at all: `default-src 'none'; connect-src ipc: http://ipc.localhost; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; frame-src 'none'; worker-src 'none'; manifest-src 'none'`. If a style nonce can replace `'unsafe-inline'`, do that.
- Block navigation to anything but the app origin and deny new windows. Disable asset protocol, `withGlobalTauri`, devtools in release, and enable `freezePrototype`.
- Treat every merchant/description/CSV field as untrusted text. No `dangerouslySetInnerHTML`, no markdown-to-HTML, no `innerHTML`, no `eval`/`new Function`. Strip control and bidi characters for display.
- Disable drag-and-drop file handling in the webview; imports go through the native picker.

This is finding SEC-07.

### 5.8 Keychain, signing, and the unsigned-build decision

The spec stores provider tokens in the macOS Keychain, which is correct. But the security of a Keychain item depends on the item's accessibility class and the app's code identity. An unsigned/ad-hoc build does not have a stable identity across rebuilds; Keychain ACL behavior can become prompt-heavy or unreliable. The review does not require notarization, but it does require:

- `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, `kSecAttrSynchronizable=false`, data-protection Keychain, and one item per connection.
- A stable local signing identity (self-signed certificate) for development, or explicit documentation that every rebuild may re-prompt for Keychain access.
- No fallback to a file if Keychain access fails; fail closed with a clear message.
- Delete the Keychain item on disconnect, and provide a "remove all provider credentials" action on uninstall/quit.
- Never include Keychain values in backups or exports; restored connections require relinking.

This is finding SEC-08.

### 5.9 Security must be a phase gate, not a Phase 10 cleanup

The plan currently puts "security review" in Phase 10. By then, the crypto format, IPC surface, and UI secret handling are expensive to change. The amended plan moves:

- Phase 0: threat model, crypto design, SQLCipher fail-closed spike, Tauri baseline, dependency audit, Plaid flow verification.
- Phase 1: command gateway, CSP, capabilities, navigation blocking, redaction, devtools off.
- Phase 2: native secret input, envelope key hierarchy, lock lifecycle, file permissions, single instance.
- Every later phase: the security tests relevant to its feature.
- Phase 10: independent review, adversarial tests, artifact signing/hash, incident runbook, final residual-risk statement.

This is finding SEC-09.

---

## 6. Target architecture (security view)

### 6.1 Process and trust boundaries

```text
┌──────────────────────────── macOS ────────────────────────────┐
│                                                              │
│  ┌───────────── WebView (untrusted) ─────────────┐            │
│  │ React UI, no network, no filesystem, no secrets│            │
│  └───────────────────┬───────────────────────────┘            │
│                      │ typed IPC, validated, bounded          │
│  ┌───────────────────▼───────────────────────────┐            │
│  │ Rust command gateway                          │            │
│  │  - window/origin check, allowlist, rate limit │            │
│  │  - opaque file jobs, sanitized errors         │            │
│  └───────┬───────────────────────┬───────────────┘            │
│          │                       │                            │
│  ┌───────▼─────────┐     ┌───────▼────────────────┐           │
│  │ Vault session   │     │ Provider services      │           │
│  │ KEK -> DEK      │     │ TLS, host allowlist,   │           │
│  │ mlock/zeroize   │     │ no redirects, caps     │           │
│  └───────┬─────────┘     └───────┬────────────────┘           │
│          │                       │                            │
│  ┌───────▼─────────┐     ┌───────▼────────────────┐           │
│  │ SQLCipher DB    │     │ macOS Keychain         │           │
│  │ + WAL (encrypted)│     │ tokens only, ThisDevice│           │
│  └─────────────────┘     └────────────────────────┘           │
│                                                              │
│  App data dir 0700, files 0600, no symlinks, atomic writes   │
└──────────────────────────────────────────────────────────────┘
```

### 6.2 Vault key hierarchy

```text
passphrase (native secure input, never in WebView)
    │ Argon2id(salt, m_cost, t_cost, p_cost)
    ▼
KEK (32 bytes, zeroized after unwrap)
    │ XChaCha20-Poly1305 unwrap (AAD binds header fields)
    ▼
DEK (32 bytes, random at vault creation, mlock'd while unlocked)
    ├── SQLCipher raw key ──> live DB + WAL
    ├── SQLCipher raw key ──> backup snapshot
    └── wrapped under export KEK ──> portable export
```

- `vault.header` is outside the DB, 0600, atomic, with a mirrored copy of the current generation. It contains no secret that is usable without the passphrase.
- `vault_metadata` **inside** the DB contains only audit data: vault ID, schema version, cipher/provider version, created/rotated timestamps, generation. KDF parameters live in the header, not the encrypted table.
- Passphrase change: pre-change backup → new DEK → SQLCipher raw-key rekey → new header/generation → verify old passphrase fails and new succeeds. A rotation journal makes a crash recoverable.
- A lost header is as fatal as a lost passphrase unless a self-contained backup exists. State this in the UI and README.

### 6.3 Secret handling

- Native secure input for passphrases and the Plaid client secret.
- `secrecy`/`zeroize` wrapper types; no `Debug`, `Clone`, or `Serialize` on secrets.
- `mlock` the DEK/KEK pages where practical; `cipher_memory_security=ON`; `temp_store=MEMORY`; no plaintext temp files.
- Core dumps disabled (`RLIMIT_CORE=0`); panic hook with no data; no third-party crash reporting; no persistent logs by default.
- Tokens loaded from Keychain only for the duration of a request and zeroized after.
- No secret in env vars, `VITE_*` variables, URLs, localStorage, IndexedDB, logs, or error messages.

### 6.4 Renderer and IPC

- Renderer never receives the passphrase, DEK, KEK, access token, client secret, or raw provider payload.
- Renderer never receives arbitrary filesystem paths or URLs. Native commands own dialogs and I/O.
- No network from the renderer; CSP has no HTTPS source. All network is Rust with a fixed host allowlist.
- On lock, clear query caches, unmount sensitive views, clear search/filter state, and navigate to the lock screen. JS garbage collection is not zeroization; document that.
- No HTML injection; all provider/CSV text rendered as text; control/bidi characters stripped for display.
- Mock bridge is development/test only, behind a compile-time feature, and cannot be selected in a release build.

### 6.5 Network and Plaid

- `reqwest` with TLS verification, HTTPS-only, redirects disabled, connect/read timeouts, response-size cap, decompression cap, and a fixed host allowlist (`production.plaid.com`, `sandbox.plaid.com` only; no user-controlled URLs).
- No automatic billable refresh endpoints; no webhooks; no inbound listener.
- Plaid flow: Hosted Link + `/link/token/get` polling, or a verified hosted HTTPS redirect page. No custom scheme or Production loopback callback. Any hosted component is documented: what is hosted, what data crosses it, and why it is necessary.
- Link tokens are bearer secrets: in Rust memory only, never logged, never persisted, never placed in a URL the app controls. Hosted Link URLs are opened directly in the system browser; their lifetime is minimized; browser history exposure is documented.
- Access tokens and client secret in Keychain only; token exchange in Rust; zeroize after use; revoke remote Items only after explicit confirmation.
- Provider responses are untrusted: typed parsing, schema validation, length/amount/date bounds, no raw payload retention, no provider-supplied URLs.

### 6.6 Backup and restore

- Snapshot via SQLite backup API or `sqlcipher_export` into a destination encrypted with the DEK; checkpoint WAL first; verify the result.
- Container: authenticated header (magic, format version, vault ID, KDF params/salt, wrapped DEK, generation, schema version, kind, created_at, DB length, DB ciphertext SHA-256) + SQLCipher DB bytes. Header AEAD is bound as AAD to all fields.
- Restore: native file picker + native passphrase prompt; parse/cap header; verify DB length/hash; derive/unwrap; open read-only with `trusted_schema=OFF`, defensive mode, `query_only`; verify vault ID, `cipher_integrity_check`, `integrity_check`, schema compatibility; make a pre-restore backup; acquire exclusive lock; atomically replace DB and remove stale WAL/SHM; fsync; reopen; on failure keep the old DB.
- Automatic backups need no passphrase prompt because the header already wraps the DEK; restore always requires the passphrase.
- Passphrase change rotates the DEK and rekeys; old backups require the old passphrase; do not silently rewrite or delete them.
- Portable export uses a separate passphrase and the same authenticated container/key-wrapping design (or an age outer layer over a self-contained container). Never a plaintext SQLite file; never provider tokens.
- Crypto-erasure: deleting the vault destroys the wrapped DEK and all backups; ciphertext remnants are unrecoverable without the DEK. Document SSD remanence limits.

### 6.7 Lock lifecycle

```text
launch ──> locked (no DB, no key)
           │ native secure passphrase prompt
           ▼
        unlocked (DEK in mlocked memory, DB open)
           │ manual lock / idle timeout / screen lock / sleep / fast user switch / logout / quit
           ▼
        locked (connections closed, DEK/KEK zeroized, renderer caches cleared)
```

Scheduled sync is skipped while locked and records why. There is no background unlock and no key in Keychain.

### 6.8 Import/export

- Native dialogs in Rust; renderer gets job IDs, counts, and sanitized errors.
- Import caps: file size, row count, field length, UTF-8 only, reject NUL, stream parse, no auto-open.
- Export: UTF-8 CSV, formula-injection escaping for cells beginning with `= + - @`, tab, CR; quote correctly; 0600 file; explicit plaintext warning; no account numbers in the filename; no auto-open.
- Duplicate review remains user-driven; no fuzzy deletion.

---

## 7. What changed in the documents

The specification was amended in place; the plan now schedules the work. The mapping is:

| Control area | Spec change | Plan change |
| --- | --- | --- |
| Threat model, trust boundaries, renderer untrusted | New §13.1, §2 principles, §1.2 decisions | Phase 0, §2 non-negotiables |
| Envelope encryption, external header, KDF | §4, §5, §7, §13.2 | Phase 0 crypto spike, Phase 2 |
| Native passphrase/secret input | §11, §13.3 | Phase 2, Phase 7, Phase 9 |
| Fail-closed SQLCipher | §13.2, §16 | Phase 0, Phase 2 |
| Lock lifecycle | §3, §12, §13.3 | Phase 2, Phase 4, Phase 8 |
| Backup/restore container and rotation | §14, §13.8 | Phase 7 |
| Renderer/CSP/capabilities/IPC | §13.4, §13.5 | Phase 1, Phase 3 |
| Plaid flow correction | §6, §9, §13.6 | Phase 0, Phase 9 |
| Keychain and signing | §4, §13.3, §13.10 | Phase 0, Phase 9, Phase 10 |
| Logging/crash/state restoration | §13.7 | Phase 1, Phase 2, Phase 10 |
| CSV/import/export | §10, §13.9 | Phase 6 |
| Demo isolation | §15, §13.11 | Phase 5 |
| Single instance, files, permissions | §13.7 | Phase 2 |
| Security testing and assurance | §16, §13.12 | §3, Phase 0/10 |
| Audit trail/incident response | §7, §13.13 | Phase 7, Phase 10 |
| Residual-risk honesty | §13.14, Definition of done | §16, §18 |

---

## 8. Security acceptance checklist

No phase may be reported complete while a relevant item is open. Phase 10 cannot close while any Critical/High item is unresolved without a written owner acceptance.

### 8.1 Foundation and build

- [ ] Threat model and trust boundaries recorded in `docs/threat-model.md`.
- [ ] Crypto design (KDF params, header format, AAD, rotation journal) recorded in `docs/security-architecture.md` with test vectors.
- [ ] SQLCipher fail-closed spike passes: `cipher_version` non-empty, `cipher_status=1`, plain SQLite cannot open the DB, wrong key rejected without modifying the file.
- [ ] A test-only plain-SQLite build is rejected by the app.
- [ ] Tauri CSP has no HTTPS sources; capability files contain only the `main` window and no dangerous plugins.
- [ ] `cargo audit`, `cargo deny`, `pnpm audit`, lockfile/frozen install, and postinstall policy pass.
- [ ] Plaid desktop flow is verified against current docs or explicitly reported as blocked; no custom scheme/loopback in Production.

### 8.2 Vault and lifecycle

- [ ] Native secure input for create/unlock/change/restore/export; renderer never receives the secret.
- [ ] Argon2id parameters are calibrated and bounded; weak/absurd header params are rejected without a multi-second DoS.
- [ ] `vault.header` is 0600, atomic, redundant, authenticated, rollback-detected; losing it is documented as fatal.
- [ ] Wrong passphrase returns a non-destructive error; no file creation, reset, or overwrite.
- [ ] `cipher_memory_security=ON`, `temp_store=MEMORY`, `secure_delete=ON`, `trusted_schema=OFF`, defensive mode, WAL encrypted, core dumps disabled.
- [ ] DEK/KEK zeroized on lock; DB connections closed; renderer caches cleared.
- [ ] Lock on screen lock/sleep/fast user switch/logout/manual; idle auto-lock default 15 minutes.
- [ ] Single-instance lock; second instance cannot touch the DB.

### 8.3 Renderer, IPC, and network

- [ ] Command inventory exists; no generic SQL/file/URL/shell commands; every command validates window, types, bounds, and enums.
- [ ] File import/export and backup/restore paths never cross IPC; native dialogs own them.
- [ ] Navigation to non-app origins blocked; new windows denied; devtools disabled in release; `freezePrototype` on; `withGlobalTauri` off; asset protocol off.
- [ ] CSP enforced and tested: zero external network requests; `connect-src` limited to IPC.
- [ ] No `dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function`, or remote assets.
- [ ] Network egress only from Rust, fixed Plaid hosts, no redirects, TLS verified, response caps.
- [ ] No token/secret in renderer state, URL, localStorage, logs, or errors.

### 8.4 Backups, exports, and recovery

- [ ] Backup snapshots are consistent (WAL checkpoint/backup API), encrypted, verified, 0600, and never plaintext.
- [ ] Container header is authenticated; DB length/hash binding; tampering, truncation, DB swapping, wrong vault ID, downgrade, and wrong passphrase fail safely.
- [ ] Restore is read-only/validated first; pre-restore backup; atomic replacement; failure leaves old DB intact.
- [ ] Passphrase change rotates the DEK; old passphrase fails; old backups are labeled and require the old passphrase.
- [ ] Portable export uses a separate strong password, contains no provider secrets, and warns that it is a full financial history.
- [ ] Crypto-erase/delete-all path destroys the wrapped DEK and backups; residual-risk wording is honest.
- [ ] No plaintext temp database or decrypted export exists during any normal path.

### 8.5 Provider and incident readiness

- [ ] Plaid client secret and access tokens are Keychain-only with correct accessibility attributes.
- [ ] Link tokens are Rust-only, short-lived, never logged/persisted; Hosted Link URL exposure is documented.
- [ ] Remote Item removal only after explicit confirmation; local delete does not silently revoke.
- [ ] Security events record unlock failures, lock, backup, restore, export, rekey, credential add/remove, integrity failure — no secrets.
- [ ] Incident runbook covers passphrase compromise, token leak, lost backup, corrupt DB, dependency compromise, and stolen Mac.
- [ ] Final handoff states what is protected, what is not, and which claims were actually tested.

---

## 9. Verification that must accompany the release

At minimum, the following evidence must be recorded in `docs/implementation-status.md` and the security test report:

1. Exact SQLCipher/provider/version, rusqlite feature flags, and the fail-closed test output.
2. Argon2id parameters and measured unlock time on the M1 Max; confirmation that header parameter tampering is bounded.
3. Proof (test output) that a plain SQLite client cannot open the ledger, WAL, or backup.
4. Proof that a wrong passphrase leaves the ledger intact and does not create/rename/overwrite it.
5. A CSP/network test showing no external requests from the renderer.
6. A secret-leak scan: sentinel passphrase/token absent from logs, DOM, localStorage, URLs, error messages, and crash artifacts.
7. Backup tamper/truncation/wrong-passphrase/restore tests, including a nonempty database.
8. Plaid flow evidence: what was verified in Sandbox, what was verified with authorized accounts, and what remains unverified.
9. Native smoke tests for Keychain, screen capture protection, screen-lock auto-lock, state restoration, and devtools.
10. Dependency audit output and artifact SHA-256.

---

## 10. Residual risks (do not hide these)

Even after the amendments, the following remain true:

- **A compromised macOS account can read the unlocked process.** `mlock` and zeroization reduce exposure but do not defeat same-user malware, a debugger with sufficient privilege, or a compromised kernel. The app protects data at rest and casual access, not a hostile OS session.
- **Screen capture is not fully preventable.** Content protection blocks the standard capture path; cameras, some remote-support tools, or a compromised window server may not be blocked. The owner should still lock the screen.
- **The passphrase is the single point of failure.** A weak or reused passphrase defeats Argon2id; a lost passphrase is unrecoverable. Backups help only if their passphrase is known.
- **Old backups remain decryptable with the old passphrase.** That is intentional and required for recovery, but a compromised old passphrase compromises old backups. The owner must delete them after a suspected compromise.
- **Exported CSV is plaintext.** The app can warn, set 0600, and sanitize formulas, but it cannot control where the owner copies the file, cloud sync, or later sharing.
- **Hosted Link puts a short-lived bearer URL in the browser.** The app can minimize lifetime and avoid logging, but browser history/extensions are outside its control. This is a documented tradeoff of the only viable desktop flow.
- **Unsigned/ad-hoc builds can be tampered with.** There is no auto-update and no notarization; the owner must verify the artifact hash and source. A stable local signing identity improves Keychain behavior but is not a distribution trust chain.
- **A compromised renderer can still ask the user to unlock.** Native input prevents silent passphrase capture, not post-unlock abuse through allowed commands. CSP, no remote content, and the command allowlist are the defenses.
- **TLS depends on the macOS trust store/proxy.** The app does not pin certificates; a compromised trust store or privileged proxy can intercept provider traffic even with TLS verification.
- **A full header+DB rollback is not detectable.** Replacing both with an older consistent pair rolls the ledger back; the app detects mismatches, not a rollback without external monotonic state.
- **Swap/hibernation is protected by FileVault, not the app.** `mlock` is best-effort and can fail under memory pressure.
- **SQLCipher/SQLite/WebKit/macOS vulnerabilities are inherited.** Keep the OS, WebView, Rust toolchain, SQLCipher, and dependencies patched; the app cannot fix upstream memory-safety bugs.
- **Crypto-erasure is logical, not physical.** Destroying the wrapped DEK makes ciphertext unrecoverable in practice, but SSD wear-leveling/remanence and copies outside the app are not controllable.
- **Scheduled sync and auto-lock trade off.** A locked vault cannot sync; the owner may choose a longer idle timeout for convenience, accepting more exposure.
- **No hardware-backed key or MFA.** The design deliberately uses a passphrase-only vault; Touch ID/Keychain unlock was excluded by the owner and would change the threat model.

These risks must appear in the README or a linked security document, not just in this review.

---

## 11. Sign-off

Security review result: **changes required; amendments applied to the specification and plan.** The design is acceptable to proceed to Phase 0 only with the critical fail-open check, the native secret-input decision, and the Plaid flow verification as explicit exit criteria. Phase 10 cannot be declared complete until the acceptance checklist above is evidenced and the residual risks are disclosed to the owner.

Reviewed documents after amendment:

- [personal-cards-build-spec.md](personal-cards-build-spec.md)
- [implementation-plan.md](implementation-plan.md)
