#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"
FRONTEND_DIR="$ROOT_DIR/frontend"
BACKEND_LOG_DIR="$BACKEND_DIR/.codex-run"
FRONTEND_LOG_DIR="$FRONTEND_DIR/.codex-run"
BACKEND_LOG="$BACKEND_LOG_DIR/backend.log"
FRONTEND_LOG="$FRONTEND_LOG_DIR/frontend.log"
BACKEND_HOST="${BACKEND_HOST:-0.0.0.0}"
BACKEND_PORT="${BACKEND_PORT:-3033}"
FRONTEND_HOST="${FRONTEND_HOST:-0.0.0.0}"
FRONTEND_PORT="${FRONTEND_PORT:-5778}"
JWT_SECRET="${IPTV_JWT_SECRET:-abcdefghijklmnopqrstuvwxyz123456}"

detect_access_host() {
  hostname -I 2>/dev/null | awk '{print $1}' || true
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

port_pid() {
  lsof -tiTCP:"$1" -sTCP:LISTEN 2>/dev/null | head -n 1 || true
}

start_backend() {
  local pid
  pid="$(port_pid "$BACKEND_PORT")"
  if [[ -n "$pid" ]]; then
    echo "Backend already running on :$BACKEND_PORT (pid $pid)"
    return
  fi

  mkdir -p "$BACKEND_LOG_DIR"
  echo "Starting backend on $BACKEND_HOST:$BACKEND_PORT ..."
  (
    cd "$BACKEND_DIR"
    nohup env \
      IPTV_JWT_SECRET="$JWT_SECRET" \
      IPTV__SERVER__HOST="$BACKEND_HOST" \
      IPTV__SERVER__PORT="$BACKEND_PORT" \
      cargo run > "$BACKEND_LOG" 2>&1 &
    echo $! > "$BACKEND_LOG_DIR/backend.pid"
  )
}

start_frontend() {
  local pid
  pid="$(port_pid "$FRONTEND_PORT")"
  if [[ -n "$pid" ]]; then
    echo "Frontend already running on :$FRONTEND_PORT (pid $pid)"
    return
  fi

  mkdir -p "$FRONTEND_LOG_DIR"
  echo "Starting frontend on $FRONTEND_HOST:$FRONTEND_PORT ..."
  (
    cd "$FRONTEND_DIR"
    nohup env \
      VITE_BACKEND_URL="http://127.0.0.1:$BACKEND_PORT" \
      VITE_API_URL="http://$ACCESS_HOST_FALLBACK:$BACKEND_PORT" \
      pnpm exec vite --host "$FRONTEND_HOST" --port "$FRONTEND_PORT" > "$FRONTEND_LOG" 2>&1 &
    echo $! > "$FRONTEND_LOG_DIR/frontend.pid"
  )
}

require_cmd lsof
require_cmd cargo
require_cmd pnpm

ACCESS_HOST_FALLBACK="${ACCESS_HOST:-$(detect_access_host)}"
if [[ -z "$ACCESS_HOST_FALLBACK" ]]; then
  ACCESS_HOST_FALLBACK="127.0.0.1"
fi

start_backend
start_frontend

sleep 2

ACCESS_HOST="$ACCESS_HOST_FALLBACK"

echo
echo "Frontend bind: http://$FRONTEND_HOST:$FRONTEND_PORT"
echo "Backend bind:  http://$BACKEND_HOST:$BACKEND_PORT"
echo "Frontend LAN:  http://$ACCESS_HOST:$FRONTEND_PORT"
echo "Backend LAN:   http://$ACCESS_HOST:$BACKEND_PORT"
echo "Logs:"
echo "  $BACKEND_LOG"
echo "  $FRONTEND_LOG"
