# Measured performance — 2026-10-23

Environment: Apple M1 Max, 64 GiB, macOS 27.0.1, Rust 1.99.1, debug test profile (unoptimized; Argon2 and SQLCipher are therefore slower than the release build). Dataset: 50,000 transactions across 24 months, one encrypted account.

Method: `cargo test --test perf -- --ignored --nocapture` (see `src-tauri/tests/perf.rs`). Six warm iterations after a discarded first run; median reported. The encrypted vault is created normally; 50k rows are inserted in one transaction through the app's SQLCipher connection.

| Measurement | Debug | Release | Target | Status (release) |
| --- | --- | --- | --- | --- |
| Insert 50k rows (one transaction, encrypted) | 9.78 s | — | — | informational |
| Warm filtered ledger search (50-row page) | 16.9 ms | **13.1 ms** | < 200 ms | Met |
| Month change / month summary | 38.9 ms | **32.3 ms** | < 200 ms | Met |
| Dashboard (summary + cards + ledger aggregates) | 48.6 ms | **40.5 ms** | — | informational |
| Cached dashboard after unlock | 51.6 ms | **42.6 ms** | roughly 2 s | Met |
| Unlock (stored KDF params) | 3.45 s | **2.12 s** | roughly 2 s | Approximately met |

The release measurements (`cargo test --release --test perf -- --ignored --nocapture`) meet the search/month targets with wide margin. Unlock is dominated by Argon2id (64 MiB, 3 iterations) plus SQLCipher open/integrity checks; 2.08 s is approximately the rough two-second cold-start target and is deliberately expensive. Cached dashboard after unlock is 42 ms. These are wall-clock medians on one machine, not universal guarantees.

Queries used the existing indexes for reporting month/date and provider identity; filtering by merchant uses indexed normalized text where available. No query-plan change was made for this measurement. No profiling claim beyond these wall-clock medians is made.
