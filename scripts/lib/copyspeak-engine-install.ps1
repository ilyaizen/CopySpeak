#Requires -Version 5.1
<#
.SYNOPSIS
    Shared helpers for CopySpeak uv-based engine installers.

.DESCRIPTION
    Dot-source this file from an engine installer:
        . "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

    All Python engines are managed by `uv`. These helpers stay deliberately
    boring: require uv, locate the engine root, create a uv project, run uv,
    validate audio output, and print a CopySpeak profile snippet.
#>

# Require uv on PATH. Fails hard (engine installers cannot proceed without it).
function Require-Uv {
    $uv = Get-Command uv -ErrorAction SilentlyContinue
    if (-not $uv) {
        Write-Host "ERROR: uv is not installed or not on PATH." -ForegroundColor Red
        Write-Host "  Install it first:  ./scripts/install-uv.ps1" -ForegroundColor Yellow
        exit 1
    }
    $version = & uv --version
    Write-Host "  Found uv: $version" -ForegroundColor Green
}

# %LOCALAPPDATA%\CopySpeak\engines  (matches the {engine_dir} placeholder).
function Get-CopySpeakEngineRoot {
    return Join-Path $env:LOCALAPPDATA "CopySpeak\engines"
}

# Create (or reset with -Force) a uv project directory for one engine.
function New-EngineProject {
    param(
        [Parameter(Mandatory)][string]$EngineDir,
        [switch]$Force
    )
    if ((Test-Path $EngineDir) -and $Force) {
        Write-Host "  Removing existing engine dir (-Force): $EngineDir" -ForegroundColor Yellow
        Remove-Item -Recurse -Force $EngineDir
    }
    if (-not (Test-Path $EngineDir)) {
        New-Item -ItemType Directory -Force $EngineDir | Out-Null
        Write-Host "  Created: $EngineDir" -ForegroundColor Gray
    }
    if (-not (Test-Path (Join-Path $EngineDir "pyproject.toml"))) {
        Write-Host "  Running: uv init --bare ($EngineDir)" -ForegroundColor Gray
        & uv --project $EngineDir init --bare
    }
}

# Run uv, echoing the exact command. Stops on non-zero exit.
function Invoke-Uv {
    param([Parameter(ValueFromRemainingArguments)][string[]]$Args)
    Write-Host "  > uv $($Args -join ' ')" -ForegroundColor DarkGray
    & uv @Args
    if ($LASTEXITCODE -ne 0) {
        throw "uv exited with code $LASTEXITCODE"
    }
}

# Validate that a synthesis smoke test produced a non-empty WAV.
function Test-AudioFile {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path $Path)) {
        Write-Host "  ERROR: expected audio file not found: $Path" -ForegroundColor Red
        return $false
    }
    $size = (Get-Item $Path).Length
    if ($size -le 44) {
        Write-Host "  ERROR: audio file is empty/too small ($size bytes): $Path" -ForegroundColor Red
        return $false
    }
    Write-Host "  OK: audio file $Path ($size bytes)" -ForegroundColor Green
    return $true
}

# Print a ready-to-paste CopySpeak profile snippet. Does NOT edit config (v1).
function Write-ProfileSnippet {
    param([Parameter(Mandatory)][string]$Json)
    Write-Host ""
    Write-Host "  Paste this into a CopySpeak profile (Engine settings -> Import):" -ForegroundColor Cyan
    Write-Host "  ----------------------------------------------------------------" -ForegroundColor DarkGray
    $Json -split "`n" | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
    Write-Host "  ----------------------------------------------------------------" -ForegroundColor DarkGray
    Write-Host ""
}
