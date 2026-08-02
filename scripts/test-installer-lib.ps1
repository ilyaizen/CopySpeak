#Requires -Version 5.1
<#
.SYNOPSIS
    Self-check for the shared installer helpers. No network, no real installs.

.DESCRIPTION
    Covers the two things that silently broke the app-driven install path:

      1. Non-interactive mode. The app spawns installers with stdin closed, so
         any Read-Host blocks forever. Every interactive helper must return its
         default when COPYSPEAK_NONINTERACTIVE=1. Run this with stdin closed
         (`< NUL`) and a hang IS the failure.
      2. uninstall-engine.ps1 on a machine where the engine was never installed
         must be a clean no-op that exits 0. Pointed at a throwaway
         LOCALAPPDATA so it can never touch a real install.

    Also parses every script under scripts/ so a stray non-ASCII character
    (which Windows PowerShell 5.1 reads as ANSI, breaking string literals)
    cannot ship again.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-installer-lib.ps1 < NUL
#>

$ErrorActionPreference = "Stop"

. "$PSScriptRoot/lib/copyspeak-engine-install.ps1"

$script:failed = 0
function Assert-That {
    param([Parameter(Mandatory)][bool]$Condition, [Parameter(Mandatory)][string]$Name)
    if ($Condition) {
        Write-Host "  ok   $Name" -ForegroundColor Green
    } else {
        Write-Host "  FAIL $Name" -ForegroundColor Red
        $script:failed++
    }
}

Write-Host "non-interactive helpers" -ForegroundColor Cyan
$env:COPYSPEAK_NONINTERACTIVE = "1"
try {
    Assert-That (Test-NonInteractive) "Test-NonInteractive is true when the env var is set"
    Assert-That ((Get-Confirmation -Prompt "x") -eq $false) "Get-Confirmation defaults to No"
    Assert-That ((Get-Confirmation -Prompt "x" -DefaultYes) -eq $true) "Get-Confirmation honours -DefaultYes"

    $voices = @(@{ Id = "a"; Label = "A" }, @{ Id = "b"; Label = "B" })
    Assert-That ((Select-VoiceFromMenu -Title "t" -Voices $voices -Default "b") -eq "b") "Select-VoiceFromMenu takes -Default"
    Assert-That ((Select-VoiceFromMenu -Title "t" -Voices $voices) -eq "a") "Select-VoiceFromMenu falls back to the first voice"
} finally {
    Remove-Item Env:\COPYSPEAK_NONINTERACTIVE -ErrorAction SilentlyContinue
}
Assert-That (-not (Test-NonInteractive)) "Test-NonInteractive is false when the env var is unset"

Write-Host "script parsing (ANSI-safe)" -ForegroundColor Cyan
foreach ($file in Get-ChildItem -Path $PSScriptRoot -Recurse -Filter *.ps1) {
    $errors = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$null, [ref]$errors)
    Assert-That ($errors.Count -eq 0) "parses: $($file.Name)"
    $nonAscii = ([char[]](Get-Content -Raw $file.FullName) | Where-Object { [int]$_ -gt 127 }).Count
    Assert-That ($nonAscii -eq 0) "ASCII-only: $($file.Name)"
}

Write-Host "uninstall-engine.ps1 no-op" -ForegroundColor Cyan
# A throwaway LOCALAPPDATA guarantees Get-CopySpeakEngineRoot points at an
# empty tree, so the dir branch takes its "nothing to remove" path. kitten has
# no uv tool, so nothing outside this sandbox is touched.
$sandbox = Join-Path ([IO.Path]::GetTempPath()) "copyspeak-uninstall-test-$PID"
New-Item -ItemType Directory -Force $sandbox | Out-Null
try {
    $out = & powershell -NoProfile -ExecutionPolicy Bypass -Command "
        `$env:LOCALAPPDATA = '$sandbox'
        `$env:COPYSPEAK_NONINTERACTIVE = '1'
        & '$PSScriptRoot/uninstall-engine.ps1' -Engine kitten
        exit `$LASTEXITCODE" 2>&1
    $code = $LASTEXITCODE
    Assert-That ($code -eq 0) "exits 0 when the engine was never installed"
    Assert-That ([bool]($out -match "\[DONE\] files")) "emits the [DONE] files marker"
    Assert-That (-not ($out -match "\[ERROR\]")) "emits no [ERROR] marker"
} finally {
    Remove-Item -Recurse -Force $sandbox -ErrorAction SilentlyContinue
}

Write-Host ""
if ($script:failed -gt 0) {
    Write-Host "$script:failed check(s) FAILED" -ForegroundColor Red
    exit 1
}
Write-Host "all checks passed" -ForegroundColor Green
