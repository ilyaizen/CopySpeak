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
    
    [switch]$Force
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
    param([string]$DefaultVersion = "3.12")
    
    # Try Python Launcher first (py.exe)
    $versions = @("3.12", "3.11", "3.10", "3.9", "3.8")
    
    $launcher = Get-Command py -ErrorAction SilentlyContinue
    if ($launcher) {
        foreach ($v in $versions) {
            $prevEAP = $ErrorActionPreference
            $ErrorActionPreference = "SilentlyContinue"
            $result = & py "-$v" --version 2>&1
            $ErrorActionPreference = $prevEAP
            
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  Found Python via launcher: $v" -ForegroundColor Green
                return "py", "-$v"
            }
        }
    }
    
    # Fallback to default python command
    $pythonCmd = Get-Command python -ErrorAction SilentlyContinue
    if ($pythonCmd) {
        $prevEAP = $ErrorActionPreference
        $ErrorActionPreference = "SilentlyContinue"
        $result = & python --version 2>&1
        $ErrorActionPreference = $prevEAP
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  Found Python: $($result.Trim())" -ForegroundColor Green
            return "python", ""
        }
    }
    
    return $null, $null
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
        $cmd, $versionArg = Get-AvailablePython
        
        if ($null -eq $cmd) {
            Write-Host "ERROR: Python not found on your system." -ForegroundColor Red
            Write-Host ""
            $installed = Install-PythonViaWinget
            if (-not $installed) {
                exit 1
            }
            
            # Try again after winget installation
            $cmd, $versionArg = Get-AvailablePython
            if ($null -eq $cmd) {
                Write-Host "ERROR: Python still not found after installation." -ForegroundColor Red
                Write-Host "Please restart your terminal and run the installer again." -ForegroundColor Yellow
                exit 1
            }
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
    
    $prevEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    if ($UseLauncher) {
        & $PythonCmd $DetectedPythonVersion $cliPath --text $testText --voice Jasper --output $testOutput --model $modelName 2>&1 | Out-Null
    }
    else {
        & $PythonCmd $cliPath --text $testText --voice Jasper --output $testOutput --model $modelName 2>&1 | Out-Null
    }
    $ErrorActionPreference = $prevEAP
    
    if (Test-Path $testOutput) {
        $fileSize = (Get-Item $testOutput).Length
        Write-Host "Test synthesis successful: $fileSize bytes" -ForegroundColor Green
        Remove-Item $testOutput -Force -ErrorAction SilentlyContinue
        return $true
    }
    else {
        Write-Host "ERROR: Output file not created" -ForegroundColor Red
        return $false
    }
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
    
    $configPath = Join-Path $env:APPDATA "CopySpeak\config.json"
    
    if (Test-Path $configPath) {
        try {
            $config = Get-Content $configPath -Raw | ConvertFrom-Json
            
            $pythonCommand = if ($UseLauncher) { "py" } else { $PythonCmd }
            $versionArg = if ($DetectedPythonVersion) { $DetectedPythonVersion } else { "" }
            
            if ($config.tts -and $config.tts.active_backend -eq "Local") {
                $config.tts.command = $pythonCommand
                
                # Update args_template based on detected Python
                $cliPath = "{home_dir}/kittentts/kittentts-cli.py"
                if ($versionArg) {
                    $config.tts.args_template = @(
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
                    $config.tts.args_template = @(
                        $cliPath
                        "--text"
                        "{raw_text}"
                        "--voice"
                        "{voice}"
                        "--output"
                        "{output}"
                    )
                }
                
                $config | ConvertTo-Json -Depth 10 | Set-Content $configPath -Encoding UTF8
                Write-Host "  Updated TTS command to: $pythonCommand" -ForegroundColor Gray
                if ($versionArg) {
                    Write-Host "  Python version: $versionArg" -ForegroundColor Gray
                }
            } else {
                Write-Host "  CopySpeak config not found or TTS backend not set to Local." -ForegroundColor Yellow
            }
        }
        catch {
            Write-Host "  Warning: Could not update CopySpeak config: $_" -ForegroundColor Yellow
        }
    } else {
        Write-Host "  CopySpeak config not found. The app will use default settings." -ForegroundColor Yellow
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
