<#
.SYNOPSIS
Starts the IPTV Recorder local development stack on Windows for Codex.

.DESCRIPTION
Starts the Rust backend and React/Vite frontend with known local development
environment variables. Logs are written to the repository logs directory.

.PARAMETER BackendOnly
Start only the backend service.

.PARAMETER FrontendOnly
Start only the frontend service.

.PARAMETER NoBrowser
Do not open the frontend URL after starting services.

.PARAMETER BackendPort
Backend port to use. Defaults to 3033.

.EXAMPLE
.\scripts\codex-dev.ps1

.EXAMPLE
.\scripts\codex-dev.ps1 -BackendOnly -BackendPort 3034
#>
[CmdletBinding()]
param(
    [switch]$BackendOnly,
    [switch]$FrontendOnly,
    [switch]$NoBrowser,
    [ValidateRange(1, 65535)]
    [int]$BackendPort = 3033
)

$ErrorActionPreference = 'Stop'

if ($BackendOnly -and $FrontendOnly) {
    throw 'Use either -BackendOnly or -FrontendOnly, not both.'
}

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$BackendDir = Join-Path $RootDir 'backend'
$FrontendDir = Join-Path $RootDir 'frontend'
$LogDir = Join-Path $RootDir 'logs'
$BackendLog = Join-Path $LogDir 'backend.log'
$FrontendLog = Join-Path $LogDir 'frontend.log'

$FrontendUrl = 'http://127.0.0.1:5173/'
$BackendUrl = "http://127.0.0.1:$BackendPort/"
$JwtSecret = 'dev-local-jwt-secret-2026-06-09-at-least-32-chars'
$InitialAdminPassword = 'Admin-Temp-2026-06-09!'

function Test-CommandAvailable {
    param([Parameter(Mandatory = $true)][string]$Name)

    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-PortListeners {
    param([Parameter(Mandatory = $true)][int]$Port)

    try {
        return @(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction Stop)
    }
    catch {
        return @()
    }
}

function Write-PortWarning {
    param(
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $listeners = Get-PortListeners -Port $Port
    if ($listeners.Count -eq 0) {
        return
    }

    $pids = $listeners | Select-Object -ExpandProperty OwningProcess -Unique
    Write-Warning "$Name port $Port is already listening. Existing PID(s): $($pids -join ', '). This script will not kill them."
}

function New-RunnerArguments {
    param(
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$LogPath,
        [Parameter(Mandatory = $true)][string[]]$CommandLine,
        [Parameter(Mandatory = $true)][hashtable]$Environment
    )

    $envLines = foreach ($key in $Environment.Keys) {
        "`$env:$key = '$($Environment[$key])'"
    }

    $commandText = @(
        '$ErrorActionPreference = ''Continue'''
        "Set-Location -LiteralPath '$WorkingDirectory'"
        $envLines
        "`$log = '$LogPath'"
        "'===== Started $(Get-Date -Format o) =====' | Out-File -LiteralPath `$log -Encoding utf8"
        "Write-Output 'Working directory: $WorkingDirectory' *>> `$log"
        "Write-Output 'Command: $($CommandLine -join ' ')' >> `$log"
        "& '$($CommandLine[0])' $($CommandLine[1..($CommandLine.Count - 1)] -join ' ') *>> `$log"
    ) -join '; '

    return @(
        '-NoProfile',
        '-ExecutionPolicy', 'Bypass',
        '-Command', $commandText
    )
}

function Start-CodexService {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$LogPath,
        [Parameter(Mandatory = $true)][string[]]$CommandLine,
        [Parameter(Mandatory = $true)][hashtable]$Environment
    )

    $args = New-RunnerArguments -WorkingDirectory $WorkingDirectory -LogPath $LogPath -CommandLine $CommandLine -Environment $Environment
    $process = Start-Process -FilePath 'powershell.exe' -ArgumentList $args -WorkingDirectory $WorkingDirectory -WindowStyle Hidden -PassThru

    [pscustomobject]@{
        Name = $Name
        Id = $process.Id
        LogPath = $LogPath
    }
}

$startBackend = -not $FrontendOnly
$startFrontend = -not $BackendOnly

if ($startBackend -and -not (Test-CommandAvailable 'cargo')) {
    throw 'Missing required command: cargo. Install Rust from https://rustup.rs/ and restart the terminal.'
}

if ($startFrontend -and -not (Test-CommandAvailable 'pnpm.cmd')) {
    throw 'Missing required command: pnpm.cmd. Install pnpm, then use pnpm.cmd on Windows PowerShell to avoid execution policy issues.'
}

if (-not (Test-Path -LiteralPath $BackendDir -PathType Container)) {
    throw "Backend directory not found: $BackendDir"
}

if (-not (Test-Path -LiteralPath $FrontendDir -PathType Container)) {
    throw "Frontend directory not found: $FrontendDir"
}

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

if ($startBackend) {
    Write-PortWarning -Port $BackendPort -Name 'Backend'
}

if ($startFrontend) {
    Write-PortWarning -Port 5173 -Name 'Frontend'
}

$started = @()

if ($startBackend) {
    $started += Start-CodexService `
        -Name 'backend' `
        -WorkingDirectory $BackendDir `
        -LogPath $BackendLog `
        -CommandLine @('cargo', 'run') `
        -Environment @{
            IPTV_JWT_SECRET = $JwtSecret
            IPTV_INITIAL_ADMIN_PASSWORD = $InitialAdminPassword
            IPTV__SERVER__PORT = "$BackendPort"
        }
}

if ($startFrontend) {
    $started += Start-CodexService `
        -Name 'frontend' `
        -WorkingDirectory $FrontendDir `
        -LogPath $FrontendLog `
        -CommandLine @('pnpm.cmd', 'dev', '--host', '127.0.0.1') `
        -Environment @{
            VITE_BACKEND_URL = "http://127.0.0.1:$BackendPort"
        }
}

Write-Host ''
Write-Host 'IPTV Recorder local development services started.'
Write-Host "Backend URL : $BackendUrl"
Write-Host "Frontend URL: $FrontendUrl"
Write-Host "Login       : admin / $InitialAdminPassword"
Write-Host "Backend log : $BackendLog"
Write-Host "Frontend log: $FrontendLog"

foreach ($service in $started) {
    Write-Host ("PID         : {0} ({1})" -f $service.Id, $service.Name)
}

Write-Host ''
Write-Host 'Recording/transcoding still requires external N_m3u8DL-RE and ffmpeg binaries on PATH or configured explicitly.'

if ($startFrontend -and -not $NoBrowser) {
    Start-Process $FrontendUrl | Out-Null
}
