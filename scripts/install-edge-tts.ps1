#Requires -Version 5.1
<#
.SYNOPSIS
    Installs edge-tts (free Microsoft Read Aloud CLI) for CopySpeak via uv.

.DESCRIPTION
    edge-tts is a tiny Python CLI that calls Microsoft's free Read Aloud
    endpoint. No API key, no model download. Installed as a uv tool so the
    `edge-tts` binary lands on PATH.

    Does NOT edit CopySpeak config. Prints setup notes on success.

.PARAMETER Force
    Reinstall even if edge-tts is already available.

.PARAMETER SmokeTest
    Synthesize one short clip after install to verify.

.EXAMPLE
    ./scripts/install-edge-tts.ps1
    ./scripts/install-edge-tts.ps1 -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Edge-TTS Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
$alreadyInstalled = [bool](Get-Command edge-tts -ErrorAction SilentlyContinue)
$effectiveForce = if ($Force) {
    $true
} elseif (-not $alreadyInstalled) {
    $false
} else {
    Get-Confirmation -Prompt "edge-tts is already installed. Reinstall from scratch?" -DefaultYes:$false
}

if (-not $effectiveForce -and $alreadyInstalled) {
    Write-Host "  edge-tts already installed: $(edge-tts --version)" -ForegroundColor Green
    Write-Host "  Use -Force or answer Yes to reinstall." -ForegroundColor Yellow
} else {
    Write-Host "  Installing edge-tts via uv tool..." -ForegroundColor Gray
    Invoke-Uv tool install edge-tts --force
}

if ($SmokeTest) {
    $out = Join-Path $env:TEMP "copyspeak-edge-test.mp3"
    Write-Host "  Smoke test..." -ForegroundColor Yellow
    edge-tts --voice "en-US-AvaMultilingualNeural" --text "Hello from Edge TTS" --write-media $out
    if (-not (Test-AudioFile -Path $out)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

Write-Host ""
Write-Host "  edge-tts is ready. In CopySpeak, use engine = Edge-TTS and pick a voice." -ForegroundColor Green
Write-Host "  No profile import needed; Edge-TTS is a built-in engine." -ForegroundColor Gray
Write-Host ""
