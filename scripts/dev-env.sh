# Source from anywhere: . /path/to/repo/scripts/dev-env.sh
# The active DSH sandbox allows writes only under the workspace, so the Rust
# toolchain and package caches live inside the checkout (gitignored).
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export REPO_ROOT="$ROOT"
export RUSTUP_HOME="$ROOT/.toolchain/rustup"
export CARGO_HOME="$ROOT/.toolchain/cargo"
export PATH="$CARGO_HOME/bin:$PATH"
export PNPM_STORE_DIR="$ROOT/.cache/pnpm-store"
export PLAYWRIGHT_BROWSERS_PATH="$ROOT/.cache/ms-playwright"
