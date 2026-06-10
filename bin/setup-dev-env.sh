#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Setting up ties development environment ==="

# ── Load environment variables ────────────────────────────────────────────────
# Source .env early so DATABASE_URL, DATABASE_PORT, etc. are available throughout.
if [ -f "$PROJECT_DIR/.env" ]; then
    set -a
    source "$PROJECT_DIR/.env"
    set +a
else
    echo "ERROR: .env file not found. Copy .env.example to .env and adjust it first." >&2
    exit 1
fi

# ── System packages ──────────────────────────────────────────────────────────
echo "--- Installing system packages ---"
pacman -Syu --noconfirm rust podman mkcert nss jq

# ── Cargo tooling ─────────────────────────────────────────────────────────────
echo "--- Installing cargo-run-bin ---"
cargo install cargo-run-bin

# ── PATH setup ────────────────────────────────────────────────────────────────
# cargo-bin stores tools under .bin/ inside the project directory.
# The exact path includes the Rust version, so we add a wildcard match.
BIN_DIR="$PROJECT_DIR/.bin"
export PATH="$BIN_DIR:$PATH"

# Ensure the just and cargo-nextest binaries managed by cargo-bin are on PATH
# by discovering them dynamically.
for tool_dir in "$BIN_DIR"/rust-*/just/*/bin; do
    export PATH="$tool_dir:$PATH"
done
for tool_dir in "$BIN_DIR"/rust-*/cargo-nextest/*/bin; do
    export PATH="$tool_dir:$PATH"
done

# ── Local TLS CA ──────────────────────────────────────────────────────────────
echo "--- Installing local certificate authority ---"
mkcert -install

# ── Development TLS certificates ──────────────────────────────────────────────
echo "--- Generating development TLS certificates ---"
just development-cert

# ── Start database containers ────────────────────────────────────────────────
echo "--- Starting development database ---"
just start-database

echo "--- Starting test database ---"
just start-test-database

# ── Migrate databases ──────────────────────────────────────────────────────────
# We must compile with SQLX_OFFLINE=true the first time because the database
# schemas don't exist yet for compile-time query verification.
echo "--- Migrating development database ---"
SQLX_OFFLINE=true cargo run -- db --database-url "$DATABASE_URL" --base-url "$BASE_URL" migrate

echo "--- Migrating test database ---"
SQLX_OFFLINE=true cargo run -- db --database-url "$DATABASE_URL_TEST" --base-url "$BASE_URL" migrate

# ── Run tests ──────────────────────────────────────────────────────────────────
echo "--- Running tests ---"
just test

echo "=== Development environment setup complete ==="