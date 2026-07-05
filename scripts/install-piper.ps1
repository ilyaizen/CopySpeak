#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Piper (piper1-gpl) for CopySpeak via uv.

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeak\engines\piper,
    installs the `piper` package, and drops the stable CLI wrapper.

    Prompts the user to pick an English voice and downloads the matching
    (.onnx + .onnx.json) pair from HuggingFace into the engine's voices\ dir
    (use -SkipVoiceDownload to skip). The chosen voice is used for the smoke
    test and baked into the emitted profile snippet.

.PARAMETER Force
    Recreate the engine project from scratch. When omitted, the installer
    still prompts interactively ("Reinstall from scratch?") — answering yes
    is equivalent to passing -Force.

.PARAMETER SmokeTest
    Synthesize one clip after install (requires a model in voices/).

.PARAMETER SkipVoiceDownload
    Do not download a voice model; just install the package + wrapper.

.EXAMPLE
    ./scripts/install-piper.ps1
    ./scripts/install-piper.ps1 -Force -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest,
    [switch]$SkipVoiceDownload
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Piper TTS Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
$effectiveForce = if ($Force) {
    $true
} else {
    Get-Confirmation -Prompt "Reinstall Piper from scratch? (deletes the existing engine dir)" -DefaultYes:$false
}

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "piper"
New-EngineProject -EngineDir $EngineDir -Force:$effectiveForce

Write-Host ""
Write-Host "  Installing Piper..." -ForegroundColor Gray
# ponytail: PyPI `piper` is an unrelated bioinformatics toolkit (databio/pypiper,
# module `pypiper`). The TTS engine ships as `piper-tts` (module `piper`).
Invoke-Uv add --project $EngineDir "piper-tts"

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

# Curated English (en_US) voices from rhasspy/piper-voices on HuggingFace.
# ponytail: the voice Id is also the model basename and the URL segment.
$piperVoices = @(
    @{ Id = "en_US-amy-medium";     Label = "Amy (female, medium)" },
    @{ Id = "en_US-lessac-medium";  Label = "Lessac (female, medium)" },
    @{ Id = "en_US-ryan-medium";    Label = "Ryan (male, medium)" },
    @{ Id = "en_US-joe-medium";     Label = "Joe (male, medium)" },
    @{ Id = "en_US-libritts-medium"; Label = "LibriTTS (mixed, medium)" }
)
$voiceBaseUrl = "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/en/en_US"

# Resolve which voice to use. Skip the menu only when the caller explicitly
# opted out of voice download AND a model already exists.
$existingModel = Get-ChildItem -Path $voicesDir -Filter "*.onnx" -ErrorAction SilentlyContinue | Select-Object -First 1
$chosenVoice = if ($SkipVoiceDownload -and $existingModel) {
    [IO.Path]::GetFileNameWithoutExtension($existingModel.Name)
} else {
    Select-VoiceFromMenu -Title "Pick an English Piper voice" -Voices $piperVoices -Default "en_US-amy-medium"
}

# Download the chosen model pair if missing (and not opted out).
$modelPath = Join-Path $voicesDir "$chosenVoice.onnx"
$configPath = Join-Path $voicesDir "$chosenVoice.onnx.json"
if (-not $SkipVoiceDownload -and (-not (Test-Path $modelPath) -or -not (Test-Path $configPath))) {
    # Derive <voice>/<quality> from "en_US-<voice>-<quality>" for the URL.
    $parts = $chosenVoice -split "-"
    if ($parts.Count -ge 3) {
        $voiceName = $parts[1]
        $quality = $parts[2]
        $onnxUrl = "$voiceBaseUrl/$voiceName/$quality/$chosenVoice.onnx"
        $jsonUrl = "$voiceBaseUrl/$voiceName/$quality/$chosenVoice.onnx.json"
        Write-Host ""
        Write-Host "  Downloading voice model: $chosenVoice" -ForegroundColor Yellow
        Write-Host "    -> $modelPath" -ForegroundColor Gray
        try {
            if (-not (Test-Path $modelPath)) {
                Invoke-WebRequest -Uri $onnxUrl -OutFile $modelPath -UseBasicParsing
            }
            if (-not (Test-Path $configPath)) {
                Invoke-WebRequest -Uri $jsonUrl -OutFile $configPath -UseBasicParsing
            }
            Write-Host "  Voice downloaded." -ForegroundColor Green
        } catch {
            Write-Host "  WARNING: voice download failed: $_" -ForegroundColor Red
            Write-Host "  You can download $chosenVoice manually from https://huggingface.co/rhasspy/piper-voices" -ForegroundColor Gray
        }
    }
} else {
    Write-Host "  Voice model already present: $chosenVoice" -ForegroundColor Green
}

Write-Host ""
Write-Host "  Voices directory: $voicesDir" -ForegroundColor Gray
Write-Host "  More voices:      https://github.com/OHF-Voice/piper1-gpl#voices" -ForegroundColor Gray

if ($SmokeTest) {
    if (-not (Test-Path $modelPath)) {
        Write-Host "  Smoke test skipped: no .onnx model in $voicesDir" -ForegroundColor Yellow
    } else {
        $testOut = Join-Path $outputDir "test.wav"
        Write-Host "  Running smoke test with voice '$chosenVoice'..." -ForegroundColor Yellow
        Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from Piper" --voice $chosenVoice --output "$testOut"
        if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
    }
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "piper-local",
  "name": "Piper (Local)",
  "engine": "local",
  "voice": "$chosenVoice",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "piper",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/piper", "python", "{engine_dir}/piper/scripts/copyspeak-piper.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}"]
  }
}
"@

Write-Host ""
Write-Host "  Piper installed at: $EngineDir" -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
