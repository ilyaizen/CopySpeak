#Requires -Version 5.1
<#
.SYNOPSIS
    Installs kale-tts-1-nano for CopySpeech via uv (CPU-only, about 29 MB).

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeech\engines\kale,
    installs ONNX Runtime, numpy, and quickjs on Python 3.12 (the newest
    release quickjs 1.19.4 ships wheels for), downloads the int8 ONNX model,
    voice tables, G2P front-end data, and runtime files from
    KaleComputer/kale-tts-1-nano, checks the model's sha256, and drops the
    stable CLI wrapper. No PyTorch, no CUDA - the graph is CPU-only.

    Licensing: weights and code Apache-2.0, dataset CC BY 4.0. The training
    audio is Maya-1 model output (Maya-1 use policy): the voices must never be
    presented as a recording of a human being, and must not be used to
    impersonate anyone.

.PARAMETER Force
    Recreate the engine project from scratch.

.PARAMETER SmokeTest
    Synthesize one clip after install.

.PARAMETER Voices
    App-driven voice selection. Accepted for the shared install-dialog
    contract; all ten voices ship in one model, so nothing depends on it (the
    first requested id becomes the profile-snippet default when recognized).

.EXAMPLE
    ./scripts/install-kale.ps1
    ./scripts/install-kale.ps1 -Force -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest,
    [string[]]$Voices
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeech-engine-install.ps1"

$Voices = ConvertTo-VoiceIds $Voices

Write-EngineBanner -Title "kale-tts-1-nano Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
# -Voices (app-driven) is non-interactive: never destructively reinstall.
$effectiveForce = if ($Force) {
    $true
} elseif ($Voices) {
    $false
} else {
    Get-Confirmation -Prompt "Reinstall kale from scratch? (deletes the existing engine dir)" -DefaultYes:$false
}

$EngineDir = Join-Path (Get-CopySpeechEngineRoot) "kale"
New-EngineProject -EngineDir $EngineDir -Force:$effectiveForce

# quickjs 1.19.4 ships wheels only up to cp312, so the project pins Python
# 3.12. uv init wrote the host Python as the floor; lower it to match.
$pyproject = Join-Path $EngineDir "pyproject.toml"
if (Test-Path $pyproject) {
    $content = Get-Content $pyproject -Raw
    $content = $content -replace 'requires-python = .*', 'requires-python = ">=3.12"'
    Set-Content $pyproject $content -Encoding ascii
}

# The int8 CPU ONNX file, sha256 from the Hugging Face SHA256SUMS.
$ModelSha256 = "4d5269974de4fba7a66f947608a9a5381b4e39c1a8a6073df6001ab7e7f76288"

Write-Host ""
Write-Host "  [STEP] engine" -ForegroundColor Yellow
Write-Host "  Installing ONNX Runtime, numpy, quickjs (Python 3.12)..." -ForegroundColor Gray
Invoke-Uv add --project $EngineDir --python 3.12 "onnxruntime" "numpy" "quickjs" "huggingface-hub>=0.34,<2"

$scriptsDir = Join-Path $EngineDir "scripts"
$voicesDir = Join-Path $EngineDir "voices"
$outputDir = Join-Path $EngineDir "output"
$weightsDir = Join-Path $EngineDir "weights"
$frontendDir = Join-Path $EngineDir "frontend"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $voicesDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null

Write-Host "  Downloading weights and G2P data (about 29 MB)..." -ForegroundColor Gray
Invoke-Uv run --project $EngineDir python (Join-Path $PSScriptRoot "kale/fetch-weights.py") --dest $weightsDir

$model = Join-Path $weightsDir "kale-tts-1-nano.onnx"
if (-not (Test-Path $model)) { throw "kale model missing after download: $model" }
$actualSha = (Get-FileHash -Algorithm SHA256 -Path $model).Hash.ToLower()
if ($actualSha -ne $ModelSha256) {
    throw "kale model sha256 mismatch: got $actualSha, expected $ModelSha256"
}
Write-Host "  Model sha256 verified." -ForegroundColor Gray

# The runtime beside the wrapper (its _near walk then finds frontend/ one
# level up), and the G2P word data at the engine root.
Move-Item (Join-Path $weightsDir "python/kale_tts.py") (Join-Path $scriptsDir "kale_tts.py") -Force
Move-Item (Join-Path $weightsDir "python/kale_frontend.js") (Join-Path $scriptsDir "kale_frontend.js") -Force
Remove-Item (Join-Path $weightsDir "python") -Recurse -Force -ErrorAction SilentlyContinue
Move-Item (Join-Path $weightsDir "frontend") $frontendDir -Force
if (-not (Test-Path (Join-Path $frontendDir "words/g2p.bin"))) {
    throw "kale front-end data missing after download: $frontendDir\words"
}

$dstWrapper = Join-Path $scriptsDir "copyspeech-kale.py"
Copy-Item (Join-Path $PSScriptRoot "kale/copyspeech-kale.py") $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray
Write-Host "  [DONE] engine" -ForegroundColor Green

# All ten voices ship in the one model; Kenzie is the catalog's first voice.
$kaleVoices = @(
    @{ Id = "Kenzie"; Label = "Kenzie (US, esports hype woman)" },
    @{ Id = "Simone"; Label = "Simone (US, deep and confident woman)" },
    @{ Id = "Amy"; Label = "Amy (British, crisp news-style)" },
    @{ Id = "Frankie"; Label = "Frankie (US, detective's partner)" },
    @{ Id = "Darlene"; Label = "Darlene (US, rough low-voiced trucker)" },
    @{ Id = "Owen"; Label = "Owen (US, bright young host)" },
    @{ Id = "Toby"; Label = "Toby (US, mischievous prankster)" },
    @{ Id = "Jack"; Label = "Jack (British, crisp news-style)" },
    @{ Id = "Earl"; Label = "Earl (US, jazz granddad)" },
    @{ Id = "Magnus"; Label = "Magnus (US, deep trailer narrator)" }
)
$kaleIds = $kaleVoices | ForEach-Object { $_.Id }
$chosenVoice = if ($Voices -and ($Voices[0] -in $kaleIds)) { $Voices[0] } else { "Kenzie" }

if ($SmokeTest) {
    Write-Host ""
    Write-Host "  Running smoke test..." -ForegroundColor Yellow
    $testOut = Join-Path $outputDir "test.wav"
    Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from kale." --voice $chosenVoice --output "$testOut"
    if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "kale-local",
  "name": "kale (Local, English)",
  "engine": "local",
  "voice": "$chosenVoice",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "kale",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/kale", "python", "{engine_dir}/kale/scripts/copyspeech-kale.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}", "--speed", "{speed}"]
  }
}
"@

Write-EngineManifest -EngineDir $EngineDir -VoicesInstalled $kaleIds

Write-Host ""
Write-Host "  kale-tts-1-nano installed at: $EngineDir" -ForegroundColor Green
Write-Host "  Voice note (Maya-1 use policy): the voices are synthetic designs;" -ForegroundColor Gray
Write-Host "  never present their output as a recording of a human being." -ForegroundColor Gray
Write-ProfileSnippet -Json $profileJson
