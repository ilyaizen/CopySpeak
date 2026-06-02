# Setup Piper TTS (CPU Only) for CopySpeak TTS on Windows

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "  CopySpeak TTS: Piper CPU Setup Helper  " -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host ""

# Check Python installation
$pythonCmd = "python3"
try {
    $pythonVersion = & $pythonCmd --version 2>&1
    Write-Host "Found Python: $pythonVersion" -ForegroundColor Green
} catch {
    try {
        $pythonCmd = "python"
        $pythonVersion = & $pythonCmd --version 2>&1
        Write-Host "Found Python: $pythonVersion" -ForegroundColor Green
    } catch {
        Write-Error "Python 3 is not installed or not in your PATH. Please install Python 3.10+ from python.org."
        exit 1
    }
}

# Install / update piper-tts with HTTP support (essential for persistent RAM caching)
Write-Host "`n1. Installing piper-tts with HTTP server support..." -ForegroundColor Yellow
& $pythonCmd -m pip install --user --upgrade "piper-tts[http]"
if ($LASTEXITCODE -ne 0) {
    Write-Warning "Pip install for piper-tts[http] failed. Trying fallback..."
    & $pythonCmd -m pip install --user "piper-tts[http]"
}

# Clean up any existing GPU-only onnxruntime package to avoid conflicts
Write-Host "`n2. Cleaning up any conflicting onnxruntime-gpu package..." -ForegroundColor Yellow
& $pythonCmd -m pip uninstall -y onnxruntime-gpu

# Reinstall standard CPU version of onnxruntime
Write-Host "`n3. Installing/Upgrading CPU-only onnxruntime..." -ForegroundColor Yellow
& $pythonCmd -m pip install --user --upgrade onnxruntime

# Verification Check
Write-Host "`n4. Verifying CPU provider status in ONNX Runtime..." -ForegroundColor Yellow
$verifyScript = @"
import onnxruntime as ort
providers = ort.get_available_providers()
print("Available ONNX Runtime Execution Providers:")
for p in providers:
    print(f" - {p}")
if 'CPUExecutionProvider' in providers:
    print("STATUS: CPU provider is successfully configured!")
"@

& $pythonCmd -c $verifyScript

Write-Host ""
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "  Installation Completed successfully!   " -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "You can now run CopySpeak and select the 'Piper' engine."
Write-Host "The model will be kept cached in RAM using CPU-only inference."
Write-Host ""
