#Requires -Version 5.1
<#
.SYNOPSIS
    Installs KittenTTS for CopySpeak text-to-speech.

.DESCRIPTION
    This script installs KittenTTS - an ultra-lightweight TTS engine (25-80MB)
    that runs on CPU without requiring a GPU. It downloads the wheel from GitHub
    releases, installs dependencies, and sets up the CLI wrapper.

.PARAMETER Model
    Which model variant to download: nano (15M, 25MB), micro (40M, 41MB), or mini (80M, 80MB).
    Default: nano (smallest and fastest)

.PARAMETER PythonVersion
    Python version to use. Default: 3.12

.PARAMETER Force
    Reinstall even if already installed.

.EXAMPLE
    ./install-kittentts.ps1
    Installs KittenTTS with the nano model (default)

.EXAMPLE
    ./install-kittentts.ps1 -Model micro
    Installs KittenTTS with the micro model (balanced)

.EXAMPLE
    ./install-kittentts.ps1 -Model mini
    Installs KittenTTS with the mini model (highest quality)

.NOTES
    Requires Python 3.8+ and pip.
#>

param(
    [ValidateSet("nano", "micro", "mini")]
    [string]$Model = "nano",
    
    [string]$PythonVersion = "3.12",
    
    [switch]$Force,
    
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"

$KittenTTSVersion = "0.8.1"
$WheelUrl = "https://github.com/KittenML/KittenTTS/releases/download/$KittenTTSVersion/kittentts-$KittenTTSVersion-py3-none-any.whl"
$InstallDir = Join-Path $env:USERPROFILE "kittentts"
$CliScript = Join-Path $PSScriptRoot "kittentts-cli.py"
$UseLauncher = $false
$PythonCmd = ""
$DetectedPythonVersion = ""

function Invoke-Python {
    param([string[]]$Arguments)
    if ($UseLauncher) {
        & py "-$PythonVersion" @Arguments
    }
    else {
        & $PythonCmd @Arguments
    }
}

function Get-AvailablePython {
    $supportedVersions = @("3.12", "3.11", "3.10", "3.9", "3.8")
    
    $launcher = Get-Command py -ErrorAction SilentlyContinue
    if ($launcher) {
        foreach ($v in $supportedVersions) {
            $prevEAP = $ErrorActionPreference
            $ErrorActionPreference = "SilentlyContinue"
            $result = & py "-$v" --version 2>&1
            $ErrorActionPreference = $prevEAP
            
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  Found Python via launcher: $v" -ForegroundColor Green
                return "py", "-$v", $v
            }
        }
        
        $prevEAP = $ErrorActionPreference
        $ErrorActionPreference = "SilentlyContinue"
        $unsupportedResult = & py --version 2>&1
        $ErrorActionPreference = $prevEAP
        
        if ($LASTEXITCODE -eq 0 -and $unsupportedResult -match "Python (\d+)\.(\d+)") {
            $major = [int]$Matches[1]
            $minor = [int]$Matches[2]
            $fullVersion = "$major.$minor"
            
            if ($major -gt 3 -or ($major -eq 3 -and $minor -gt 12)) {
                return "py", "", $fullVersion, $true
            }
        }
    }
    
    $pythonCmd = Get-Command python -ErrorAction SilentlyContinue
    if ($pythonCmd) {
        $prevEAP = $ErrorActionPreference
        $ErrorActionPreference = "SilentlyContinue"
        $result = & python --version 2>&1
        $ErrorActionPreference = $prevEAP
        
        if ($LASTEXITCODE -eq 0 -and $result -match "Python (\d+)\.(\d+)") {
            $major = [int]$Matches[1]
            $minor = [int]$Matches[2]
            $fullVersion = "$major.$minor"
            
            if ($major -gt 3 -or ($major -eq 3 -and $minor -gt 12)) {
                return "python", "", $fullVersion, $true
            }
            
            if ($major -eq 3 -and $minor -ge 8 -and $minor -le 12) {
                Write-Host "  Found Python: $($result.Trim())" -ForegroundColor Green
                return "python", "", $fullVersion, $false
            }
        }
    }
    
    return $null, $null, "", $false
}

