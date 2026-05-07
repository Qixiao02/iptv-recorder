#!/usr/bin/env bash
set -euo pipefail

BACKEND_PORT="${BACKEND_PORT:-3033}"
FRONTEND_PORT="${FRONTEND_PORT:-5778}"

stop_port() {
  local name="$1"
  local port="$2"
  local pids
  pids="$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)"

  if [[ -z "$pids" ]]; then
    echo "$name not running on :$port"
    return
  fi

  echo "Stopping $name on :$port ($pids)"
  kill $pids
}

stop_port backend "$BACKEND_PORT"
stop_port frontend "$FRONTEND_PORT"
