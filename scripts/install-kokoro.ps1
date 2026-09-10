#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Kokoro TTS for CopySpeak via uv.

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeak\engines\kokoro,
    installs kokoro-onnx and Misaki English, drops the CLI wrapper, and exports
    a duration-capable ONNX model using the pinned upstream exporter. The first
    installation also downloads the Kokoro checkpoint and export dependencies.

    The third-party `kokoro-tts` CLI this used to install reloaded the 310 MB
    model on every invocation and gave no way to pick an execution provider, so
    neither the resident daemon nor GPU inference could reach it. Talking to
    kokoro-onnx through our own wrapper gets both.

    Prompts the user to pick a default English voice, baked into the profile
    snippet.

.PARAMETER Force
    Recreate the engine project from scratch.

.PARAMETER Cuda
    Also install onnxruntime-gpu and the NVIDIA CUDA/cuDNN runtime wheels into
    the engine project, then verify with a real GPU synthesis.

.PARAMETER SmokeTest
    Synthesize one clip after install to verify.

.PARAMETER SkipModelDownload
    Do not download the model files; just install the package. Synthesis will
    fail until the files are present, so this is for offline/reuse scenarios.

.EXAMPLE
    ./scripts/install-kokoro.ps1
    ./scripts/install-kokoro.ps1 -SmokeTest
    ./scripts/install-kokoro.ps1 -Cuda
#>