function Install-PythonViaWinget {
    Write-Host ""
    Write-Host "  Python not found on your system." -ForegroundColor Yellow
    Write-Host "  KittenTTS requires Python 3.8 or higher." -ForegroundColor Yellow
    Write-Host ""
    $response = Read-Host "  Install Python 3.12 via winget? (Y/n)"
    
    if ($response -eq "" -or $response -eq "Y" -or $response -eq "y") {
        Write-Host "  Installing Python 3.12 via winget..." -ForegroundColor Gray
        
        $prevEAP = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        winget install Python.Python.3.12 --accept-package-agreements --accept-source-agreements --silent 2>&1 | Out-Null
        $wingetExitCode = $LASTEXITCODE
        $ErrorActionPreference = $prevEAP
        
        if ($wingetExitCode -eq 0 -or $wingetExitCode -eq -1978335189) {
            Write-Host "  Python 3.12 installed successfully!" -ForegroundColor Green
            Write-Host "  You may need to restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
            return $true
        } else {
            Write-Host "  Failed to install Python via winget." -ForegroundColor Red
            Write-Host "  Error code: $wingetExitCode" -ForegroundColor Red
        }
    }
    
    Write-Host ""
    Write-Host "  Please install Python manually from:" -ForegroundColor Yellow
    Write-Host "  https://www.python.org/downloads/" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Make sure to check 'Add Python to PATH' during installation." -ForegroundColor Yellow
    Write-Host ""
    return $false
}

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "  $Message" -ForegroundColor Cyan
    Write-Host "  $('-' * 50)" -ForegroundColor DarkGray
}

function Test-Python {
    try {
        $cmd, $versionArg, $detectedVersion, $isUnsupported = Get-AvailablePython
        
        if ($null -eq $cmd) {
            Write-Host "ERROR: Python not found on your system." -ForegroundColor Red
            Write-Host ""
            $installed = Install-PythonViaWinget
            if (-not $installed) {
                exit 1
            }
            
            $cmd, $versionArg, $detectedVersion, $isUnsupported = Get-AvailablePython
            if ($null -eq $cmd) {
                Write-Host "ERROR: Python still not found after installation." -ForegroundColor Red
                Write-Host "Please restart your terminal and run the installer again." -ForegroundColor Yellow
                exit 1
            }
        }
        
        if ($isUnsupported) {
            Write-Host ""
            Write-Host "  ========================================" -ForegroundColor Red
            Write-Host "  ERROR: Unsupported Python version" -ForegroundColor Red
            Write-Host "  ========================================" -ForegroundColor Red
            Write-Host ""
            Write-Host "  Detected: Python $detectedVersion" -ForegroundColor Yellow
            Write-Host "  Required: Python 3.8, 3.9, 3.10, 3.11, or 3.12" -ForegroundColor Yellow
            Write-Host ""
            Write-Host "  Python 3.13+ has compatibility issues with KittenTTS dependencies." -ForegroundColor Gray
            Write-Host ""
            Write-Host "  To fix this:" -ForegroundColor Cyan
            Write-Host "  1. Install Python 3.12 from: https://www.python.org/downloads/release/python-3120/" -ForegroundColor Gray
            Write-Host "  2. Or run: winget install Python.Python.3.12" -ForegroundColor Gray
            Write-Host "  3. Re-run this installer" -ForegroundColor Gray
            Write-Host ""
            exit 1
        }
        
        $script:PythonCmd = $cmd
        $script:UseLauncher = ($cmd -eq "py")
        $script:DetectedPythonVersion = $versionArg
    }
    catch {
        Write-Host "ERROR: Failed to detect Python: $_" -ForegroundColor Red
        exit 1
    }
}

