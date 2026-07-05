#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Kokoro TTS (kokoro-tts CLI) for CopySpeak via uv.

.DESCRIPTION
    Installs kokoro-tts as a uv tool so the `kokoro-tts` binary is on PATH,
    then downloads the required model files (kokoro-v1.0.onnx + voices-v1.0.bin)
    into <engine_dir>/kokoro/models/. The kokoro-tts binary does NOT bundle or
    auto-download these — synthesis fails without them — so the installer
    fetches them up front (~335 MB total) and the args_template points at them
    via --model/--voices.

    Prompts the user to pick a default English voice, baked into the profile
    snippet.

.PARAMETER Force
    Reinstall even if kokoro-tts is already available. When omitted, the
    installer still prompts interactively ("Reinstall?").

.PARAMETER SmokeTest
    Synthesize one clip after install to verify.

.PARAMETER SkipModelDownload
    Do not download the model files; just install the binary. Synthesis will
    fail until the files are present, so this is for offline/reuse scenarios.

.EXAMPLE
    ./scripts/install-kokoro.ps1
    ./scripts/install-kokoro.ps1 -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest,
    [switch]$SkipModelDownload
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Kokoro TTS Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
$alreadyInstalled = [bool](Get-Command kokoro-tts -ErrorAction SilentlyContinue)
$effectiveForce = if ($Force) {
    $true
} elseif (-not $alreadyInstalled) {
    $false
} else {
    Get-Confirmation -Prompt "kokoro-tts is already installed. Reinstall from scratch?" -DefaultYes:$false
}

if (-not $effectiveForce -and $alreadyInstalled) {
    Write-Host "  kokoro-tts already installed." -ForegroundColor Green
    Write-Host "  Use -Force or answer Yes to reinstall." -ForegroundColor Yellow
} else {
    Write-Host "  Installing kokoro-tts via uv tool..." -ForegroundColor Gray
    Invoke-Uv tool install kokoro-tts --force
}

# Model files. kokoro-tts requires these and does not auto-download them.
# Stable home is <engine_dir>/kokoro/models/ so {engine_dir} resolves them
# the same way it resolves piper's wrapper.
$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "kokoro"
$modelsDir = Join-Path $EngineDir "models"
New-Item -ItemType Directory -Force $modelsDir | Out-Null

$modelFile = Join-Path $modelsDir "kokoro-v1.0.onnx"
$voicesFile = Join-Path $modelsDir "voices-v1.0.bin"
$modelUrl = "https://github.com/nazdridoy/kokoro-tts/releases/download/v1.0.0/kokoro-v1.0.onnx"
$voicesUrl = "https://github.com/nazdridoy/kokoro-tts/releases/download/v1.0.0/voices-v1.0.bin"

if (-not $SkipModelDownload) {
    if (-not (Test-Path $modelFile)) {
        Write-Host ""
        Write-Host "  Downloading kokoro-v1.0.onnx (~310 MB, full quality)..." -ForegroundColor Yellow
        Write-Host "    -> $modelFile" -ForegroundColor Gray
        try {
            Invoke-WebRequest -Uri $modelUrl -OutFile $modelFile -UseBasicParsing
        } catch {
            Write-Host "  WARNING: model download failed: $_" -ForegroundColor Red
            Write-Host "  Re-run with -Force, or download manually from $modelUrl" -ForegroundColor Gray
        }
    } else {
        Write-Host "  Model already present: $modelFile" -ForegroundColor Green
    }
    if (-not (Test-Path $voicesFile)) {
        Write-Host ""
        Write-Host "  Downloading voices-v1.0.bin (~25 MB)..." -ForegroundColor Yellow
        Write-Host "    -> $voicesFile" -ForegroundColor Gray
        try {
            Invoke-WebRequest -Uri $voicesUrl -OutFile $voicesFile -UseBasicParsing
        } catch {
            Write-Host "  WARNING: voices download failed: $_" -ForegroundColor Red
            Write-Host "  Re-run with -Force, or download manually from $voicesUrl" -ForegroundColor Gray
        }
    } else {
        Write-Host "  Voices already present: $voicesFile" -ForegroundColor Green
    }
}

# Kokoro ships many built-in voices; en_* voice ids: af_* = American female,
# am_* = American male, bf_* = British female, bm_* = British male.
$kokoroVoices = @(
    @{ Id = "af_heart";    Label = "Heart (American female, flagship)" },
    @{ Id = "af_bella";    Label = "Bella (American female)" },
    @{ Id = "af_nicole";   Label = "Nicole (American female)" },
    @{ Id = "af_sarah";    Label = "Sarah (American female)" },
    @{ Id = "am_adam";     Label = "Adam (American male)" },
    @{ Id = "am_michael";  Label = "Michael (American male)" },
    @{ Id = "bf_emma";     Label = "Emma (British female)" },
    @{ Id = "bm_george";   Label = "George (British male)" }
)
$chosenVoice = Select-VoiceFromMenu -Title "Pick a default Kokoro voice" -Voices $kokoroVoices -Default "af_heart"

$profileJson = @"
{
  "schema_version": 1,
  "id": "kokoro-local",
  "name": "Kokoro (Local)",
  "engine": "local",
  "voice": "$chosenVoice",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "kokoro-tts",
    "command": "kokoro-tts",
    "args_template": ["{input}", "{output}", "--voice", "{voice}", "--model", "{engine_dir}/kokoro/models/kokoro-v1.0.onnx", "--voices", "{engine_dir}/kokoro/models/voices-v1.0.bin"]
  }
}
"@

if ($SmokeTest -and (Get-Command kokoro-tts -ErrorAction SilentlyContinue) -and (Test-Path $modelFile) -and (Test-Path $voicesFile)) {
    $out = Join-Path $env:TEMP "copyspeak-kokoro-test.wav"
    $txt = Join-Path $env:TEMP "copyspeak-kokoro-test.txt"
    "Hello from Kokoro" | Set-Content -Path $txt -Encoding utf8
    Write-Host ""
    Write-Host "  Smoke test..." -ForegroundColor Yellow
    kokoro-tts $txt $out --voice $chosenVoice --model $modelFile --voices $voicesFile
    if (-not (Test-AudioFile -Path $out)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

Write-Host ""
Write-Host "  Kokoro installed." -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
