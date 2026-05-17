#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"
FRONTEND_DIR="$ROOT_DIR/frontend"

BACKEND_LOG_DIR="$BACKEND_DIR/.codex-run"
FRONTEND_LOG_DIR="$FRONTEND_DIR/.codex-run"
BACKEND_LOG="$BACKEND_LOG_DIR/backend.log"
FRONTEND_LOG="$FRONTEND_LOG_DIR/frontend.log"
BACKEND_PID_FILE="$BACKEND_LOG_DIR/backend.pid"
FRONTEND_PID_FILE="$FRONTEND_LOG_DIR/frontend.pid"

BACKEND_HOST="${BACKEND_HOST:-0.0.0.0}"
BACKEND_PORT="${BACKEND_PORT:-3033}"
FRONTEND_HOST="${FRONTEND_HOST:-0.0.0.0}"
FRONTEND_PORT="${FRONTEND_PORT:-5778}"
JWT_SECRET="${IPTV_JWT_SECRET:-abcdefghijklmnopqrstuvwxyz123456}"
LOG_LINES="${LOG_LINES:-80}"

COMMAND="${1:-status}"
TARGET="${2:-all}"
OPTION="${3:-}"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/dev.sh start [all|backend|frontend]
  ./scripts/dev.sh stop [all|backend|frontend]
  ./scripts/dev.sh restart [all|backend|frontend]
  ./scripts/dev.sh status
  ./scripts/dev.sh logs [all|backend|frontend] [--follow]

Examples:
  ./scripts/dev.sh start
  ./scripts/dev.sh restart backend
  ./scripts/dev.sh stop frontend
  ./scripts/dev.sh logs backend
  ./scripts/dev.sh logs all --follow
EOF
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

ensure_log_dirs() {
  mkdir -p "$BACKEND_LOG_DIR" "$FRONTEND_LOG_DIR"
}

detect_access_host() {
  hostname -I 2>/dev/null | awk '{print $1}' || true
}

read_pid_file() {
  local pid_file="$1"
  if [[ -f "$pid_file" ]]; then
    tr -d '[:space:]' < "$pid_file"
  fi
}

is_pid_running() {
  local pid="$1"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

port_pid() {
  local port="$1"
  lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | head -n 1 || true
}

cleanup_pid_file_if_stale() {
  local pid_file="$1"
  local pid
  pid="$(read_pid_file "$pid_file")"

  if [[ -n "$pid" ]] && ! is_pid_running "$pid"; then
    rm -f "$pid_file"
  fi
}

component_log_file() {
  case "$1" in
    backend) echo "$BACKEND_LOG" ;;
    frontend) echo "$FRONTEND_LOG" ;;
    *)
      echo "Unknown component: $1" >&2
      exit 1
      ;;
  esac
}

component_pid_file() {
  case "$1" in
    backend) echo "$BACKEND_PID_FILE" ;;
    frontend) echo "$FRONTEND_PID_FILE" ;;
    *)
      echo "Unknown component: $1" >&2
      exit 1
      ;;
  esac
}

component_port() {
  case "$1" in
    backend) echo "$BACKEND_PORT" ;;
    frontend) echo "$FRONTEND_PORT" ;;
    *)
      echo "Unknown component: $1" >&2
      exit 1
      ;;
  esac
}

component_name() {
  case "$1" in
    backend) echo "Backend" ;;
    frontend) echo "Frontend" ;;
    *)
      echo "Unknown component: $1" >&2
      exit 1
      ;;
  esac
}

start_backend() {
  local pid
  pid="$(port_pid "$BACKEND_PORT")"
  if [[ -n "$pid" ]]; then
    echo "Backend already running on :$BACKEND_PORT (pid $pid)"
    return
  fi

  ensure_log_dirs
  echo "Starting backend on $BACKEND_HOST:$BACKEND_PORT ..."
  (
    cd "$BACKEND_DIR"
    nohup env \
      IPTV_JWT_SECRET="$JWT_SECRET" \
      IPTV__SERVER__HOST="$BACKEND_HOST" \
      IPTV__SERVER__PORT="$BACKEND_PORT" \
      cargo run >"$BACKEND_LOG" 2>&1 &
    echo $! >"$BACKEND_PID_FILE"
  )
}

start_frontend() {
  local pid
  pid="$(port_pid "$FRONTEND_PORT")"
  if [[ -n "$pid" ]]; then
    echo "Frontend already running on :$FRONTEND_PORT (pid $pid)"
    return
  fi

  ensure_log_dirs
  echo "Starting frontend on $FRONTEND_HOST:$FRONTEND_PORT ..."
  (
    cd "$FRONTEND_DIR"
    nohup env \
      VITE_BACKEND_URL="http://127.0.0.1:$BACKEND_PORT" \
      pnpm exec vite --host "$FRONTEND_HOST" --port "$FRONTEND_PORT" >"$FRONTEND_LOG" 2>&1 &
    echo $! >"$FRONTEND_PID_FILE"
  )
}

