#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Kokoro TTS (kokoro-tts CLI) for CopySpeak via uv.

.DESCRIPTION
    Installs kokoro-tts as a uv tool so the `kokoro-tts` binary is on PATH.
    Models are handled by the CLI itself.

.PARAMETER Force
    Reinstall even if kokoro-tts is already available.

.PARAMETER SmokeTest
    Synthesize one clip after install to verify.

.EXAMPLE
    ./scripts/install-kokoro.ps1
    ./scripts/install-kokoro.ps1 -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Kokoro TTS Installer"

Require-Uv

if (-not $Force -and (Get-Command kokoro-tts -ErrorAction SilentlyContinue)) {
    Write-Host "  kokoro-tts already installed." -ForegroundColor Green
    Write-Host "  Use -Force to reinstall." -ForegroundColor Yellow
} else {
    Write-Host "  Installing kokoro-tts via uv tool..." -ForegroundColor Gray
    Invoke-Uv tool install kokoro-tts --force
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "kokoro-local",
  "name": "Kokoro (Local)",
  "engine": "local",
  "voice": "af_nicole",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "kokoro-tts",
    "command": "kokoro-tts",
    "args_template": ["{input}", "{output}", "--voice", "{voice}"]
  }
}
"@

if ($SmokeTest -and (Get-Command kokoro-tts -ErrorAction SilentlyContinue)) {
    $out = Join-Path $env:TEMP "copyspeak-kokoro-test.wav"
    $txt = Join-Path $env:TEMP "copyspeak-kokoro-test.txt"
    "Hello from Kokoro" | Set-Content -Path $txt -Encoding utf8
    Write-Host "  Smoke test..." -ForegroundColor Yellow
    kokoro-tts $txt $out --voice af_nicole
    if (-not (Test-AudioFile -Path $out)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

Write-Host ""
Write-Host "  Kokoro installed." -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
