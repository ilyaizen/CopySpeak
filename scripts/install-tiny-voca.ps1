#Requires -Version 5.1
<#
.SYNOPSIS
    Installs tiny-voca for CopySpeech via uv (CPU-only, about 45 MB of weights).

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeak\engines\tiny-voca,
    installs ONNX Runtime and the dizione phonemizer (pinned commit), downloads the
    ONNX model and phonemizer data from n4ze3m/tiny-voca, checks the model's
    sha256, and drops the stable CLI wrapper. No PyTorch, no CUDA.

.PARAMETER Force
    Recreate the engine project from scratch.

.PARAMETER SmokeTest
    Synthesize one clip after install.

.PARAMETER Voices
    App-driven voice selection. The first id that is F2 or M2 becomes the
    profile-snippet default. Both voices ship in one model, so nothing else
    depends on it.

.EXAMPLE
    ./scripts/install-tiny-voca.ps1
    ./scripts/install-tiny-voca.ps1 -Force -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$SmokeTest,
    [string[]]$Voices
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeech-engine-install.ps1"

$Voices = ConvertTo-VoiceIds $Voices

Write-EngineBanner -Title "tiny-voca Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
# -Voices (app-driven) is non-interactive: never destructively reinstall.
$effectiveForce = if ($Force) {
    $true
} elseif ($Voices) {
    $false
} else {
    Get-Confirmation -Prompt "Reinstall tiny-voca from scratch? (deletes the existing engine dir)" -DefaultYes:$false
}

$EngineDir = Join-Path (Get-CopySpeechEngineRoot) "tiny-voca"
New-EngineProject -EngineDir $EngineDir -Force:$effectiveForce

# dizione is the phonemizer tiny-voca needs; pinned to a commit, not a branch.
$Diziones = "dizione @ git+https://github.com/n4ze3m/dizione@8cd889e4de7be6bd1dd2e6891344a0729fa14dd4"
# The model's ONNX file, sha256 from the Hugging Face LFS pointer.
$ModelSha256 = "fe6d918791a15b82e37465981d7f27a070e46b46635e5a3482fc8dff87a1ae46"

Write-Host ""
Write-Host "  [STEP] engine" -ForegroundColor Yellow
Write-Host "  Installing ONNX Runtime, numpy, soundfile, dizione..." -ForegroundColor Gray
Invoke-Uv add --project $EngineDir "onnxruntime" "numpy" "soundfile" "huggingface-hub>=0.34,<2" $Diziones

$scriptsDir = Join-Path $EngineDir "scripts"
$voicesDir = Join-Path $EngineDir "voices"
$outputDir = Join-Path $EngineDir "output"
$weightsDir = Join-Path $EngineDir "weights"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $voicesDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null

Write-Host "  Downloading weights (about 45 MB)..." -ForegroundColor Gray
Invoke-Uv run --project $EngineDir python (Join-Path $PSScriptRoot "tiny-voca/fetch-weights.py") --dest $weightsDir

$model = Join-Path $weightsDir "onnx\model.onnx"
if (-not (Test-Path $model)) { throw "tiny-voca model missing after download: $model" }
$actualSha = (Get-FileHash -Algorithm SHA256 -Path $model).Hash.ToLower()
if ($actualSha -ne $ModelSha256) {
    throw "tiny-voca model sha256 mismatch: got $actualSha, expected $ModelSha256"
}
Write-Host "  Model sha256 verified." -ForegroundColor Gray

$dstWrapper = Join-Path $scriptsDir "copyspeech-tiny-voca.py"
Copy-Item (Join-Path $PSScriptRoot "tiny-voca/copyspeech-tiny-voca.py") $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray
Write-Host "  [DONE] engine" -ForegroundColor Green

# Both voices ship in the one model; F2 is the CLI default upstream.
$tinyVoices = @(
    @{ Id = "F2"; Label = "F2 (female)" },
    @{ Id = "M2"; Label = "M2 (male)" }
)
$chosenVoice = if ($Voices -and ($Voices[0] -in @("F2", "M2"))) { $Voices[0] } else { "M2" }

if ($SmokeTest) {
    Write-Host ""
    Write-Host "  Running smoke test..." -ForegroundColor Yellow
    $testOut = Join-Path $outputDir "test.wav"
    Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from tiny voca." --voice $chosenVoice --output "$testOut"
    if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "tiny-voca-local",
  "name": "tiny-voca (Local, English)",
  "engine": "local",
  "voice": "$chosenVoice",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "tiny-voca",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/tiny-voca", "python", "{engine_dir}/tiny-voca/scripts/copyspeech-tiny-voca.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}", "--speed", "{speed}"]
  }
}
"@

Write-EngineManifest -EngineDir $EngineDir -VoicesInstalled ($tinyVoices.Id)

Write-Host ""
Write-Host "  tiny-voca installed at: $EngineDir" -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