stop_component() {
  local component="$1"
  local pid_file
  local port
  local pid
  local fallback_pid
  local name

  pid_file="$(component_pid_file "$component")"
  port="$(component_port "$component")"
  name="$(component_name "$component")"

  cleanup_pid_file_if_stale "$pid_file"
  pid="$(read_pid_file "$pid_file")"

  if is_pid_running "$pid"; then
    echo "Stopping $name (pid $pid)"
    kill "$pid"
    rm -f "$pid_file"
    return
  fi

  fallback_pid="$(port_pid "$port")"
  if [[ -n "$fallback_pid" ]]; then
    echo "Stopping $name on :$port (pid $fallback_pid)"
    kill "$fallback_pid"
    rm -f "$pid_file"
    return
  fi

  echo "$name is not running"
}

show_component_status() {
  local component="$1"
  local pid_file
  local log_file
  local port
  local pid
  local fallback_pid
  local name

  pid_file="$(component_pid_file "$component")"
  log_file="$(component_log_file "$component")"
  port="$(component_port "$component")"
  name="$(component_name "$component")"

  cleanup_pid_file_if_stale "$pid_file"
  pid="$(read_pid_file "$pid_file")"

  if is_pid_running "$pid"; then
    echo "$name: running (pid $pid, port $port)"
  else
    fallback_pid="$(port_pid "$port")"
    if [[ -n "$fallback_pid" ]]; then
      echo "$name: running (pid $fallback_pid, port $port, pid file missing)"
    else
      echo "$name: stopped (port $port)"
    fi
  fi

  if [[ -f "$log_file" ]]; then
    echo "  log: $log_file"
  else
    echo "  log: missing"
  fi
}

show_access_info() {
  local access_host
  access_host="${ACCESS_HOST:-$(detect_access_host)}"
  if [[ -z "$access_host" ]]; then
    access_host="127.0.0.1"
  fi

  echo
  echo "Frontend bind: http://$FRONTEND_HOST:$FRONTEND_PORT"
  echo "Backend bind:  http://$BACKEND_HOST:$BACKEND_PORT"
  echo "Frontend LAN:  http://$access_host:$FRONTEND_PORT"
  echo "Backend LAN:   http://$access_host:$BACKEND_PORT"
}

show_log() {
  local component="$1"
  local file

  file="$(component_log_file "$component")"

  if [[ ! -f "$file" ]]; then
    echo "$(component_name "$component") log not found: $file" >&2
    return 1
  fi

  echo "===== $(component_name "$component") ====="
  tail -n "$LOG_LINES" "$file"
}

follow_logs() {
  local target="$1"
  local files=()

  case "$target" in
    backend)
      [[ -f "$BACKEND_LOG" ]] && files+=("$BACKEND_LOG")
      ;;
    frontend)
      [[ -f "$FRONTEND_LOG" ]] && files+=("$FRONTEND_LOG")
      ;;
    all)
      [[ -f "$BACKEND_LOG" ]] && files+=("$BACKEND_LOG")
      [[ -f "$FRONTEND_LOG" ]] && files+=("$FRONTEND_LOG")
      ;;
    *)
      echo "Unknown log target: $target" >&2
      usage
      exit 1
      ;;
  esac

  if [[ ${#files[@]} -eq 0 ]]; then
    echo "No log files found." >&2
    exit 1
  fi

  tail -f "${files[@]}"
}

start_target() {
  case "$1" in
    backend) start_backend ;;
    frontend) start_frontend ;;
    all)
      start_backend
      start_frontend
      show_access_info
      ;;
    *)
      echo "Unknown target: $1" >&2
      usage
      exit 1
      ;;
  esac
}

stop_target() {
  case "$1" in
    backend) stop_component backend ;;
    frontend) stop_component frontend ;;
    all)
      stop_component frontend
      stop_component backend
      ;;
    *)
      echo "Unknown target: $1" >&2
      usage
      exit 1
      ;;
  esac
}

restart_target() {
  stop_target "$1"
  sleep 1
  start_target "$1"
}

status_target() {
  show_component_status backend
  show_component_status frontend
}

logs_target() {
  local target="$1"
  local follow="${2:-}"

  if [[ "$follow" == "--follow" ]]; then
    follow_logs "$target"
    return
  fi

  case "$target" in
    backend)
      show_log backend
      ;;
    frontend)
      show_log frontend
      ;;
    all)
      show_log backend || true
      echo
      show_log frontend || true
      ;;
    *)
      echo "Unknown log target: $target" >&2
      usage
      exit 1
      ;;
  esac
}

require_cmd lsof
require_cmd cargo
require_cmd pnpm

case "$COMMAND" in
  start)
    start_target "$TARGET"
    ;;
  stop)
    stop_target "$TARGET"
    ;;
  restart)
    restart_target "$TARGET"
    ;;
  status)
    status_target
    ;;
  logs)
    logs_target "$TARGET" "$OPTION"
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    echo "Unknown command: $COMMAND" >&2
    usage
    exit 1
    ;;
esac
