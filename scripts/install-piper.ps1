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

.PARAMETER Cuda
    Also install onnxruntime-gpu and the NVIDIA CUDA/cuDNN runtime wheels into
    the engine project, then verify with a real GPU synthesis.

.PARAMETER Force
    Recreate the engine project from scratch. When omitted, the installer
    still prompts interactively ("Reinstall from scratch?") - answering yes
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
    # Install the GPU runtime (onnxruntime-gpu + the nvidia-* CUDA/cuDNN wheels)
    # into this engine's uv project and verify it with a real CUDA synthesis.
    [switch]$Cuda,
    [switch]$SmokeTest,
    [switch]$SkipVoiceDownload,
    # App-driven voice selection: bypasses the interactive menu. The first id
    # is the profile-snippet default; all ids are downloaded. Manual runs omit
    # -Voices and get the numbered menu instead.
    [string[]]$Voices
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

Write-EngineBanner -Title "Piper TTS Installer"

Require-Uv

# Interactive force prompt: -Force bypasses; a blank Enter keeps the install.
# -Voices (app-driven) is non-interactive: never destructively reinstall.
$effectiveForce = if ($Force) {
    $true
} elseif ($Voices) {
    $false
} else {
    Get-Confirmation -Prompt "Reinstall Piper from scratch? (deletes the existing engine dir)" -DefaultYes:$false
}

$EngineDir = Join-Path (Get-CopySpeakEngineRoot) "piper"
New-EngineProject -EngineDir $EngineDir -Force:$effectiveForce

Write-Host ""
Write-Host "  Installing Piper..." -ForegroundColor Gray
# ponytail: PyPI `piper` is an unrelated bioinformatics toolkit (databio/pypiper,
# module `pypiper`). The TTS engine ships as `piper-tts` (module `piper`).
Invoke-Uv add --project $EngineDir "piper-tts[alignment]==1.8.0"

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

# Resolve the set of voices to install. -Voices (app-driven) bypasses the
# interactive menu; otherwise the manual menu picks one. The first id is the
# profile-snippet default.
$existingModel = Get-ChildItem -Path $voicesDir -Filter "*.onnx" -ErrorAction SilentlyContinue | Select-Object -First 1
$wantedVoices = if ($Voices -and $Voices.Count -gt 0) {
    @($Voices)
} elseif ($SkipVoiceDownload -and $existingModel) {
    @([IO.Path]::GetFileNameWithoutExtension($existingModel.Name))
} else {
    @(Select-VoiceFromMenu -Title "Pick an English Piper voice" -Voices $piperVoices -Default "en_US-amy-medium")
}
$chosenVoice = $wantedVoices[0]

# Download each wanted model pair if missing. Per-voice [STEP]/[DONE]/[ERROR]
# markers let the frontend track per-voice status through the event stream.
# A failed voice must fail the whole run, or the app reports a green install
# for an engine that cannot synthesize.
$voiceFailures = 0
foreach ($v in $wantedVoices) {
    $modelPath = Join-Path $voicesDir "$v.onnx"
    $configPath = Join-Path $voicesDir "$v.onnx.json"
    if ((Test-Path $modelPath) -and (Test-Path $configPath)) {
        Write-Host "  [DONE] voice:$v (already present)" -ForegroundColor Green
        continue
    }
    if ($SkipVoiceDownload) {
        Write-Host "  [ERROR] voice:$v (skipped, -SkipVoiceDownload)" -ForegroundColor Red
        $voiceFailures++
        continue
    }
    Write-Host "  [STEP] voice:$v" -ForegroundColor Yellow
    $parts = $v -split "-"
    if ($parts.Count -ge 3) {
        $voiceName = $parts[1]
        $quality = $parts[2]
        $onnxUrl = "$voiceBaseUrl/$voiceName/$quality/$v.onnx"
        $jsonUrl = "$voiceBaseUrl/$voiceName/$quality/$v.onnx.json"
        Write-Host "    -> $modelPath" -ForegroundColor Gray
        try {
            if (-not (Test-Path $modelPath)) {
                Invoke-WebRequest -Uri $onnxUrl -OutFile $modelPath -UseBasicParsing
            }
            if (-not (Test-Path $configPath)) {
                Invoke-WebRequest -Uri $jsonUrl -OutFile $configPath -UseBasicParsing
            }
            Write-Host "  [DONE] voice:$v" -ForegroundColor Green
        } catch {
            # A half-written .onnx would look "present" on the next run and
            # then fail at synthesis time, so clear the partial download.
            Remove-Item -Force -ErrorAction SilentlyContinue $modelPath, $configPath
            Write-Host "  [ERROR] voice:$v : $_" -ForegroundColor Red
            $voiceFailures++
        }
    } else {
        Write-Host "  [ERROR] voice:$v (unrecognized id shape)" -ForegroundColor Red
        $voiceFailures++
    }
}

Write-Host ""
Write-Host "  Voices directory: $voicesDir" -ForegroundColor Gray
Write-Host "  More voices:      https://github.com/OHF-Voice/piper1-gpl#voices" -ForegroundColor Gray

if ($Cuda) {
    Write-Host ""
    if (Add-CudaRuntime -EngineDir $EngineDir) {
        Test-CudaSynthesis -EngineDir $EngineDir -Wrapper $dstWrapper -Voice $chosenVoice | Out-Null
    }
}

if ($SmokeTest) {
    $smokeModel = Join-Path $voicesDir "$chosenVoice.onnx"
    if (-not (Test-Path $smokeModel)) {
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

# Record installed voices (source of truth = present .onnx basenames) so the
# frontend can pre-check the "add a voice later" dialog.
$installedVoices = @(Get-ChildItem -Path $voicesDir -Filter "*.onnx" -ErrorAction SilentlyContinue | ForEach-Object { [IO.Path]::GetFileNameWithoutExtension($_.Name) })
Write-EngineManifest -EngineDir $EngineDir -VoicesInstalled $installedVoices

Write-Host ""
if ($voiceFailures -gt 0) {
    Write-Host "  [ERROR] engine ($voiceFailures voice download(s) failed)" -ForegroundColor Red
    Write-Host "  Piper package is installed at $EngineDir, but retry the failed voices." -ForegroundColor Yellow
    exit 1
}
Write-Host "  [DONE] engine" -ForegroundColor Green
Write-Host "  Piper installed at: $EngineDir" -ForegroundColor Green
Write-ProfileSnippet -Json $profileJson
