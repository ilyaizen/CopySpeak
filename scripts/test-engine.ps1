#Requires -Version 5.1
<#
.SYNOPSIS
    Smoke-tests an installed CopySpeak uv engine by synthesizing one WAV.

.PARAMETER Engine
    Engine name under %LOCALAPPDATA%\CopySpeak\engines (e.g. piper).

.PARAMETER Text
    Text to synthesize. Default: "CopySpeak engine test".

.PARAMETER Voice
    Voice name. Default: default.

.PARAMETER Device
    cpu (default) or cuda. `cuda` needs the engine installed with -Cuda, and is
    the only honest check that GPU inference works — get_available_providers()
    reports CUDA even when its DLLs are missing.

.EXAMPLE
    ./scripts/test-engine.ps1 -Engine piper
#>

param(
    [Parameter(Mandatory)][string]$Engine,
    [string]$Text = "CopySpeak engine test",
    [string]$Voice = "default",
    [ValidateSet("cpu", "cuda")][string]$Device = "cpu"
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Require-Uv

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) $Engine
if (-not (Test-Path $EngineDir)) {
    Write-Host "ERROR: engine not installed: $EngineDir" -ForegroundColor Red
    Write-Host "  Install it first, e.g. ./scripts/install-$Engine.ps1" -ForegroundColor Yellow
    exit 1
}

$wrapper = Join-Path $EngineDir "scripts/copyspeak-$Engine.py"
if (-not (Test-Path $wrapper)) {
    Write-Host "ERROR: wrapper not found: $wrapper" -ForegroundColor Red
    exit 1
}

$outDir = Join-Path $EngineDir "output"
New-Item -ItemType Directory -Force $outDir | Out-Null
$out = Join-Path $outDir "test.wav"

Write-Host "  Synthesizing with $Engine on $Device..." -ForegroundColor Gray
Invoke-Uv run --project $EngineDir python "$wrapper" --text "$Text" --voice "$Voice" --output "$out" --device $Device

if (Test-AudioFile -Path $out) {
    Write-Host "  $Engine engine OK." -ForegroundColor Green
} else {
    exit 1
}
