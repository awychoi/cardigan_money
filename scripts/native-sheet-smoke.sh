#!/bin/bash
# Debug-only evidence that the native AppKit secure sheet presents and can be
# cancelled. This does NOT enter a passphrase and does NOT replace the owner's
# native smoke test.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/scripts/dev-env.sh"
mkdir -p "$ROOT/.cache/appdata"
LOG="$ROOT/.cache/native-sheet-smoke.log"
rm -f "$LOG"
set +e
CARDIGAN_MONEY_APP_DATA_DIR="$ROOT/.cache/appdata" \
CARDIGAN_MONEY_NATIVE_SHEET_SMOKE=1 \
  pnpm tauri dev > "$LOG" 2>&1
set -e
if grep -q "NATIVE_SHEET_SMOKE=passed" "$LOG"; then
  echo "PASS: native NSSecureTextField sheet presented and cancelled"
else
  echo "FAIL: native sheet smoke did not report success"
  tail -20 "$LOG"
  exit 1
fi
