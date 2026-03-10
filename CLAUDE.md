# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

IPTV Recorder is a Rust-based IPTV M3U management and scheduled recording system with a React frontend. It manages TV channel streams, supports cron-based scheduled recording, and provides a web UI for administration.

## Build and Development Commands

### Backend (Rust)

```bash
cd backend

# Development build
cargo build

# Release build
cargo build --release

# Run directly
cargo run

# Run with environment variable
IPTV__SERVER__PORT=8080 cargo run

# Run tests
cargo test

# Run specific test
cargo test test_channel_create

# Lint check
cargo clippy

# Format code
cargo fmt

# Check compilation without building
cargo check
```

### Frontend (React/Vite)

```bash
cd frontend

# Install dependencies
pnpm install

# Development server
pnpm dev

# Production build
pnpm build

# Lint
pnpm lint

# Preview production build
pnpm preview
```

## Architecture

### Backend Structure (`backend/src/`)

The backend follows a layered architecture:

- **`main.rs`** - Entry point, orchestrates initialization: config → database → process manager → scheduler → web server
- **`config.rs`** - Configuration management using figment (priority: env vars > config file > defaults)
- **`api/`** - HTTP/WebSocket layer
  - `router.rs` - Axum route definitions
  - `handlers.rs` - HTTP request handlers
  - `websocket.rs` - WebSocket real-time updates
- **`services/`** - Business logic layer
  - `channel.rs` - Channel CRUD, M3U import, and pagination
  - `schedule.rs` - Recording schedule management
  - `recording.rs` - Recording task execution and cancellation
  - `scheduler.rs` - Cron-based job scheduling
  - `transcode.rs` - UDP to HLS transcoding service
  - `m3u_parser.rs` - M3U playlist parsing
- **`core/`** - Infrastructure layer
  - `database.rs` - SQLite connection pool and migrations
  - `event.rs` - Event bus for async messaging (broadcast channel)
  - `process.rs` - External process management for recording tools
- **`models/`** - Data models (Channel, Schedule, Task)

### Frontend Structure (`frontend/src/`)

React 19 + TypeScript + Vite frontend:

- **`api/`** - API client modules (channels, schedules, tasks, system, websocket)
- **`stores/`** - Zustand state stores (channelStore, taskStore, settingStore, uiStore)
- **`pages/`** - Page components (Dashboard, Channels, Schedules, Tasks, Settings)
- **`locales/`** - i18n translation files (zh-CN, en-US)

### Key Data Flow

1. **Scheduled Recording**: Cron scheduler triggers → Check channel status → Verify concurrent limits → Start recording process → Monitor progress → Update task status → Broadcast via WebSocket
2. **Real-time Updates**: Events published to broadcast channel → WebSocket subscribers receive updates → Frontend state updates
3. **M3U Import**: Parse M3U content → Validate URLs → Batch insert to database → Return import statistics

## External Dependencies

- **N_m3u8DL-RE**: External HLS/DASH stream download tool (required for recording)
  - Download: https://github.com/nilaoda/N_m3u8DL-RE
  - Configured via `recorder.executable` in config

## Configuration

Config file location: `backend/config/default.toml` (copy to `config.toml` for customization)

Environment variables use `IPTV__` prefix with double underscore for nesting:
- `IPTV__SERVER__PORT=8080` → `server.port = 8080`
- `IPTV__DATABASE__PATH=/data/db.sqlite` → `database.path = "/data/db.sqlite"`

## API Endpoints

Base URL: `http://localhost:3000/api`

- `GET/POST/PUT/DELETE /api/channels` - Channel management (GET supports pagination)
- `GET /api/channels/all` - Get all channels without pagination
- `POST /api/channels/import/url` - Import M3U from URL
- `POST /api/channels/import/content` - Import M3U from content
- `GET/POST/PUT/DELETE /api/schedules` - Schedule management
- `POST /api/schedules/{id}/toggle` - Enable/disable schedule
- `GET/DELETE /api/tasks` - Task management
- `POST /api/tasks/{id}/cancel` - Cancel running task
- `POST /api/tasks/manual` - Start manual recording
- `GET /api/scheduler/upcoming` - Get upcoming scheduled tasks
- `POST /api/transcode/start` - Start UDP to HLS transcoding
- `GET /api/transcode/hls/{session_id}/{filename}` - Get HLS files
- `POST /api/transcode/{session_id}` - Stop transcoding session
- `GET /api/proxy/stream?url={url}` - Proxy stream requests (CORS bypass)
- `WS /ws` - WebSocket for real-time updates

## Tech Stack

**Backend**: Axum, Tokio, SQLite (sqlx), tokio-cron-scheduler, tracing, reqwest, anyhow/thiserror

**Frontend**: React 19, TypeScript, Vite, Ant Design, TanStack Query, Zustand, React Router, Axios, i18next

## Code Conventions

- Rust: snake_case for functions/modules, PascalCase for types
- TypeScript: camelCase for variables/functions, PascalCase for components
- Use `anyhow::Result` for application errors with `.context()` for error messages
- Use `tracing` macros (info, warn, error, debug) for logging