param(
    [switch]$Force,
    [switch]$Cuda,
    [switch]$SmokeTest,
    [switch]$SkipModelDownload,
    # App-driven voice selection: bypasses the interactive menu. The first id
    # is the profile-snippet default; all Kokoro voices share the one model.
    [string[]]$Voices
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Kokoro TTS Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
# -Voices (app-driven) is non-interactive: never destructively reinstall.
$effectiveForce = if ($Force) {
    $true
} elseif ($Voices) {
    $false
} else {
    Get-Confirmation -Prompt "Reinstall Kokoro from scratch? (deletes the existing engine dir)" -DefaultYes:$false
}

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "kokoro"
# ponytail: models/ sits inside the engine dir, so -Force would re-download
# 335 MB. Stash the model files across the reset and put them back.
$modelsDir = Join-Path $EngineDir "models"
$stash = $null
if ($effectiveForce -and (Test-Path $modelsDir)) {
    $stash = Join-Path $env:TEMP "copyspeak-kokoro-models-$(Get-Random)"
    Move-Item $modelsDir $stash
}
New-EngineProject -EngineDir $EngineDir -Force:$effectiveForce
if ($stash) {
    Move-Item $stash $modelsDir
    Write-Host "  Kept existing model files (no re-download)." -ForegroundColor Gray
}

Write-Host ""
Write-Host "  [STEP] engine" -ForegroundColor Yellow
$engineOk = $true
try {
    Write-Host "  Installing Kokoro and its English caption frontend..." -ForegroundColor Gray
    Invoke-Uv add --project $EngineDir "kokoro-onnx==0.6.1" "misaki[en]==0.9.4" "en-core-web-sm @ https://github.com/explosion/spacy-models/releases/download/en_core_web_sm-3.8.0/en_core_web_sm-3.8.0-py3-none-any.whl"
} catch {
    Write-Host "  [ERROR] engine (uv add failed: $_)" -ForegroundColor Red
    $engineOk = $false
}

$scriptsDir = Join-Path $EngineDir "scripts"
$outputDir = Join-Path $EngineDir "output"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null
New-Item -ItemType Directory -Force $modelsDir | Out-Null

$srcWrapper = Join-Path $PSScriptRoot "kokoro/copyspeak-kokoro.py"
$dstWrapper = Join-Path $scriptsDir "copyspeak-kokoro.py"
Copy-Item $srcWrapper $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray
if ($engineOk) { Write-Host "  [DONE] engine" -ForegroundColor Green }

# Model files. kokoro-onnx requires these and does not auto-download them.
# Stable home is <engine_dir>/models/ so the wrapper resolves them relative to
# itself, the same way piper resolves its voices/.
$modelFile = Join-Path $modelsDir "kokoro-v1.0-duration.onnx"
$voicesFile = Join-Path $modelsDir "voices-v1.0.bin"
$voicesUrl = "https://github.com/nazdridoy/kokoro-tts/releases/download/v1.0.0/voices-v1.0.bin"

if (-not $SkipModelDownload) {
    Write-Host "  [STEP] model" -ForegroundColor Yellow
    $modelOk = $true
    foreach ($f in @(
        @{ Path = $voicesFile; Url = $voicesUrl; Label = "voices-v1.0.bin (~25 MB)" }
    )) {
        if (Test-Path $f.Path) {
            Write-Host "  Already present: $($f.Path)" -ForegroundColor Green
            continue
        }
        Write-Host ""
        Write-Host "  Downloading $($f.Label)..." -ForegroundColor Yellow
        Write-Host "    -> $($f.Path)" -ForegroundColor Gray
        try {
            Invoke-WebRequest -Uri $f.Url -OutFile $f.Path -UseBasicParsing
        } catch {
            # Drop the partial file; otherwise the next run sees it as present.
            Remove-Item -Force -ErrorAction SilentlyContinue $f.Path
            Write-Host "  WARNING: download failed: $_" -ForegroundColor Red
            Write-Host "  Re-run with -Force, or download manually from $($f.Url)" -ForegroundColor Gray
            $modelOk = $false
        }
    }
    if ($engineOk -and -not (Test-Path $modelFile)) {
        # Keep the old audio-only model intact. Promote the new artifact only
        # after upstream's export and native-duration verification both succeed.
        $exportDir = Join-Path $modelsDir "native-export"
        New-Item -ItemType Directory -Force $exportDir | Out-Null
        $exportScript = Join-Path $exportDir "export.py"
        $exportConfig = Join-Path $exportDir "config.json"
        $checkpoint = Join-Path $exportDir "kokoro-v1_0.pth"
        $pendingModel = Join-Path $exportDir "kokoro-v1.0-duration.onnx"
        $checkpointBase = "https://huggingface.co/hexgrad/Kokoro-82M/resolve/f3ff3571791e39611d31c381e3a41a3af07b4987"
        try {
            foreach ($source in @(
                @{ Path = $exportScript; Url = "https://raw.githubusercontent.com/thewh1teagle/kokoro-onnx/3596b26764286a7de9d90c363e988d50578918e5/scripts/export.py" },
                @{ Path = $exportConfig; Url = "$checkpointBase/config.json" },
                @{ Path = $checkpoint; Url = "$checkpointBase/kokoro-v1_0.pth" }
            )) {
                if (-not (Test-Path $source.Path)) {
                    $partial = $source.Path + ".download"
                    Invoke-WebRequest -Uri $source.Url -OutFile $partial -UseBasicParsing
                    Move-Item -LiteralPath $partial -Destination $source.Path -Force
                }
            }
            # Kokoro's export package requires NumPy 1.x, which has no Python
            # 3.13 wheel. Keep this build environment separate from inference.
            Invoke-Uv run --no-project --python 3.12 --with "onnx==1.22.0" --with "onnxscript==0.7.2" --with "onnxruntime==1.29.0" --with "torch==2.14.0" --with "transformers==5.17.0" $exportScript --config $exportConfig --checkpoint $checkpoint --output $pendingModel
            Move-Item -LiteralPath $pendingModel -Destination $modelFile
        } catch {
            Write-Host "  [ERROR] native caption model export: $_" -ForegroundColor Red
            $modelOk = $false
        }
    }
    if ($modelOk -and (Test-Path $modelFile) -and (Test-Path $voicesFile)) {
        Write-Host "  [DONE] model" -ForegroundColor Green
    } else {
        Write-Host "  [ERROR] model (one or more files missing)" -ForegroundColor Red
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
# All listed voices share the one model file; the default is just the
# profile-snippet pick.
$chosenVoice = if ($Voices -and $Voices.Count -gt 0) { $Voices[0] } else { Select-VoiceFromMenu -Title "Pick a default Kokoro voice" -Voices $kokoroVoices -Default "af_heart" }

$kokoroOk = $engineOk -and (Test-Path $modelFile) -and (Test-Path $voicesFile)

if ($Cuda -and $kokoroOk) {
    Write-Host ""
    if (Add-CudaRuntime -EngineDir $EngineDir) {
        Test-CudaSynthesis -EngineDir $EngineDir -Wrapper $dstWrapper -Voice $chosenVoice | Out-Null
    }
}

if ($SmokeTest -and $kokoroOk) {
    Write-Host ""
    Write-Host "  [STEP] smoke" -ForegroundColor Yellow
    $testOut = Join-Path $outputDir "test.wav"
    Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from Kokoro" --voice $chosenVoice --output "$testOut"
    if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  [ERROR] smoke" -ForegroundColor Red; exit 1 }
    Write-Host "  [DONE] smoke" -ForegroundColor Green
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "kokoro-local",
  "name": "Kokoro (Local)",
  "engine": "kokoro",
  "voice": "$chosenVoice",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": { "engine": "kokoro", "cuda": false }
}
"@

if ($kokoroOk) {
    Write-EngineManifest -EngineDir $EngineDir -VoicesInstalled ($kokoroVoices.Id)
}

Write-Host ""
if ($kokoroOk) {
    Write-Host "  Kokoro installed at: $EngineDir" -ForegroundColor Green
    Write-ProfileSnippet -Json $profileJson
} else {
    Write-Host "  [ERROR] engine (kokoro-onnx or model files missing - re-run without -SkipModelDownload)" -ForegroundColor Red
    exit 1
}
