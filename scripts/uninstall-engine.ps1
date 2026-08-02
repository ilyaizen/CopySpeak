#Requires -Version 5.1
<#
.SYNOPSIS
    Uninstalls a CopySpeak local TTS engine.

.DESCRIPTION
    The mirror of the install-*.ps1 scripts. Every local engine leaves at most
    two traces, so one script covers all of them:

      * a uv-managed project dir under %LOCALAPPDATA%\CopySpeak\engines\<name>
        (piper, kitten) or a model dir (kokoro)
      * a `uv tool` install putting a binary on PATH (edge-tts, kokoro-tts,
        pocket-tts)

    Emits the same [STEP]/[DONE]/[ERROR] markers as the installers so the app
    can stream progress through the shared install-progress event pipe.

    uv itself is deliberately NOT uninstallable from here: it is a shared
    prerequisite that other engines (and possibly other apps) depend on.

.PARAMETER Engine
    kitten | piper | kokoro | pocket | edge

.PARAMETER KeepModels
    Uninstall the package/binary but leave downloaded model files on disk
    (useful when reinstalling on a metered connection).

.EXAMPLE
    ./scripts/uninstall-engine.ps1 -Engine piper
    ./scripts/uninstall-engine.ps1 -Engine kokoro -KeepModels
#>

param(
    [Parameter(Mandatory)]
    [ValidateSet("kitten", "piper", "kokoro", "pocket", "edge")]
    [string]$Engine,
    [switch]$KeepModels
)

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

# What each engine leaves behind. Dir is relative to the CopySpeak engine root;
# Tool is the `uv tool` name (engines installed as project deps have none).
$layout = @{
    kitten = @{ Dir = "kitten"; Tool = $null;          Title = "Kitten TTS" }
    piper  = @{ Dir = "piper";  Tool = $null;          Title = "Piper TTS" }
    kokoro = @{ Dir = "kokoro"; Tool = "kokoro-tts";   Title = "Kokoro TTS" }
    pocket = @{ Dir = $null;    Tool = "pocket-tts";   Title = "Pocket TTS" }
    edge   = @{ Dir = $null;    Tool = "edge-tts";     Title = "Edge-TTS" }
}[$Engine]

Write-EngineBanner -Title "$($layout.Title) Uninstaller"

$failed = $false

# -- uv tool -----------------------------------------------------------------
if ($layout.Tool) {
    Write-Host "  [STEP] tool" -ForegroundColor Yellow
    if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
        # No uv means nothing was installed through it; not an error.
        Write-Host "  uv not on PATH; nothing to uninstall." -ForegroundColor Gray
        Write-Host "  [DONE] tool" -ForegroundColor Green
    } else {
        Write-Host "  > uv tool uninstall $($layout.Tool)" -ForegroundColor DarkGray
        # uv exits non-zero when the tool was never installed - that is a
        # success for us, so inspect the exit code instead of throwing.
        & uv tool uninstall $layout.Tool 2>&1 | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  [DONE] tool" -ForegroundColor Green
        } elseif (-not (Get-Command $layout.Tool -ErrorAction SilentlyContinue)) {
            Write-Host "  $($layout.Tool) is not installed." -ForegroundColor Gray
            Write-Host "  [DONE] tool" -ForegroundColor Green
        } else {
            Write-Host "  [ERROR] tool: uv tool uninstall exited $LASTEXITCODE" -ForegroundColor Red
            $failed = $true
        }
    }
}

# -- engine directory --------------------------------------------------------
if ($layout.Dir) {
    $engineDir = Join-Path (Get-CopySpeakEngineRoot) $layout.Dir
    Write-Host "  [STEP] files" -ForegroundColor Yellow
    if (-not (Test-Path $engineDir)) {
        Write-Host "  Nothing at $engineDir." -ForegroundColor Gray
        Write-Host "  [DONE] files" -ForegroundColor Green
    } elseif ($KeepModels) {
        # Drop the venv/wrapper/manifest but keep the expensive downloads.
        Write-Host "  Keeping model files; removing project files in $engineDir" -ForegroundColor Gray
        foreach ($leaf in @(".venv", "scripts", "output", "pyproject.toml", "uv.lock", "manifest.json")) {
            $p = Join-Path $engineDir $leaf
            if (Test-Path $p) { Remove-Item -Recurse -Force $p }
        }
        Write-Host "  [DONE] files" -ForegroundColor Green
    } else {
        Write-Host "  Removing $engineDir" -ForegroundColor Gray
        try {
            Remove-Item -Recurse -Force $engineDir
            Write-Host "  [DONE] files" -ForegroundColor Green
        } catch {
            # Almost always a running python.exe holding the venv open.
            Write-Host "  [ERROR] files: $_" -ForegroundColor Red
            Write-Host "  Close anything using the engine (a playing clip) and retry." -ForegroundColor Yellow
            $failed = $true
        }
    }
}

Write-Host ""
if ($failed) {
    Write-Host "  $($layout.Title) uninstall incomplete." -ForegroundColor Red
    exit 1
}
Write-Host "  $($layout.Title) uninstalled." -ForegroundColor Green
Write-Host "  Remove any CopySpeak profile that pointed at it from the Voices page." -ForegroundColor Gray
