#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_LOG="$ROOT_DIR/backend/.codex-run/backend.log"
FRONTEND_LOG="$ROOT_DIR/frontend/.codex-run/frontend.log"
MODE="${1:-all}"

show_log() {
  local label="$1"
  local file="$2"

  if [[ ! -f "$file" ]]; then
    echo "$label log not found: $file" >&2
    return 1
  fi

  echo "===== $label ====="
  tail -n 80 "$file"
}

follow_both() {
  local files=()
  [[ -f "$BACKEND_LOG" ]] && files+=("$BACKEND_LOG")
  [[ -f "$FRONTEND_LOG" ]] && files+=("$FRONTEND_LOG")

  if [[ ${#files[@]} -eq 0 ]]; then
    echo "No log files found." >&2
    exit 1
  fi

  tail -f "${files[@]}"
}

case "$MODE" in
  backend)
    show_log backend "$BACKEND_LOG"
    ;;
  frontend)
    show_log frontend "$FRONTEND_LOG"
    ;;
  follow)
    follow_both
    ;;
  all)
    show_log backend "$BACKEND_LOG" || true
    echo
    show_log frontend "$FRONTEND_LOG" || true
    ;;
  *)
    echo "Usage: $0 [all|backend|frontend|follow]" >&2
    exit 1
    ;;
esac
