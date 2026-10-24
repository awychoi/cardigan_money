#!/bin/bash
# Phase 0 evidence: the app's exact cipher verification rejects a binary that
# links plain SQLite. Run from the repository root.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=dev-env.sh
. "$ROOT/scripts/dev-env.sh"
mkdir -p "$ROOT/.cache/spike"
DB="$ROOT/.cache/spike/phase0.cmdb"
rm -f "$DB" "$DB-wal" "$DB-shm"

echo "== create encrypted spike DB (SQLCipher build) =="
(cd "$ROOT/src-tauri" && cargo run --quiet --example make_spike_db -- "$DB")

echo "== plain SQLite client cannot read the DB =="
if /usr/bin/sqlite3 "$DB" "SELECT count(*) FROM sqlite_master;" 2>"$ROOT/.cache/spike/sqlite3.err"; then
  echo "FAIL: plain sqlite3 opened the encrypted database"
  exit 1
fi
echo "PASS: sqlite3 refused: $(tr -d '\n' < "$ROOT/.cache/spike/sqlite3.err")"

echo "== plain-SQLite build of the app check rejects the DB =="
set +e
(cd "$ROOT/tools/plain-sqlite-probe" && CARGO_TARGET_DIR="$ROOT/.cache/plain-probe-target" cargo run --quiet -- "$DB")
code=$?
set -e
if [ "$code" -ne 2 ]; then
  echo "FAIL: expected the plain build check to exit 2, got $code"
  exit 1
fi
echo "PASS: plain-SQLite build rejected (exit $code)"
