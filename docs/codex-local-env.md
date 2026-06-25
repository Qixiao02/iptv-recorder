# Codex Windows Local Environment

This note describes the small local setup used by Codex workers on Windows for this repository.

## Prerequisites

- Rust toolchain with `cargo` available in PowerShell.
- Node.js and `pnpm.cmd` available in PowerShell.
- Frontend dependencies installed with:

```powershell
cd D:\work\Porject\iptv-recorder\frontend
pnpm.cmd install
```

Use `pnpm.cmd` instead of bare `pnpm` in PowerShell. On some Windows systems, the `pnpm.ps1` shim is blocked by the execution policy.

## Start

From the repository root:

```powershell
.\scripts\codex-dev.ps1
```

Useful variants:

```powershell
.\scripts\codex-dev.ps1 -BackendOnly
.\scripts\codex-dev.ps1 -FrontendOnly
.\scripts\codex-dev.ps1 -NoBrowser
.\scripts\codex-dev.ps1 -BackendPort 3034
```

The script starts:

- Backend: `http://127.0.0.1:3033/`
- Frontend: `http://127.0.0.1:5173/`

Temporary local login:

- User: `admin`
- Password: `Admin-Temp-2026-06-09!`

## Environment

The script sets these local development variables:

```powershell
IPTV_JWT_SECRET=dev-local-jwt-secret-2026-06-09-at-least-32-chars
IPTV_INITIAL_ADMIN_PASSWORD=Admin-Temp-2026-06-09!
IPTV__SERVER__PORT=3033
VITE_BACKEND_URL=http://127.0.0.1:3033
```

`-BackendPort` changes the backend port and the frontend `VITE_BACKEND_URL` value for that run.

## Logs

Logs are written to:

- `D:\work\Porject\iptv-recorder\logs\backend.log`
- `D:\work\Porject\iptv-recorder\logs\frontend.log`

The script creates the `logs` directory if it does not exist. It warns when a port is already listening, but it does not stop or kill any existing process.

## External Recording Tools

Real recording and transcoding depend on external binaries such as `N_m3u8DL-RE` and `ffmpeg`. The local Codex script does not verify that those tools are installed, so the web UI and API can be started even when full recording functionality is not ready.

## Troubleshooting

- `Missing required command: cargo`: install Rust with rustup, then open a new terminal.
- `Missing required command: pnpm.cmd`: install pnpm and confirm `pnpm.cmd --version` works.
- `pnpm.ps1 cannot be loaded`: use `pnpm.cmd` from PowerShell.
- Port warning for `3033` or `5173`: another process is already listening. Stop it manually or choose another backend port with `-BackendPort`.