function Install-KittenTTS {
    Write-Step "Installing KittenTTS wheel..."
    
    $tempDir = [System.IO.Path]::GetTempPath()
    $wheelFile = Join-Path $tempDir "kittentts-$KittenTTSVersion-py3-none-any.whl"
    
    try {
        Write-Host "  Downloading from GitHub releases..." -ForegroundColor Gray
        Invoke-WebRequest -Uri $WheelUrl -OutFile $wheelFile -UseBasicParsing
        Write-Host "  Downloaded: $wheelFile" -ForegroundColor Gray
        
        Write-Host "  Installing wheel..." -ForegroundColor Gray
        $prevEAP = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        if ($UseLauncher) {
            & $PythonCmd $DetectedPythonVersion -m pip install $wheelFile --quiet 2>&1 | Out-Null
        }
        else {
            & $PythonCmd -m pip install $wheelFile --quiet 2>&1 | Out-Null
        }
        $ErrorActionPreference = $prevEAP
        
        Write-Host "  Cleaning up..." -ForegroundColor Gray
        Remove-Item $wheelFile -Force -ErrorAction SilentlyContinue
        
        Write-Host "KittenTTS $KittenTTSVersion installed successfully" -ForegroundColor Green
    }
    catch {
        Write-Host "ERROR: Failed to install KittenTTS: $_" -ForegroundColor Red
        exit 1
    }
}

function Install-CliWrapper {
    Write-Step "Setting up CLI wrapper..."
    
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Write-Host "  Created: $InstallDir" -ForegroundColor Gray
    }
    
    $destScript = Join-Path $InstallDir "kittentts-cli.py"
    
    if (Test-Path $CliScript) {
        Copy-Item $CliScript $destScript -Force
        Write-Host "  Copied CLI wrapper to: $destScript" -ForegroundColor Gray
    }
    else {
        Write-Host "WARNING: kittentts-cli.py not found in script directory." -ForegroundColor Yellow
        Write-Host "  Creating inline wrapper..." -ForegroundColor Gray
        
        $wrapperContent = @"
#!/usr/bin/env python3
"""CLI wrapper for KittenTTS - used by CopySpeak application."""

import argparse
import sys
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description="KittenTTS CLI wrapper for CopySpeak")
    parser.add_argument("--text", required=True, help="Text to synthesize")
    parser.add_argument("--voice", default="Jasper", help="Voice name (default: Jasper)")
    parser.add_argument("--output", required=True, help="Output WAV file path")
    parser.add_argument("--model", default="KittenML/kitten-tts-nano-0.8", help="Model name")
    args = parser.parse_args()
    
    try:
        from kittentts import KittenTTS
        import soundfile as sf
        
        tts = KittenTTS(args.model)
        audio = tts.generate(text=args.text, voice=args.voice)
        sf.write(args.output, audio, 24000)
        print("Audio saved to " + args.output, file=sys.stderr)
        
    except ImportError as e:
        print("ERROR: Missing dependency: " + str(e), file=sys.stderr)
        print("Install with: pip install kittentts soundfile", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print("ERROR: " + str(e), file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
"@
        Set-Content -Path $destScript -Value $wrapperContent -Encoding UTF8
        Write-Host "  Created: $destScript" -ForegroundColor Gray
    }
}

