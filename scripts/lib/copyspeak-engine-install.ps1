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
        Write-Host "  Running: uv init --bare --name $projectName ($EngineDir)" -ForegroundColor Gray
        & uv --project $EngineDir init --bare --name $projectName
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
        [Parameter(Mandatory)][string[]]$VoicesInstalled,
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
