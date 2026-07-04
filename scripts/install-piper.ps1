#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Piper (piper1-gpl) for CopySpeak via uv.

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeak\engines\piper,
    installs the `piper` package, and drops the stable CLI wrapper.

    Piper needs a voice model (.onnx + .onnx.json) per voice. Place model pairs
    in %LOCALAPPDATA%\CopySpeak\engines\piper\voices and pass the model basename
    (without extension) as the profile voice. The installer does NOT download
    models automatically; see https://github.com/OHF-Voice/piper1-gpl for voices.

.PARAMETER Force
    Recreate the engine project from scratch.

.PARAMETER SmokeTest
    Synthesize one clip after install (requires a model in voices/).

.EXAMPLE
    ./scripts/install-piper.ps1
#>

param(
    [switch]$Force,
    [switch]$SmokeTest
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Piper TTS Installer"

Require-Uv

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "piper"
New-EngineProject -EngineDir $EngineDir -Force:$Force

Write-Host ""
Write-Host "  Installing Piper..." -ForegroundColor Gray
Invoke-Uv add --project $EngineDir "piper"

$scriptsDir = Join-Path $EngineDir "scripts"
$voicesDir = Join-Path $EngineDir "voices"
$outputDir = Join-Path $EngineDir "output"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $voicesDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null

$srcWrapper = Join-Path $PSScriptRoot "piper/copyspeak-piper.py"
$dstWrapper = Join-Path $scriptsDir "copyspeak-piper.py"
Copy-Item $srcWrapper $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray

Write-Host ""
Write-Host "  Piper needs voice models. Drop <voice>.onnx + <voice>.onnx.json into:" -ForegroundColor Yellow
Write-Host "    $voicesDir" -ForegroundColor Gray
Write-Host "  Get voices from https://github.com/OHF-Voice/piper1-gpl#voices" -ForegroundColor Gray

if ($SmokeTest) {
    $model = Get-ChildItem -Path $voicesDir -Filter "*.onnx" -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $model) {
        Write-Host "  Smoke test skipped: no .onnx model in $voicesDir" -ForegroundColor Yellow
    } else {
        $voiceName = [IO.Path]::GetFileNameWithoutExtension($model.Name)
        $testOut = Join-Path $outputDir "test.wav"
        Write-Host "  Running smoke test with voice '$voiceName'..." -ForegroundColor Yellow
        Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from Piper" --voice $voiceName --output "$testOut"
        if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
    }
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "piper-local",
  "name": "Piper (Local)",
  "engine": "local",
  "voice": "en_US-joe-medium",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "piper",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/piper", "python", "scripts/copyspeak-piper.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}"]
  }
}
"@

Write-Host ""
Write-Host "  Piper installed at: $EngineDir" -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
