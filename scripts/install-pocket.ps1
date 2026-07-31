#Requires -Version 5.1
<#
.SYNOPSIS
    Installs PocketSphinx-based Pocket TTS (pocket-tts CLI) for CopySpeak via uv.

.DESCRIPTION
    Installs pocket-tts as a uv tool so the `pocket-tts` binary is on PATH.

.PARAMETER Force
    Reinstall even if pocket-tts is already available.

.PARAMETER SmokeTest
    Synthesize one clip after install to verify.

.EXAMPLE
    ./scripts/install-pocket.ps1
#>

param(
    [switch]$Force,
    [switch]$SmokeTest,
    # App-driven invocation passes -Voices; pocket-tts has one built-in
    # voice (default), so the arg is silently accepted and ignored.
    [string[]]$Voices
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Pocket TTS Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
$alreadyInstalled = [bool](Get-Command pocket-tts -ErrorAction SilentlyContinue)
$effectiveForce = if ($Force) {
    $true
} elseif (-not $alreadyInstalled) {
    $false
} else {
    Get-Confirmation -Prompt "pocket-tts is already installed. Reinstall from scratch?" -DefaultYes:$false
}

Write-Host "  [STEP] engine" -ForegroundColor Yellow
if (-not $effectiveForce -and $alreadyInstalled) {
    Write-Host "  pocket-tts already installed." -ForegroundColor Green
    Write-Host "  Use -Force or answer Yes to reinstall." -ForegroundColor Yellow
} else {
    Write-Host "  Installing pocket-tts via uv tool..." -ForegroundColor Gray
    Invoke-Uv tool install pocket-tts --force
}
Write-Host "  [DONE] engine" -ForegroundColor Green

$profileJson = @"
{
  "schema_version": 1,
  "id": "pocket-local",
  "name": "Pocket (Local)",
  "engine": "local",
  "voice": "default",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "pocket-tts",
    "command": "pocket-tts",
    "args_template": ["generate", "--voice", "{voice}", "--text", "{raw_text}", "--output-path", "{output}"]
  }
}
"@

if ($SmokeTest -and (Get-Command pocket-tts -ErrorAction SilentlyContinue)) {
    $out = Join-Path $env:TEMP "copyspeak-pocket-test.wav"
    Write-Host "  Smoke test..." -ForegroundColor Yellow
    pocket-tts generate --voice default --text "Hello from Pocket TTS" --output-path $out
    if (-not (Test-AudioFile -Path $out)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

Write-Host ""
Write-Host "  Pocket TTS installed." -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
