#Requires -Version 5.1
<#
.SYNOPSIS
    Installs uv (the Python package/environment manager) for CopySpeak local engines.

.DESCRIPTION
    uv is a hard requirement for all CopySpeak Python-based TTS engines. This is
    the ONLY installer allowed to run when uv is missing; every other engine
    installer fails fast and points here.

    Tries winget first, then falls back to the official Astral install script.

.PARAMETER Force
    Reinstall even if uv is already present.

.EXAMPLE
    ./scripts/install-uv.ps1
#>

param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host "  |  uv Installer for CopySpeak          |" -ForegroundColor Magenta
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host ""

$existing = Get-Command uv -ErrorAction SilentlyContinue
if ($existing -and -not $Force) {
    $version = & uv --version
    Write-Host "  uv already installed: $version" -ForegroundColor Green
    Write-Host "  Use -Force to reinstall." -ForegroundColor Yellow
    exit 0
}

$winget = Get-Command winget -ErrorAction SilentlyContinue
if ($winget) {
    Write-Host "  Installing uv via winget..." -ForegroundColor Gray
    Write-Host "  > winget install astral-sh.uv --accept-package-agreements --accept-source-agreements" -ForegroundColor DarkGray
    $prevEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    winget install astral-sh.uv --accept-package-agreements --accept-source-agreements --silent 2>&1 | Out-Null
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prevEAP
    if ($code -eq 0 -or $code -eq -1978335189) {
        Write-Host "  uv installed via winget." -ForegroundColor Green
        Write-Host "  Restart your terminal so uv is on PATH, then run an engine installer." -ForegroundColor Yellow
        exit 0
    }
    Write-Host "  winget install failed (code $code); falling back to official script." -ForegroundColor Yellow
}

Write-Host "  Installing uv via official Astral script..." -ForegroundColor Gray
Write-Host "  > irm https://astral.sh/uv/install.ps1 | iex" -ForegroundColor DarkGray
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"

$check = Get-Command uv -ErrorAction SilentlyContinue
if ($check) {
    Write-Host "  uv installed: $(& uv --version)" -ForegroundColor Green
} else {
    Write-Host "  uv installed. Restart your terminal so it appears on PATH." -ForegroundColor Yellow
}
