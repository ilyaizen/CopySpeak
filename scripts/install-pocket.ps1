#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Kyutai Pocket TTS for CopySpeak via uv.

.DESCRIPTION
    Creates a uv-managed project under %LOCALAPPDATA%\CopySpeak\engines\pocket,
    installs pocket-tts, and drops the stable CLI wrapper. Model weights
    auto-download from Hugging Face on first synthesis.

    The `pocket-tts generate` CLI this used to install reloaded the model and
    re-derived the voice state on every invocation — the two slow steps. Talking
    to the Python API through our own wrapper keeps both resident.

.PARAMETER Force
    Recreate the engine project from scratch.

.PARAMETER Cuda
    Install a CUDA build of torch into the engine project instead of the CPU
    one, then verify with a real GPU synthesis. Pocket is designed for CPU and
    on a fast laptop chip the GPU is no quicker; it pays off mainly on
    thread-limited machines.

.PARAMETER SmokeTest
    Synthesize one clip after install to verify (first run downloads weights).

.EXAMPLE
    ./scripts/install-pocket.ps1
    ./scripts/install-pocket.ps1 -SmokeTest
#>

param(
    [switch]$Force,
    [switch]$Cuda,
    [switch]$SmokeTest,
    # App-driven voice selection: bypasses the interactive menu. The first id
    # is the profile-snippet default; all Pocket voices share the one model.
    [string[]]$Voices
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Pocket TTS Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
# -Voices (app-driven) is non-interactive: never destructively reinstall.
$effectiveForce = if ($Force) {
    $true
} elseif ($Voices) {
    $false
} else {
    Get-Confirmation -Prompt "Reinstall Pocket from scratch? (deletes the existing engine dir)" -DefaultYes:$false
}

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "pocket"
New-EngineProject -EngineDir $EngineDir -Force:$effectiveForce

Write-Host ""
Write-Host "  [STEP] engine" -ForegroundColor Yellow
Write-Host "  Installing pocket-tts..." -ForegroundColor Gray
# Windows PyPI torch wheels are already CPU-only, so no extra index is needed
# here; -Cuda swaps in a CUDA build below.
Invoke-Uv add --project $EngineDir "pocket-tts"

$scriptsDir = Join-Path $EngineDir "scripts"
$outputDir = Join-Path $EngineDir "output"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null

$srcWrapper = Join-Path $PSScriptRoot "pocket/copyspeak-pocket.py"
$dstWrapper = Join-Path $scriptsDir "copyspeak-pocket.py"
Copy-Item $srcWrapper $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray
Write-Host "  [DONE] engine" -ForegroundColor Green

# Kyutai's built-in voice catalog. Weights (and the voice prompts) download
# from Hugging Face on first synthesis.
$pocketVoices = @(
    @{ Id = "alba";     Label = "Alba (English, female)" },
    @{ Id = "anna";     Label = "Anna (English, female)" },
    @{ Id = "eve";      Label = "Eve (English, female)" },
    @{ Id = "jane";     Label = "Jane (English, female)" },
    @{ Id = "mary";     Label = "Mary (English, female)" },
    @{ Id = "charles";  Label = "Charles (English, male)" },
    @{ Id = "george";   Label = "George (English, male)" },
    @{ Id = "michael";  Label = "Michael (English, male)" },
    @{ Id = "paul";     Label = "Paul (English, male)" },
    @{ Id = "estelle";  Label = "Estelle (French, female)" },
    @{ Id = "lola";     Label = "Lola (Spanish, female)" },
    @{ Id = "giovanni"; Label = "Giovanni (Italian, male)" },
    @{ Id = "juergen";  Label = "Juergen (German, male)" },
    @{ Id = "rafael";   Label = "Rafael (Portuguese, male)" }
)
$chosenVoice = if ($Voices -and $Voices.Count -gt 0) { $Voices[0] } else { Select-VoiceFromMenu -Title "Pick a default Pocket voice" -Voices $pocketVoices -Default "alba" }

if ($Cuda) {
    Write-Host ""
    if (Add-CudaRuntime -EngineDir $EngineDir -Runtime torch) {
        Test-CudaSynthesis -EngineDir $EngineDir -Wrapper $dstWrapper -Voice $chosenVoice | Out-Null
    }
}

if ($SmokeTest) {
    Write-Host ""
    Write-Host "  [STEP] smoke" -ForegroundColor Yellow
    Write-Host "  Running smoke test (first run downloads the weights)..." -ForegroundColor Yellow
    $testOut = Join-Path $outputDir "test.wav"
    Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from Pocket TTS" --voice $chosenVoice --output "$testOut"
    if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  [ERROR] smoke" -ForegroundColor Red; exit 1 }
    Write-Host "  [DONE] smoke" -ForegroundColor Green
}

$profileJson = @"
{
  "schema_version": 1,
  "id": "pocket-local",
  "name": "Pocket (Local)",
  "engine": "pocket",
  "voice": "$chosenVoice",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": { "engine": "pocket", "cuda": false }
}
"@

Write-EngineManifest -EngineDir $EngineDir -VoicesInstalled ($pocketVoices.Id)

Write-Host ""
Write-Host "  Pocket TTS installed at: $EngineDir" -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
