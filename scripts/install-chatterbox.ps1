#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Resemble AI Chatterbox as a CopySpeak local TTS engine via uv.

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeak\engines\chatterbox,
    installs Chatterbox, and drops the stable CLI wrapper. Does NOT touch system
    Python and does NOT auto-edit CopySpeak config; it prints a profile snippet.

    Run the smoke test only after you approve it (-SmokeTest), per repo rules.

.PARAMETER Force
    Recreate the engine project from scratch.

.PARAMETER SmokeTest
    Run one synthesis after install to verify a valid WAV is produced.

.EXAMPLE
    ./scripts/install-chatterbox.ps1

.EXAMPLE
    ./scripts/install-chatterbox.ps1 -Force -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-Host ""
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host "  |  Chatterbox Installer for CopySpeak |" -ForegroundColor Magenta
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host ""

Require-Uv

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "chatterbox"
New-EngineProject -EngineDir $EngineDir -Force:$Force

# Install Chatterbox. Upstream is published on PyPI as `chatterbox-tts`.
Write-Host ""
Write-Host "  Installing Chatterbox (this can be large; Torch is pulled in)..." -ForegroundColor Gray
Invoke-Uv add --project $EngineDir "chatterbox-tts" "torchaudio"

# Copy the stable wrapper + ensure voices/ and output/ dirs exist.
$scriptsDir = Join-Path $EngineDir "scripts"
$voicesDir = Join-Path $EngineDir "voices"
$outputDir = Join-Path $EngineDir "output"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $voicesDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null

$srcWrapper = Join-Path $PSScriptRoot "chatterbox/copyspeak-chatterbox.py"
$dstWrapper = Join-Path $scriptsDir "copyspeak-chatterbox.py"
Copy-Item $srcWrapper $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray

if ($SmokeTest) {
    Write-Host ""
    Write-Host "  Running synthesis smoke test (first run downloads the model)..." -ForegroundColor Yellow
    $testOut = Join-Path $outputDir "test.wav"
    Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from Chatterbox" --voice default --output "$testOut"
    if (-not (Test-AudioFile -Path $testOut)) {
        Write-Host "  Smoke test FAILED." -ForegroundColor Red
        exit 1
    }
}

Write-Host ""
Write-Host "  Chatterbox installed at: $EngineDir" -ForegroundColor Green

$profileJson = @"
{
  "schema_version": 1,
  "id": "chatterbox-local",
  "name": "Chatterbox (Local)",
  "engine": "local",
  "voice": "default",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": { "preset": "chatterbox" }
}
"@
Write-ProfileSnippet -Json $profileJson

Write-Host "  Equivalent CopySpeak local CLI config:" -ForegroundColor Cyan
Write-Host '  command:       uv' -ForegroundColor Gray
Write-Host '  args_template: ["run","--project","{engine_dir}/chatterbox","python",' -ForegroundColor Gray
Write-Host '                 "scripts/copyspeak-chatterbox.py","--text-file","{input}",' -ForegroundColor Gray
Write-Host '                 "--voice","{voice}","--output","{output}"]' -ForegroundColor Gray
Write-Host ""
