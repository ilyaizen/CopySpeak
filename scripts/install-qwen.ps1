#Requires -Version 5.1
<#
.SYNOPSIS
    Installs Qwen3-TTS CustomVoice for CopySpeak via uv.
#>

param(
    [switch]$Force,
    [switch]$Cuda,
    [switch]$SmokeTest,
    [string[]]$Voices
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Qwen3-TTS Installer"
Require-Uv

$effectiveForce = if ($Force) {
    $true
} elseif ($Voices) {
    $false
} else {
    Get-Confirmation -Prompt "Reinstall Qwen3-TTS from scratch? (deletes the existing engine dir)" -DefaultYes:$false
}

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "qwen"
New-EngineProject -EngineDir $EngineDir -Force:$effectiveForce

Write-Host ""
Write-Host "  [STEP] engine" -ForegroundColor Yellow
Write-Host "  Installing qwen-tts + soundfile..." -ForegroundColor Gray
Invoke-Uv add --project $EngineDir "qwen-tts" "soundfile"

$scriptsDir = Join-Path $EngineDir "scripts"
$outputDir = Join-Path $EngineDir "output"
New-Item -ItemType Directory -Force $scriptsDir | Out-Null
New-Item -ItemType Directory -Force $outputDir | Out-Null
$srcWrapper = Join-Path $PSScriptRoot "qwen/copyspeak-qwen.py"
$dstWrapper = Join-Path $scriptsDir "copyspeak-qwen.py"
Copy-Item $srcWrapper $dstWrapper -Force
Write-Host "  Wrapper installed: $dstWrapper" -ForegroundColor Gray
Write-Host "  [DONE] engine" -ForegroundColor Green

$qwenVoices = @(
    @{ Id = "Aiden";    Label = "Aiden (English male)" },
    @{ Id = "Ryan";     Label = "Ryan (English male)" },
    @{ Id = "Vivian";   Label = "Vivian (Chinese female)" },
    @{ Id = "Serena";   Label = "Serena (Chinese female)" },
    @{ Id = "Uncle_Fu"; Label = "Uncle Fu (Chinese male)" },
    @{ Id = "Dylan";    Label = "Dylan (Beijing male)" },
    @{ Id = "Eric";     Label = "Eric (Chengdu male)" },
    @{ Id = "Ono_Anna"; Label = "Ono Anna (Japanese female)" },
    @{ Id = "Sohee";    Label = "Sohee (Korean female)" }
)
$chosenVoice = if ($Voices -and $Voices.Count -gt 0) { $Voices[0] } else { Select-VoiceFromMenu -Title "Pick a default Qwen3-TTS voice" -Voices $qwenVoices -Default "Aiden" }

if ($Cuda) {
    Write-Host ""
    if (Add-CudaRuntime -EngineDir $EngineDir -Runtime torch) {
        Test-CudaSynthesis -EngineDir $EngineDir -Wrapper $dstWrapper -Voice $chosenVoice | Out-Null
    }
}

if ($SmokeTest) {
    Write-Host ""
    Write-Host "  Running smoke test (first run downloads the model)..." -ForegroundColor Yellow
    $testOut = Join-Path $outputDir "test.wav"
    Invoke-Uv run --project $EngineDir python "$dstWrapper" --text "Hello from Qwen3 TTS" --voice $chosenVoice --output "$testOut"
    if (-not (Test-AudioFile -Path $testOut)) { Write-Host "  Smoke test FAILED." -ForegroundColor Red; exit 1 }
}

Write-EngineManifest -EngineDir $EngineDir -VoicesInstalled ($qwenVoices.Id)
Write-Host ""
Write-Host "  Qwen3-TTS installed at: $EngineDir" -ForegroundColor Green
