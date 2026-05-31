"""Backend wrappers that invoke the real CopySpeak TTS/TTS software, never reimplement rendering."""
from __future__ import annotations
import os, shutil, subprocess, sys
from pathlib import Path

INSTALL = "Build/install CopySpeak TTS and set COPYSPEAK_EXE, or install Kitten/Piper/Kokoro CLI for synthesis."


def find_copyspeak():
    candidates = [
        os.environ.get("COPYSPEAK_EXE"),
        shutil.which("copyspeak"),
        shutil.which("CopySpeak TTS"),
        shutil.which("copyspeak-tts"),
    ]
    root = Path(__file__).resolve().parents[4]
    candidates += [
        str(root/"src-tauri"/"target"/"release"/"CopySpeak TTS.exe"),
        str(root/"src-tauri"/"target"/"release"/"copyspeak-tts.exe"),
        str(root/"src-tauri"/"target"/"release"/"copyspeak.exe"),
        str(root/"src-tauri"/"target"/"debug"/"CopySpeak TTS.exe"),
        str(root/"src-tauri"/"target"/"debug"/"copyspeak-tts.exe"),
        str(root/"src-tauri"/"target"/"debug"/"copyspeak.exe"),
    ]
    for c in candidates:
        if c and Path(c).exists(): return str(Path(c).resolve())
    raise RuntimeError("CopySpeak TTS executable not found. " + INSTALL)


def find_tts_engine(engine):
    if engine == "kitten":
        env = os.environ.get("KITTENTTS_CLI")
        root = Path(__file__).resolve().parents[4]
        local = root / "kittentts-cli.py"
        if env and Path(env).exists(): return [sys.executable, env]
        if local.exists(): return [sys.executable, str(local)]
    exe = shutil.which(engine) or shutil.which(f"{engine}-tts")
    if exe: return [exe]
    raise RuntimeError(f"TTS engine '{engine}' not found. {INSTALL}")


def synthesize(text, output, config, overwrite=False):
    out = Path(output)
    if out.exists() and not overwrite:
        raise FileExistsError(f"Output exists: {out} (use --overwrite)")
    out.parent.mkdir(parents=True, exist_ok=True)
    engine = config.get("engine", "kitten")
    cmd = find_tts_engine(engine) + ["--text", text, "--output", str(out)]
    if config.get("voice"): cmd += ["--voice", str(config["voice"])]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"TTS backend failed ({result.returncode}): {result.stderr or result.stdout}")
    if not out.exists() or out.stat().st_size <= 0:
        raise RuntimeError(f"TTS backend did not create a non-empty output: {out}")
    return {"output": str(out.resolve()), "file_size": out.stat().st_size, "engine": engine, "method": "real-backend-subprocess"}


def launch_app():
    exe = find_copyspeak()
    subprocess.Popen([exe], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return {"launched": exe}
