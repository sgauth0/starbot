#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CLI_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

API_URL="${STARBOTT_API_URL:-http://127.0.0.1:3737}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 3
  }
}

need_cmd curl

BIN="${CLI_DIR}/target/debug/starbott"
if [[ ! -x "$BIN" ]]; then
  # Prefer offline build (this environment often blocks crates.io).
  (cd "$CLI_DIR" && cargo build --offline >/dev/null) || (cd "$CLI_DIR" && cargo build >/dev/null)
fi

# Sanity check: make sure API is reachable before launching a TUI.
if ! curl -fsS --max-time 15 "${API_URL}/health" >/dev/null 2>&1; then
  echo "API not reachable at ${API_URL}." >&2
  echo "Start the canonical STARBOT API, then re-run this script." >&2
  exit 2
fi

# Dev-only shortcut: allow a direct token override via STARBOTT_TOKEN.
if [[ -z "${STARBOTT_TOKEN:-}" ]]; then
  if [[ -n "${CI:-}" ]]; then
    echo "Not authenticated (CI mode). Set STARBOTT_TOKEN or run: ${BIN} --api-url ${API_URL} auth login" >&2
    exit 2
  fi

  # If the user already has a saved token in their CLI profile, whoami will succeed.
  if ! "$BIN" --api-url "$API_URL" whoami >/dev/null 2>&1; then
    echo "Not authenticated. Starting device login..." >&2
    "$BIN" --api-url "$API_URL" auth login
  fi
fi

exec "$BIN" --api-url "$API_URL" "$@"
