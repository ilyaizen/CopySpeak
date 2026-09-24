#Requires -Version 5.1
<#
.SYNOPSIS
    Shared helpers for CopySpeak uv-based engine installers.

.PROGRESS-MARKERS
    Installers emit three machine-parseable line prefixes on stdout so the
    CopySpeak frontend can track state transitions through the streamed event
    pipe (everything else is free-form log noise):
        [STEP] <name>   - a phase started (name = 'voice:<id>' for per-voice
                          Piper downloads, or 'engine'/'model' for shared steps)
        [DONE] <name>   - a phase completed
        [ERROR] <name>  - a phase failed (trailing ': <detail>' optional)

.DESCRIPTION
    Dot-source this file from an engine installer:
        . "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

    All Python engines are managed by `uv`. These helpers stay deliberately
    boring: require uv, locate the engine root, create a uv project, run uv,
    validate audio output, and print a CopySpeak profile snippet.
#>

# Add uv's standard install locations to this process's PATH.
#
# Installers launched by the app inherit CopySpeak's PATH, which is a snapshot
# from app launch. Right after install-uv.ps1 runs, uv exists on disk but not
# in that snapshot, so every engine installer would fail until the app is
# restarted. Re-probing the well-known locations avoids that restart.
function Add-UvToPath {
    $candidates = @(
        (Join-Path $env:USERPROFILE ".local\bin"),                        # Astral install script
        (Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Links"),           # winget shim
        (Join-Path $env:LOCALAPPDATA "Programs\uv")
    )
    foreach ($dir in $candidates) {
        if ((Test-Path (Join-Path $dir "uv.exe")) -and ($env:PATH -notlike "*$dir*")) {
            $env:PATH = "$dir;$env:PATH"
            Write-Host "  Added to PATH for this run: $dir" -ForegroundColor Gray
        }
    }
}

# Require uv on PATH. Fails hard (engine installers cannot proceed without it).
function Require-Uv {
    $uv = Get-Command uv -ErrorAction SilentlyContinue
    if (-not $uv) {
        Add-UvToPath
        $uv = Get-Command uv -ErrorAction SilentlyContinue
    }
    if (-not $uv) {
        Write-Host "ERROR: uv is not installed or not on PATH." -ForegroundColor Red
        Write-Host "  Install it first:  ./scripts/install-uv.ps1" -ForegroundColor Yellow
        exit 1
    }
    $version = & uv --version
    Write-Host "  Found uv: $version" -ForegroundColor Green
}

# %LOCALAPPDATA%\CopySpeak\engines  (matches the {engine_dir} placeholder).
function Get-CopySpeakEngineRoot {
    return Join-Path $env:LOCALAPPDATA "CopySpeak\engines"
}

# Create (or reset with -Force) a uv project directory for one engine.
function New-EngineProject {
    param(
        [Parameter(Mandatory)][string]$EngineDir,
        [switch]$Force
    )
    if ((Test-Path $EngineDir) -and $Force) {
        Write-Host "  Removing existing engine dir (-Force): $EngineDir" -ForegroundColor Yellow
        Remove-Item -Recurse -Force $EngineDir
    }
    if (-not (Test-Path $EngineDir)) {
        New-Item -ItemType Directory -Force $EngineDir | Out-Null
        Write-Host "  Created: $EngineDir" -ForegroundColor Gray
    }
    if (-not (Test-Path (Join-Path $EngineDir "pyproject.toml"))) {
        # ponytail: --name avoids a collision when the directory basename equals
        # a PyPI package name (e.g. engines\piper + `uv add piper` -> "self-
        # dependencies are not permitted"). Prefix keeps the project name unique.
        $projectName = "copyspeak-$(Split-Path $EngineDir -Leaf)"
        # ponytail: the target dir is positional. `uv --project X init` was
        # accepted by older uv and is a hard error from 0.12 on
        # ("The `--project` option cannot be used in `uv init`").
        Write-Host "  Running: uv init --bare --name $projectName ($EngineDir)" -ForegroundColor Gray
        & uv init --bare --name $projectName $EngineDir
    }
}

# Run uv, echoing the exact command. Stops on non-zero exit.
function Invoke-Uv {
    param([Parameter(ValueFromRemainingArguments)][string[]]$Args)
    Write-Host "  > uv $($Args -join ' ')" -ForegroundColor DarkGray
    & uv @Args
    if ($LASTEXITCODE -ne 0) {
        throw "uv exited with code $LASTEXITCODE"
    }
}

# Install the GPU inference runtime into one engine's uv project.
#
# Deliberately NOT `pip install --user`: engines live in isolated uv projects,
# and a user-site onnxruntime-gpu would shadow whatever build the project
# resolved, for every engine at once.
#
# The nvidia-* wheels carry the CUDA/cuDNN DLLs. onnxruntime cannot find them on
# its own under Windows, so each wrapper registers their bin directories itself
# (enable_cuda_dlls in scripts/<engine>/copyspeak-<engine>.py).
function Add-CudaRuntime {
    param(
        [Parameter(Mandatory)][string]$EngineDir,
        # onnx: piper / kitten / kokoro.  torch: pocket.
        [ValidateSet("onnx", "torch")][string]$Runtime = "onnx"
    )
    Write-Host "  [STEP] cuda" -ForegroundColor Yellow
    try {
        if ($Runtime -eq "torch") {
            # PyPI torch has been CPU-only since 2.4; CUDA wheels live only on
            # https://download.pytorch.org/whl/*. cu124 caps at 2.6.0 for
            # cp313-win, so an unpinned `uv add --index cu124 torch` resolves
            # to PyPI 2.14.0 CPU (newer) and gives "Torch not compiled with CUDA".
            # Use explicit cu126 index (CUDA 12.6, compatible with driver 616.92/CUDA 13.4)
            # and pin to 2.8.0+cu126 - exists for cp313-win_amd64 and satisfies
            # qwen-tts/pocket-tts - so the CUDA build sticks across later `uv add`.
            $pyproject = Join-Path $EngineDir "pyproject.toml"
            if (Test-Path $pyproject) {
                $content = Get-Content $pyproject -Raw
                if ($content -notmatch 'pytorch-cu126') {
                    Add-Content $pyproject "`n[[tool.uv.index]]`nname = `"pytorch-cu126`"`nurl = `"https://download.pytorch.org/whl/cu126`"`nexplicit = true`n"
                    $content = Get-Content $pyproject -Raw
                }
                if ($content -notmatch '\[tool\.uv\.sources\]') {
                    Add-Content $pyproject "`n[tool.uv.sources]`ntorch = { index = `"pytorch-cu126`" }`ntorchaudio = { index = `"pytorch-cu126`" }`n"
                } else {
                    # torchaudio starts with "torch", so check the torch mapping
                    # before adding the torchaudio line or it always matches.
                    if ($content -notmatch 'torch.*pytorch-cu126') {
                        $content = $content -replace '(\[tool\.uv\.sources\])', "`$1`ntorch = { index = `"pytorch-cu126`" }"
                    }
                    if ($content -notmatch 'torchaudio.*pytorch-cu126') {
                        $content = $content -replace '(\[tool\.uv\.sources\])', "`$1`ntorchaudio = { index = `"pytorch-cu126`" }"
                    }
                    Set-Content $pyproject $content -Encoding utf8
                }
            }
            # torch and torchaudio must come from the same index: qwen_tts/pocket
            # import torchaudio, and an unpinned PyPI torchaudio (2.11) beside
            # cu126 torch (2.8) dies at `import torchaudio` with "[WinError 127]
            # The specified procedure could not be found" - _torchaudio.pyd asks
            # the older torch C++ ABI for symbols it does not export. CPU
            # profiles are unaffected (PyPI torch + PyPI torchaudio match).
            Invoke-Uv add --project $EngineDir "torch==2.8.0" "torchaudio==2.8.0"
        } else {
            # The CPU wheel and the GPU wheel both provide the `onnxruntime`
            # module and write the same files. kokoro-onnx, kittentts and
            # piper-tts each require the CPU wheel transitively, so `uv remove`
            # is a no-op and whichever wheel installs last owns the module.
            # Exclude it from resolution instead.
            $pyproject = Join-Path $EngineDir "pyproject.toml"
            $content = Get-Content $pyproject -Raw
            if ($content -notmatch 'exclude-dependencies') {
                if ($content -match '(?m)^\[tool\.uv\]\s*$') {
                    $content = $content -replace '(?m)^(\[tool\.uv\])\s*$', "`$1`nexclude-dependencies = [`"onnxruntime`"]"
                    Set-Content $pyproject $content -Encoding utf8
                } else {
                    Add-Content $pyproject "`n[tool.uv]`nexclude-dependencies = [`"onnxruntime`"]`n"
                }
            }
            # Current onnxruntime-gpu builds target CUDA 13 (the provider imports
            # cublas64_13/cudart64_13); paired with the old cu12 wheels, session
            # creation dies with "Invalid handle. Cannot load symbol cudnnCreate".
            # nvidia-cuda-runtime-cu13 is a deprecated 0.0.1 placeholder whose
            # build fails on purpose - the CUDA 13 runtime publishes under
            # nvidia-cuda-runtime. cudnn-cu13 pulls cublas/nvrtc along; the
            # provider's FFT ops still import cufft64_12, so that one stays cu12.
            # Pin the exact set onnxruntime-gpu 1.30.0 ships against: newer
            # cudnn-cu13 (9.26) + cublas (13.8) each load via ctypes, but the
            # provider bridge fails on cublasLt64_13.dll (Error 126) and ORT
            # silently runs on CPU.
            Invoke-Uv add --project $EngineDir "onnxruntime-gpu==1.30.0" `
                "nvidia-cuda-runtime==13.0.96" "nvidia-cuda-nvrtc==13.0.88" `
                "nvidia-cudnn-cu13==9.24.0.43" "nvidia-cublas==13.1.1.3" `
                "nvidia-cufft-cu12==11.4.1.4"
            # Uninstalling the excluded CPU wheel deletes files it shared with
            # onnxruntime-gpu; reinstall the GPU wheel over the hole.
            Invoke-Uv sync --project $EngineDir --reinstall-package onnxruntime-gpu
        }
    } catch {
        Write-Host "  [ERROR] cuda ($_)" -ForegroundColor Red
        return $false
    }
    Write-Host "  [DONE] cuda" -ForegroundColor Green
    return $true
}

# Prove the GPU actually works by synthesizing with --device cuda.
#
# `ort.get_available_providers()` lists CUDAExecutionProvider even when the DLLs
# are missing, so it can only ever produce a false green. Real audio out of a
# real CUDA session is the only honest check.
function Test-CudaSynthesis {
    param(
        [Parameter(Mandatory)][string]$EngineDir,
        [Parameter(Mandatory)][string]$Wrapper,
        [Parameter(Mandatory)][string]$Voice
    )
    Write-Host "  [STEP] cuda-check" -ForegroundColor Yellow
    $out = Join-Path $EngineDir "output/cuda-check.wav"
    New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
    Remove-Item -Force -ErrorAction SilentlyContinue $out
    try {
        Invoke-Uv run --project $EngineDir python "$Wrapper" `
            --text "GPU check" --voice $Voice --output "$out" --device cuda
    } catch {
        Write-Host "  [ERROR] cuda-check ($_)" -ForegroundColor Red
        Write-Host "  GPU synthesis failed; the engine still works on CPU." -ForegroundColor Yellow
        return $false
    }
    if (-not (Test-AudioFile -Path $out)) {
        Write-Host "  [ERROR] cuda-check (no audio produced)" -ForegroundColor Red
        return $false
    }
    Write-Host "  [DONE] cuda-check" -ForegroundColor Green
    Write-Host "  Enable 'GPU acceleration' in this engine's settings to use it." -ForegroundColor Gray
    return $true
}

# Validate that a synthesis smoke test produced a non-empty WAV.
function Test-AudioFile {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path $Path)) {
        Write-Host "  ERROR: expected audio file not found: $Path" -ForegroundColor Red
        return $false
    }
    $size = (Get-Item $Path).Length
    if ($size -le 44) {
        Write-Host "  ERROR: audio file is empty/too small ($size bytes): $Path" -ForegroundColor Red
        return $false
    }
    Write-Host "  OK: audio file $Path ($size bytes)" -ForegroundColor Green
    return $true
}

# Write %LOCALAPPDATA%\CopySpeak\engines\<engine>\manifest.json recording which
# voices are installed. The frontend reads it to pre-check the "add a voice
# later" dialog. Call once at the end of a successful install / voice re-run;
# the caller is the source of truth for the voice list (Piper: present .onnx
# basenames; Kitten/Kokoro: all catalog voices since the model is shared).
function Write-EngineManifest {
    param(
        [Parameter(Mandatory)][string]$EngineDir,
        # A failed/skipped download can leave this empty; let the caller's
        # [ERROR] reporting run instead of a parameter-binding crash.
        [Parameter(Mandatory)][AllowEmptyCollection()][string[]]$VoicesInstalled,
        [string]$Version = "1.0"
    )
    $manifest = [ordered]@{
        version          = $Version
        voices_installed = @($VoicesInstalled)
        installed_at     = (Get-Date).ToString("o")
    }
    $manifestPath = Join-Path $EngineDir "manifest.json"
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -Path $manifestPath -Encoding utf8
    Write-Host "  [DONE] manifest ($($manifest.voices_installed.Count) voices)" -ForegroundColor Green
}

# -- CLI chrome (shared banner + permission prompt) --------------------------

# True when the app (not a human) is driving: install.rs spawns installers with
# stdin closed, so any Read-Host would fail or block forever. Every interactive
# helper below silently takes its default in that mode.
function Test-NonInteractive {
    return ($env:COPYSPEAK_NONINTERACTIVE -eq "1")
}

function Write-EngineBanner {
    param([Parameter(Mandatory)][string]$Title)
    $bar = "=" * 40
    Write-Host "" -ForegroundColor Magenta
    Write-Host "  $bar" -ForegroundColor Magenta
    Write-Host "  |  $Title$(" " * [Math]::Max(1, 37 - $Title.Length))|" -ForegroundColor Magenta
    Write-Host "  $bar" -ForegroundColor Magenta
    Write-Host "" -ForegroundColor Magenta
}

# Ask the user before doing something destructive or network-heavy.
# Defaults to No (so a blind Enter keeps the safe path). Returns $true for yes.
function Get-Confirmation {
    param(
        [Parameter(Mandatory)][string]$Prompt,
        [switch]$DefaultYes
    )
    $default = if ($DefaultYes) { "Y" } else { "N" }
    if (Test-NonInteractive) { return [bool]$DefaultYes }
    Write-Host "  $Prompt" -ForegroundColor Yellow
    $answer = Read-Host "  Proceed? (y/N) [$default]"
    if ([string]::IsNullOrWhiteSpace($answer)) { $answer = $default }
    return $answer.Trim().ToUpperInvariant() -eq "Y"
}

# Print a numbered menu of voices and return the chosen Id.
#
# The app runs installers via `powershell -File`, which binds `-Voices a,b` as
# ONE string "a,b" (and silently drops space-separated extra ids), so the app
# sends one comma-joined argument and each installer splits it back here.
# Always returns an array, even for a single id.
function ConvertTo-VoiceIds {
    param([string[]]$Voices)
    return ,@($Voices -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

# Voices is an array of hashtables: @{ Id = "..."; Label = "..." }.
# Defaults to the entry marked Default (or the first) on a blank Enter.
function Select-VoiceFromMenu {
    param(
        [Parameter(Mandatory)][string]$Title,
        [Parameter(Mandatory)]$Voices,
        [string]$Default
    )
    if ($Voices.Count -eq 1) { return $Voices[0].Id }

    $defaultId = if ($Default) { $Default } else { $Voices[0].Id }
    if (Test-NonInteractive) { return $defaultId }
    Write-Host ""
    Write-Host "  $Title" -ForegroundColor Cyan
    for ($i = 0; $i -lt $Voices.Count; $i++) {
        $v = $Voices[$i]
        $marker = if ($v.Id -eq $defaultId) { " (default)" } else { "" }
        Write-Host ("    {0,2}. {1} - {2}{3}" -f ($i + 1), $v.Id, $v.Label, $marker) -ForegroundColor Gray
    }
    do {
        $choice = Read-Host "  Pick a voice [1-$($Voices.Count)]"
        if ([string]::IsNullOrWhiteSpace($choice)) { return $defaultId }
        $idx = 0
        if ([int]::TryParse($choice.Trim(), [ref]$idx) -and $idx -ge 1 -and $idx -le $Voices.Count) {
            return $Voices[$idx - 1].Id
        }
        Write-Host "  Invalid choice; try again." -ForegroundColor Red
    } while ($true)
}

# Print a ready-to-paste CopySpeak profile snippet. Does NOT edit config (v1).
function Write-ProfileSnippet {
    param([Parameter(Mandatory)][string]$Json)
    Write-Host ""
    Write-Host "  Paste this into a CopySpeak profile (Engine settings -> Import):" -ForegroundColor Cyan
    Write-Host "  ----------------------------------------------------------------" -ForegroundColor DarkGray
    $Json -split "`n" | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
    Write-Host "  ----------------------------------------------------------------" -ForegroundColor DarkGray
    Write-Host ""
}
