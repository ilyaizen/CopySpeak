#Requires -Version 5.1
<#
.SYNOPSIS
    Installs KittenTTS for CopySpeak via uv (CPU-only, 25-80MB).

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeak\engines\kitten,
    installs the KittenTTS wheel + soundfile, and drops the stable CLI wrapper.

.PARAMETER Force
    Recreate the engine project from scratch.

.PARAMETER SmokeTest
    Synthesize one clip after install (first run downloads the model).

.EXAMPLE
    ./scripts/install-kittentts.ps1
    ./scripts/install-kittentts.ps1 -Force -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Kitten TTS Installer"

Require-Uv

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "kitten"
New-EngineProject -EngineDir $EngineDir -Force:$Force

# KittenTTS is published as a GitHub release wheel (not on PyPI).
# ponytail: pin the URL; bump here when KittenML cuts a new release.
$WheelUrl = "https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl"

Write-Host ""
Write-Host "  Installing KittenTTS + soundfile..." -ForegroundColor Gray
Invoke-Uv add --project $EngineDir $WheelUrl "soundfile"

$scriptsDir = Join-Path $EngineDir "scripts"
$voicesDir = Join-Path $EngineDir "voices"
$outputDir = Join-Path $EngineDir "output"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $voicesDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null

$srcWrapper = Join-Path $PSScriptRoot "kitten/copyspeak-kitten.py"
$dstWrapper = Join-Path $scriptsDir "copyspeak-kitten.py"
Copy-Item $srcWrapper $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray

if ($SmokeTest) {
    Write-Host ""
    Write-Host "  Running smoke test (first run downloads the model)..." -ForegroundColor Yellow
    $testOut = Join-Path $outputDir "test.wav"
    Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from Kitten TTS" --voice Rosie --output "$testOut"
    if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "kitten-local",
  "name": "Kitten TTS (Local)",
  "engine": "local",
  "voice": "Rosie",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "kitten-tts",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/kitten", "python", "{engine_dir}/kitten/scripts/copyspeak-kitten.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}"]
  }
}
"@

Write-Host ""
Write-Host "  Kitten TTS installed at: $EngineDir" -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