function Test-Installation {
    Write-Step "Verifying installation..."
    
    $testText = "CopySpeak test"
    $testOutput = Join-Path ([System.IO.Path]::GetTempPath()) "kittentts_test.wav"
    $cliPath = Join-Path $InstallDir "kittentts-cli.py"
    
    $modelMap = @{
        "nano"  = "KittenML/kitten-tts-nano-0.8"
        "micro" = "KittenML/kitten-tts-micro-0.8"
        "mini"  = "KittenML/kitten-tts-mini-0.8"
    }
    $modelName = $modelMap[$Model]
    
    Write-Host "  Model: $modelName" -ForegroundColor Gray
    Write-Host "  First run will download model (~25-80MB)..." -ForegroundColor Yellow
    
    $cmdArgs = @()
    if ($UseLauncher) {
        $cmdArgs += $DetectedPythonVersion
    }
    $cmdArgs += @($cliPath, "--text", $testText, "--voice", "Jasper", "--output", $testOutput, "--model", $modelName)
    
    if ($Verbose) {
        $argStr = $cmdArgs -join '" "'
        Write-Host "  Command: $PythonCmd `"$argStr`"" -ForegroundColor DarkGray
    }
    
    $prevEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $output = & $PythonCmd @cmdArgs 2>&1
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $prevEAP
    
    if ($exitCode -ne 0) {
        Write-Host ""
        Write-Host "  ========================================" -ForegroundColor Red
        Write-Host "  ERROR: CLI failed with exit code $exitCode" -ForegroundColor Red
        Write-Host "  ========================================" -ForegroundColor Red
        Write-Host ""
        if ($output) {
            Write-Host "  CLI Output:" -ForegroundColor Yellow
            Write-Host "  $('-' * 50)" -ForegroundColor DarkGray
            $output | ForEach-Object { 
                $line = $_.ToString()
                if ($line -match "ERROR|error|Error|Exception|Traceback|failed|Failed|ModuleNotFoundError|ImportError") {
                    Write-Host "  $line" -ForegroundColor Red
                } else {
                    Write-Host "  $line" -ForegroundColor Gray
                }
            }
            Write-Host "  $('-' * 50)" -ForegroundColor DarkGray
        }
        Write-Host ""
        Write-Host "  Possible causes:" -ForegroundColor Cyan
        Write-Host "  - Missing dependency (soundfile, numpy)" -ForegroundColor Gray
        Write-Host "  - Network error during model download" -ForegroundColor Gray
        Write-Host "  - Insufficient disk space" -ForegroundColor Gray
        Write-Host "  - Antivirus blocking Python" -ForegroundColor Gray
        Write-Host ""
        Write-Host "  Try running with -Verbose for more details" -ForegroundColor Yellow
        return $false
    }
    
    if (-not (Test-Path $testOutput)) {
        Write-Host ""
        Write-Host "  ========================================" -ForegroundColor Red
        Write-Host "  ERROR: Output file was not created" -ForegroundColor Red
        Write-Host "  ========================================" -ForegroundColor Red
        Write-Host ""
        Write-Host "  Expected: $testOutput" -ForegroundColor Yellow
        if ($output) {
            Write-Host ""
            Write-Host "  CLI Output:" -ForegroundColor Yellow
            Write-Host "  $('-' * 50)" -ForegroundColor DarkGray
            $output | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
            Write-Host "  $('-' * 50)" -ForegroundColor DarkGray
        }
        Write-Host ""
        Write-Host "  This usually indicates a model loading or audio generation failure." -ForegroundColor Yellow
        return $false
    }
    
    $fileSize = (Get-Item $testOutput).Length
    Write-Host "  Test synthesis successful: $fileSize bytes" -ForegroundColor Green
    Remove-Item $testOutput -Force -ErrorAction SilentlyContinue
    return $true
}

function Show-Success {
    Write-Host ""
    Write-Host "  ========================================" -ForegroundColor Green
    Write-Host "   KittenTTS installed successfully!" -ForegroundColor Green
    Write-Host "  ========================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Model: $Model ($(@{nano="15M, 25MB";micro="40M, 41MB";mini="80M, 80MB"}[$Model]))" -ForegroundColor Cyan
    Write-Host "  CLI: $InstallDir\kittentts-cli.py" -ForegroundColor Cyan
    Write-Host "  Python: $PythonCmd $DetectedPythonVersion" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Available voices: Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo" -ForegroundColor Gray
    Write-Host ""
    Write-Host "  Next steps:" -ForegroundColor Yellow
    Write-Host "  1. Open CopySpeak" -ForegroundColor Gray
    Write-Host "  2. Go to Engine settings" -ForegroundColor Gray
    Write-Host "  3. Select 'KittenTTS' tab" -ForegroundColor Gray
    Write-Host "  4. Click 'Test Engine' to verify" -ForegroundColor Gray
    Write-Host ""
}

function Update-Config {
    Write-Step "Updating CopySpeak configuration..."
    
    $configDir = Join-Path $env:APPDATA "CopySpeak"
    $configPath = Join-Path $configDir "config.json"
    
    $pythonCommand = if ($UseLauncher) { "py" } else { $PythonCmd }
    $versionArg = if ($DetectedPythonVersion) { $DetectedPythonVersion } else { "" }
    $cliPath = "{home_dir}/kittentts/kittentts-cli.py"
    
    if ($versionArg) {
        $argsTemplate = @(
            $versionArg
            $cliPath
            "--text"
            "{raw_text}"
            "--voice"
            "{voice}"
            "--output"
            "{output}"
        )
    } else {
        $argsTemplate = @(
            $cliPath
            "--text"
            "{raw_text}"
            "--voice"
            "{voice}"
            "--output"
            "{output}"
        )
    }
    
    if (Test-Path $configPath) {
        try {
            $config = Get-Content $configPath -Raw | ConvertFrom-Json
            
            if ($config.tts) {
                $config.tts.command = $pythonCommand
                $config.tts.args_template = $argsTemplate
                
                $config | ConvertTo-Json -Depth 10 | Set-Content $configPath -Encoding UTF8
                Write-Host "  Updated TTS command to: $pythonCommand $versionArg" -ForegroundColor Gray
            } else {
                Write-Host "  Warning: Config exists but has no TTS section" -ForegroundColor Yellow
            }
        }
        catch {
            Write-Host "  Warning: Could not update CopySpeak config: $_" -ForegroundColor Yellow
        }
    } else {
        try {
            if (-not (Test-Path $configDir)) {
                New-Item -ItemType Directory -Path $configDir -Force | Out-Null
            }
            
            $defaultConfig = @{
                version = "1.0"
                general = @{
                    language = "en"
                    start_minimized = $false
                    auto_start = $false
                    check_updates = $true
                }
                tts = @{
                    active_backend = "Local"
                    preset = "kitten-tts"
                    command = $pythonCommand
                    args_template = $argsTemplate
                    voice = "Rosie"
                    openai = @{
                        api_key = ""
                        model = "tts-1"
                        voice = "alloy"
                        speed = 1.0
                    }
                    elevenlabs = @{
                        api_key = ""
                        voice_id = ""
                        model = "eleven_multilingual_v2"
                        voice_style = 0
                        use_speaker_boost = $false
                    }
                }
                audio = @{
                    volume = 1.0
                    speed = 1.0
                    retrigger_mode = "Queue"
                }
                history = @{
                    enabled = $true
                    max_items = 100
                    auto_delete_days = 0
                }
            }
            
            $defaultConfig | ConvertTo-Json -Depth 10 | Set-Content $configPath -Encoding UTF8
            Write-Host "  Created config with TTS command: $pythonCommand $versionArg" -ForegroundColor Gray
        }
        catch {
            Write-Host "  Warning: Could not create CopySpeak config: $_" -ForegroundColor Yellow
            Write-Host "  You may need to manually configure the TTS engine in CopySpeak." -ForegroundColor Yellow
        }
    }
}

function Show-Usage {
    Write-Host ""
    Write-Host "  Usage: kittentts-cli.py --text `"Hello world`" --voice Jasper --output output.wav" -ForegroundColor Gray
    Write-Host ""
}

# Main execution
Write-Host ""
Write-Host "  ========================================" -ForegroundColor Magenta
Write-Host "  |  KittenTTS Installer for CopySpeak  |" -ForegroundColor Magenta
Write-Host "  ========================================" -ForegroundColor Magenta

Test-Python

if (-not $Force) {
    try {
        Invoke-Python -Arguments "-c", "import kittentts" 2>$null
        if ($LASTEXITCODE -eq 0) {
            Write-Host ""
            Write-Host "  KittenTTS is already installed. Use -Force to reinstall." -ForegroundColor Yellow
            Show-Usage
        }
    }
    catch {}
}

Install-KittenTTS
Install-CliWrapper

if (Test-Installation) {
    Update-Config
    Show-Success
}
else {
    Write-Host ""
    Write-Host "  Installation completed but test failed." -ForegroundColor Yellow
    Write-Host "  Check the error messages above for details." -ForegroundColor Yellow
    Write-Host ""
    exit 1
}
