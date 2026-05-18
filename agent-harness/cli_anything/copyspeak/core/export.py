from __future__ import annotations
import wave
from . import queue as queue_core
from cli_anything.copyspeak.utils.copyspeak_backend import synthesize


def export_text(project, text, output, overwrite=False):
    result = synthesize(text, output, project.get("config", {}), overwrite=overwrite)
    _verify_audio(result["output"])
    project.setdefault("history", []).append({"text": text, **result})
    return result


def export_queue(project, out_dir, overwrite=False):
    from pathlib import Path
    results=[]
    for i,item in enumerate(queue_core.list_items(project), 1):
        safe = "".join(c if c.isalnum() or c in "-_" else "_" for c in item.get("label") or f"item-{i}")[:48]
        output = Path(out_dir) / f"{i:03d}-{safe}.wav"
        res = export_text(project, item["text"], str(output), overwrite=overwrite)
        item["status"] = "exported"; item["output"] = res["output"]
        results.append(res)
    return {"count": len(results), "outputs": results}


def _verify_audio(path):
    with open(path, "rb") as f:
        head = f.read(12)
    if head[:4] == b"RIFF" and head[8:12] == b"WAVE":
        with wave.open(path, "rb") as w:
            if w.getnframes() <= 0: raise RuntimeError("WAV contains no frames")
        return True
    # Allow other real backend audio formats by magic bytes.
    if head[:3] in (b"ID3",) or head[:4] in (b"OggS", b"fLaC"):
        return True
    raise RuntimeError(f"Unsupported or invalid audio output: {path}")
