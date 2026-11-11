#!/usr/bin/env python3
"""CopySpeak local-engine bench: TTFA, RAM, and VRAM at the daemon protocol layer.

Stage 0 of docs_internal/egm/concepts_brief.md. No product code changes.

Definitions
- TTFA      : request-line write -> first PCM payload byte read from the wrapper.
- load      : process spawn -> the READY 2 handshake (includes the wrapper's
              pre-handshake warm-up; Qwen does not warm up).
- cold TTFA : the first request after READY 2 (captures per-call first costs).
- warm TTFA : median over N timed requests; request 2 after READY is discarded
              as settle.
- full      : request write -> the {"end":true} frame.
- RAM       : summed working set (RSS) of the daemon process tree (uv + python),
              sampled on a fixed interval.
- VRAM      : device-wide GPU memory in use (NVML), reported as a delta against
              the pre-spawn baseline.

Usage (from the repo root):
    uv run --project scripts/bench python scripts/bench/bench.py
    uv run --project scripts/bench python scripts/bench/bench.py --engines piper --runs 3
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import queue
import shutil
import statistics
import subprocess
import sys
import threading
import time
from datetime import datetime
from pathlib import Path

import psutil

try:
    import pynvml as nvml
except Exception:  # no NVIDIA driver, or package missing
    nvml = None

REPO_ROOT = Path(__file__).resolve().parents[2]
ENGINE_ROOT = Path(
    os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData" / "Local"))
) / "CopySpeak" / "engines"
CREATE_NO_WINDOW = 0x08000000 if os.name == "nt" else 0

# Defaults mirror the app's default profiles and catalog
# (src-tauri/src/commands/tts/helpers.rs first_class_local_cli +
# src-tauri/src/config/tts.rs default profiles). The default-profile shape for
# Kitten sends NO --model/--speed, so the wrapper defaults ship: nano-0.8, 1.0.
ENGINE_SPECS: dict[str, dict] = {
    "kitten": {"wrapper": "scripts/copyspeak-kitten.py", "voice": "Rosie", "model": None},
    "qwen": {
        "wrapper": "scripts/copyspeak-qwen.py",
        "voice": "Aiden",
        "model": "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice",
    },
    "piper": {"wrapper": "scripts/copyspeak-piper.py", "voice": "en_US-amy-medium", "model": None},
    "kokoro": {"wrapper": "scripts/copyspeak-kokoro.py", "voice": "af_heart", "model": None},
    "pocket": {"wrapper": "scripts/copyspeak-pocket.py", "voice": "alba", "model": None},
}
ENGINE_ORDER = ["kitten", "qwen", "piper", "kokoro", "pocket"]

SENTENCE = (
    "The morning train arrived late, so Maya read forty pages of her novel "
    "before the platform announcements finally called her name."
)

_FRAG_SENTENCES = [
    "The old workshop smelled of cedar shavings and machine oil.",
    "Every morning, Tomas swept the floor before the first customer arrived.",
    "Then he set his tools in a neat row beside the vise.",
    "His grandfather had built violins in this same room.",
    "The paper templates still hung from pegs along the north wall.",
    "Tomas traced one template with a fingertip.",
    "He felt the smooth curve worn by three generations of hands.",
    "Outside, a trolley rattled past the window.",
    "A neighbor called a greeting and he answered without looking up.",
    "Then he tuned a string, listened, and adjusted the peg a quarter turn.",
    "The radio on the shelf played low, half music and half static.",
    "By noon the light through the dusty glass had turned the color of honey.",
    "He glued the ribs, clamped the mold, and wrote the date in pencil.",
    "The last customer of the day wanted a repair, not a new build.",
]


def fragment_text() -> str:
    """Largest whole-sentence text at or under the 600-char default fragment
    size (src-tauri/src/config/sanitization.rs:80)."""
    parts: list[str] = []
    size = 0
    for sentence in _FRAG_SENTENCES:
        candidate = (" ".join(parts + [sentence]))
        if len(candidate) > 600:
            break
        parts.append(sentence)
        size = len(candidate)
    assert size > 400, "fragment text came out too short"
    return " ".join(parts)


class EngineError(Exception):
    """The wrapper replied {"ok":false, ...}."""


class ProtocolError(Exception):
    """The wrapper broke the v2 framing."""


class PipeClosed(Exception):
    """EOF on the wrapper's stdout."""


class Vram:
    """Device-wide GPU memory sampler via NVML. None when unavailable."""

    def __init__(self) -> None:
        self.handle = None
        self.name = None
        self.driver = None
        self.cuda_version = None
        self.total_mib = None
        if nvml is None:
            return
        try:
            nvml.nvmlInit()
            self.handle = nvml.nvmlDeviceGetHandleByIndex(0)
            info = nvml.nvmlDeviceGetMemoryInfo(self.handle)
            self.name = nvml.nvmlDeviceGetName(self.handle)
            if isinstance(self.name, bytes):
                self.name = self.name.decode()
            self.driver = nvml.nvmlSystemGetDriverVersion()
            if isinstance(self.driver, bytes):
                self.driver = self.driver.decode()
            self.cuda_version = nvml.nvmlSystemGetCudaDriverVersion()
            self.total_mib = info.total / (1024 * 1024)
        except Exception:
            self.handle = None

    @property
    def available(self) -> bool:
        return self.handle is not None

    def used_mib(self) -> float | None:
        if self.handle is None:
            return None
        try:
            return nvml.nvmlDeviceGetMemoryInfo(self.handle).used / (1024 * 1024)
        except Exception:
            return None

    def cuda_version_string(self) -> str | None:
        if self.cuda_version is None:
            return None
        major, minor = divmod(self.cuda_version, 1000)
        return f"{major}.{minor}"


class Session:
    """One resident wrapper daemon speaking protocol v2, with process-tree and
    VRAM sampling. Mirrors local_daemon.rs's spawn + protocol handling."""

    def __init__(self, engine: str, spec: dict, device: str, command: list[str],
                 stderr_path: Path, ready_timeout: float, request_timeout: float,
                 end_timeout: float, sample_interval: float, vram: Vram) -> None:
        self.engine = engine
        self.spec = spec
        self.device = device
        self.ready_timeout = ready_timeout
        self.request_timeout = request_timeout
        self.end_timeout = end_timeout
        self.vram = vram
        self.samples: list[tuple[float, float | None, float | None]] = []
        self.stderr_lines: list[str] = []
        self.providers: list[str] = []
        self.status = "ok"
        self.error: str | None = None

        self.args = command
        self.t_spawn = time.perf_counter()
        self.proc = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            creationflags=CREATE_NO_WINDOW,
            env=child_env(),
        )
        self._stderr_file = stderr_path.open("wb")
        self._stderr_thread = threading.Thread(target=self._pump_stderr, daemon=True)
        self._stderr_thread.start()

        self._req_q: queue.Queue = queue.Queue()
        self._reply_q: queue.Queue = queue.Queue()
        self._reader = threading.Thread(target=self._reader_loop, daemon=True)
        self._reader.start()

        self._stop = threading.Event()
        self._sampler = threading.Thread(target=self._sample_loop, args=(sample_interval,), daemon=True)
        self._sampler.start()

        # Handshake: READY 2, skipping stray stdout lines like the Rust side.
        self.t_ready: float | None = None
        deadline = time.monotonic() + ready_timeout
        try:
            while True:
                remaining = max(0.1, deadline - time.monotonic())
                try:
                    kind, payload = self._reply_q.get(timeout=remaining)
                except queue.Empty:
                    self.status = "load_failed"
                    self.error = f"wrapper did not reach READY within {ready_timeout:.0f}s"
                    raise PipeClosed(self.error) from None
                if kind == "ready":
                    self.t_ready = time.perf_counter()
                    break
                if kind == "handshake_error":
                    self.status = "load_failed"
                    self.error = payload
                    raise PipeClosed(payload)
        except BaseException:
            # __init__ failed, so run_case's finally has no Session to tear down.
            # Kill the tree and stop the threads here instead of leaking them.
            self.teardown()
            raise

    # ── protocol ──────────────────────────────────────────────────────────

    def request(self, text: str) -> dict:
        """Send one request; return timing + stream stats. Raises on error.

        `request_timeout` bounds the wait for the FIRST PCM byte (the TTFA
        budget). After first audio, `end_timeout` bounds the rest of the
        stream; a stream that outlives it returns a partial result with
        full_s=None rather than discarding the TTFA measurement.
        """
        assert self.proc.stdin is not None
        if not self._reader.is_alive():
            raise PipeClosed("reader thread stopped; pipe is dead")
        line = (json.dumps({"text": text}, ensure_ascii=True) + "\n").encode("utf-8")
        t_write = time.perf_counter()
        self.proc.stdin.write(line)
        self.proc.stdin.flush()
        self._req_q.put(("request", t_write))
        try:
            kind, payload = self._reply_q.get(timeout=self.request_timeout)
        except queue.Empty:
            raise PipeClosed(f"no first PCM byte within {self.request_timeout:.0f}s")
        if kind == "reply_error":
            raise payload
        if kind == "first_byte":
            ttfa = payload["ttfa_s"]
            try:
                kind, payload = self._reply_q.get(timeout=self.end_timeout)
            except queue.Empty:
                payload = {"ttfa_s": ttfa, "full_s": None, "bytes": None, "chunks": None,
                           "caption_frames": None, "sample_rate": None, "channels": None,
                           "end_timeout": True}
        payload["text_len"] = len(text)
        return payload

    def _reader_loop(self) -> None:
        out = self.proc.stdout
        # Handshake phase: tolerate stray stdout lines (read_handshake analog).
        while True:
            line = out.readline()
            if not line:
                self._reply_q.put(("handshake_error", "wrapper exited without READY"))
                return
            text = line.decode("utf-8", "replace").strip()
            if text == "READY 2":
                self._reply_q.put(("ready", None))
                break
            if text.startswith("READY"):
                self._reply_q.put(("handshake_error", f"unexpected handshake: {text!r}"))
                return
            if text:
                self._stderr_lines.append(f"[stdout] {text}")
        # Reply phase: one reply per queued request.
        while True:
            cmd = self._req_q.get()
            if cmd[0] == "quit":
                return
            t_write = cmd[1]
            try:
                reply = self._read_reply(out, t_write)
            except (EngineError, ProtocolError, PipeClosed) as exc:
                self._reply_q.put(("reply_error", exc))
                return
            self._reply_q.put(("reply", reply))

    def _read_reply(self, out, t_write: float) -> dict:
        header: dict | None = None
        t_first: float | None = None
        t_end: float | None = None
        chunks = 0
        total_bytes = 0
        caption_frames = 0
        while True:
            line = out.readline()
            if not line:
                raise PipeClosed("daemon closed its output mid-reply")
            text = line.decode("utf-8", "replace").strip()
            try:
                frame = json.loads(text)
            except json.JSONDecodeError:
                raise ProtocolError(f"unparseable frame: {text[:100]!r}")
            if frame.get("ok") is False:
                raise EngineError(str(frame.get("error", "unknown daemon error")))
            if header is None and frame.get("ok") is True:
                header = frame
                continue
            if "captions" in frame and isinstance(frame, dict) and len(frame) == 1:
                caption_frames += 1
                continue
            if frame.get("end") is True:
                t_end = time.perf_counter()
                break
            n = frame.get("chunk")
            if isinstance(n, int) and n >= 0:
                data = out.read(n)
                if len(data) != n:
                    raise PipeClosed(f"short read of {n}-byte chunk: got {len(data)}")
                if t_first is None:
                    t_first = time.perf_counter()
                    self._reply_q.put(("first_byte", {"ttfa_s": t_first - t_write}))
                chunks += 1
                total_bytes += len(data)
                continue
            raise ProtocolError(f"unexpected frame: {text[:100]!r}")
        if header is None:
            raise ProtocolError("reply ended with no ok header")
        return {
            "ttfa_s": (t_first - t_write) if t_first is not None else None,
            "full_s": (t_end - t_write) if t_end is not None else None,
            "bytes": total_bytes,
            "chunks": chunks,
            "caption_frames": caption_frames,
            "sample_rate": header.get("sample_rate"),
            "channels": header.get("channels"),
        }

    # ── sampling + teardown ───────────────────────────────────────────────

    def _sample_loop(self, interval: float) -> None:
        while not self._stop.is_set():
            rss = self._tree_rss_mib()
            used = self.vram.used_mib()
            self.samples.append((time.perf_counter(), rss, used))
            self._stop.wait(interval)

    def _tree_rss_mib(self) -> float | None:
        try:
            parent = psutil.Process(self.proc.pid)
            procs = [parent, *parent.children(recursive=True)]
            total = 0.0
            for proc in procs:
                try:
                    total += proc.memory_info().rss
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass
            return total / (1024 * 1024)
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            return None

    def _pump_stderr(self) -> None:
        stream = self.proc.stderr
        assert stream is not None
        for raw in iter(stream.readline, b""):
            text = raw.decode("utf-8", "replace").rstrip()
            self._stderr_file.write(raw)
            self._stderr_file.flush()
            self.stderr_lines.append(text)
            if "providers:" in text:
                self.providers.append(text.strip())

    def memory_stats(self, t_last_end: float | None) -> dict:
        rss_load, rss_all, rss_steady, v_all, v_steady = [], [], [], [], []
        v_baseline = self.samples[0][2] if self.samples else None
        for t, rss, used in self.samples:
            if rss is not None:
                rss_all.append(rss)
                if self.t_ready is not None and t <= self.t_ready:
                    rss_load.append(rss)
                if t_last_end is not None and t >= t_last_end:
                    rss_steady.append(rss)
            if used is not None:
                v_all.append(used)
                if t_last_end is None or t >= t_last_end:
                    v_steady.append(used)
        ram = {
            "load_peak_mib": max(rss_load) if rss_load else None,
            "steady_mib": statistics.median(rss_steady) if rss_steady else None,
            "overall_peak_mib": max(rss_all) if rss_all else None,
        }
        if not self.vram.available:
            return {"ram": ram, "vram": None}
        vram = {
            "baseline_mib": v_baseline,
            "steady_mib": statistics.median(v_steady) if v_steady else None,
            "peak_mib": max(v_all) if v_all else None,
        }
        for key, src in (("delta_steady_mib", "steady_mib"), ("delta_peak_mib", "peak_mib")):
            vram[key] = (
                vram[src] - v_baseline
                if vram[src] is not None and v_baseline is not None
                else None
            )
        return {"ram": ram, "vram": vram}

    def teardown(self) -> None:
        try:
            if self.proc.stdin and not self.proc.stdin.closed:
                self.proc.stdin.close()  # wrappers exit 0 on stdin EOF
            self.proc.wait(timeout=10)
        except Exception:
            pass
        try:
            parent = psutil.Process(self.proc.pid)
            for child in parent.children(recursive=True):
                child.kill()
            parent.kill()
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
        try:
            self.proc.wait(timeout=5)
        except Exception:
            pass
        self._stop.set()
        self._stderr_file.close()


def engine_root_dir(engine: str) -> Path:
    return ENGINE_ROOT / engine


_UV: str | None = None


def uv_command() -> str:
    """Absolute path to uv. Resolved once; falls back to the bare name."""
    global _UV
    if _UV is None:
        _UV = shutil.which("uv") or "uv"
    return _UV


def child_env() -> dict:
    """Child-process environment for engine spawns.

    The bench itself runs under `uv run`, which sets VIRTUAL_ENV to the bench
    venv. A nested `uv run --project <engine>` then sees a mismatch and dies.
    The Rust app never sets VIRTUAL_ENV, so stripping it mirrors production.
    """
    env = dict(os.environ)
    env.pop("VIRTUAL_ENV", None)
    return env


def serve_args(engine_dir: Path, spec: dict, device: str) -> list[str]:
    """Mirror CliTtsBackend::serve_args for the default-profile shape."""
    args = [
        "run",
        "--project",
        str(engine_dir),
        "python",
        str(engine_dir / spec["wrapper"]),
        "--voice",
        spec["voice"],
    ]
    if spec["model"]:
        args += ["--model", spec["model"]]
    if device == "cuda":
        args += ["--device", "cuda"]
    args.append("--serve")
    return args


ENV_PROBE = r"""
import json, sys
d = {"python": ".".join(map(str, sys.version_info[:3]))}
try:
    import torch
    d["torch"] = torch.__version__
    d["torch_cuda"] = bool(torch.cuda.is_available())
    if d["torch_cuda"]:
        d["gpu_name"] = torch.cuda.get_device_name(0)
except Exception as e:
    d["torch_error"] = repr(e)[:120]
try:
    import onnxruntime
    d["onnxruntime"] = onnxruntime.__version__
    d["onnxruntime_providers"] = list(onnxruntime.get_available_providers())
except Exception as e:
    d["onnxruntime_error"] = repr(e)[:120]
print(json.dumps(d))
"""


def engine_env(engine: str, timeout: float = 180.0) -> dict:
    engine_dir = engine_root_dir(engine)
    try:
        proc = subprocess.run(
            [uv_command(), "run", "--project", str(engine_dir), "python", "-c", ENV_PROBE],
            capture_output=True,
            text=True,
            timeout=timeout,
            creationflags=CREATE_NO_WINDOW,
            env=child_env(),
        )
        for line in reversed(proc.stdout.splitlines()):
            line = line.strip()
            if line.startswith("{"):
                return json.loads(line)
        return {"error": (proc.stderr or proc.stdout).strip()[-300:] or "no output"}
    except Exception as exc:
        return {"error": repr(exc)[:200]}


def host_env(vram: Vram) -> dict:
    return {
        "timestamp": datetime.now().astimezone().isoformat(timespec="seconds"),
        "os": platform.platform(),
        "python_bench": platform.python_version(),
        "cpu": platform.processor() or platform.machine(),
        "cpu_cores_physical": psutil.cpu_count(logical=False),
        "cpu_cores_logical": psutil.cpu_count(logical=True),
        "ram_total_mib": round(psutil.virtual_memory().total / (1024 * 1024), 1),
        "gpu": vram.name,
        "gpu_driver": vram.driver,
        "gpu_cuda_driver": vram.cuda_version_string(),
        "gpu_total_mib": round(vram.total_mib, 1) if vram.total_mib else None,
    }


def silent_cpu_fallback(device: str, providers: list[str], status: str) -> bool | None:
    """Concept-brief finding 8: --device cuda that ends up CPU-only.

    Judges ONLY the resolved `providers:` lines. ONNX UserWarning text mentions
    'CUDAExecutionProvider' even when it is unavailable, so warning lines must
    not count as CUDA use.
    """
    if device != "cuda" or status != "ok":
        return None
    # The wrapper emits exactly one `providers: [...]` line; ONNX UserWarnings
    # mention "Available providers:" too, so require the bracketed list shape.
    provider_lines = [line for line in providers if "providers: [" in line]
    if not provider_lines:
        return None  # torch engines print no providers line
    return not any("CUDAExecutionProvider" in line for line in provider_lines)


def run_case(engine: str, spec: dict, device: str, input_name: str, text: str,
             opts: argparse.Namespace, vram: Vram, out_dir: Path) -> dict:
    stderr_path = out_dir / "stderr" / f"{engine}-{device}-{input_name}.log"
    command = [uv_command(), *serve_args(engine_root_dir(engine), spec, device)]
    result: dict = {
        "engine": engine,
        "device": device,
        "input": input_name,
        "text_len": len(text),
        "args": command,
        "status": "ok",
    }
    session = None
    try:
        session = Session(engine, spec, device, command, stderr_path, opts.ready_timeout,
                          opts.request_timeout, opts.end_timeout, opts.sample_interval, vram)
        result["load_s"] = round(session.t_ready - session.t_spawn, 3)

        cold = session.request(text)
        result["cold"] = {k: (round(v, 4) if isinstance(v, float) else v) for k, v in cold.items()}

        t_last_end = time.perf_counter()
        if cold.get("end_timeout"):
            result["status"] = "partial"
            result["partial_reason"] = "cold stream exceeded --end-timeout; TTFA recorded, full not"
        else:
            settle = session.request(text)  # settle, discarded
            if settle.get("end_timeout"):
                result["status"] = "partial"
                result["partial_reason"] = ("settle stream exceeded --end-timeout; "
                                            "cold TTFA recorded only")
            else:
                warm_runs = []
                for _ in range(opts.runs):
                    run = session.request(text)
                    warm_runs.append({k: (round(v, 4) if isinstance(v, float) else v)
                                      for k, v in run.items()})
                    if run.get("full_s") is not None:
                        t_last_end = time.perf_counter()
                    if run.get("end_timeout"):
                        result["status"] = "partial"
                        result["partial_reason"] = ("warm stream exceeded --end-timeout; "
                                                    "completed runs' TTFA recorded")
                        break
                if warm_runs:
                    ttfas = [r["ttfa_s"] for r in warm_runs if r["ttfa_s"] is not None]
                    fulls = [r["full_s"] for r in warm_runs if r["full_s"] is not None]
                    result["warm"] = {
                        "runs": warm_runs,
                        "ttfa_median_s": round(statistics.median(ttfas), 4) if ttfas else None,
                        "ttfa_min_s": round(min(ttfas), 4) if ttfas else None,
                        "ttfa_max_s": round(max(ttfas), 4) if ttfas else None,
                        "full_median_s": round(statistics.median(fulls), 4) if fulls else None,
                    }
        if result["status"] == "partial":
            t_last_end = None  # steady window is the whole session

        time.sleep(opts.dwell)  # steady-state sampling window
        stats = session.memory_stats(t_last_end)
        result["ram_mib"] = {k: round(v, 1) if v is not None else None for k, v in stats["ram"].items()}
        if stats["vram"] is not None:
            result["vram_mib"] = {
                k: round(v, 1) if v is not None else None for k, v in stats["vram"].items()
            }
        result["providers"] = session.providers
        result["silent_cpu_fallback"] = silent_cpu_fallback(device, session.providers, result["status"])
        result["stderr_tail"] = session.stderr_lines[-30:]
    except (EngineError, ProtocolError, PipeClosed) as exc:
        result["status"] = "load_failed" if session is None or session.t_ready is None else "request_failed"
        result["error"] = str(exc)
        if session is not None:
            result["providers"] = session.providers
            result["stderr_tail"] = session.stderr_lines[-30:]
            time.sleep(opts.dwell)
            stats = session.memory_stats(None)
            result["ram_mib"] = {k: round(v, 1) if v is not None else None for k, v in stats["ram"].items()}
    finally:
        if session is not None:
            session.teardown()
    return result


def run_engine(engine: str, spec: dict, devices: list[str], opts: argparse.Namespace,
               vram: Vram, out_dir: Path) -> list[dict]:
    rows: list[dict] = []
    env = engine_env(engine)
    print(f"  env: {json.dumps(env)[:220]}")
    dead_device: set[str] = set()
    for device in devices:
        if device not in dead_device and device == "cuda" and not vram.available:
            rows.append({"engine": engine, "device": "cuda", "input": "-",
                         "status": "no-nvidia-driver"})
            dead_device.add("cuda")
            continue
        for input_name, text in (("sentence", SENTENCE), ("fragment", fragment_text())):
            if input_name not in opts.inputs:
                continue
            if device in dead_device:
                rows.append({"engine": engine, "device": device, "input": input_name,
                             "status": "skipped-after-load-failure"})
                continue
            t0 = time.perf_counter()
            print(f"  running {engine} / {device} / {input_name} ...", flush=True)
            row = run_case(engine, spec, device, input_name, text, opts, vram, out_dir)
            ttfa = (row.get("warm") or {}).get("ttfa_median_s")
            print(f"    -> {row['status']} load={row.get('load_s', '-')}s "
                  f"warm_ttfa_med={ttfa}s ({time.perf_counter() - t0:.0f}s)", flush=True)
            rows.append(row)
            if row["status"] == "load_failed":
                dead_device.add(device)
    return [{"env": env, "rows": rows}]


def fmt(value, unit="") -> str:
    if value is None:
        return "-"
    return f"{value}{unit}"


def write_summary(results: dict, out_dir: Path) -> None:
    lines: list[str] = []
    meta = results["meta"]
    lines.append("# Local-engine baseline (Stage 0)")
    lines.append("")
    lines.append(f"Recorded {meta['host']['timestamp']}. Harness: `scripts/bench/`, "
                 "daemon protocol v2, no app code involved.")
    lines.append("")
    lines.append("Definitions: TTFA = request write to first PCM byte; load = spawn to "
                 "READY 2; RAM = process-tree working set; VRAM = device-wide NVML delta "
                 "vs pre-spawn baseline; warm = median of "
                 f"{meta['runs']} timed runs after one cold + one settle request.")
    lines.append("")
    lines.append("## Host")
    for key, value in meta["host"].items():
        lines.append(f"- {key}: {value}")
    lines.append("")
    for engine, engine_result in results["engines"].items():
        env = engine_result["env"]
        lines.append(f"## {engine}")
        lines.append("")
        lines.append(f"Env: `{json.dumps(env)}`")
        lines.append("")
        for device in ("cpu", "cuda"):
            rows = [r for r in engine_result["rows"] if r.get("device") == device]
            if not rows:
                continue
            lines.append(f"### {device}")
            lines.append("")
            lines.append("| input | status | load s | cold TTFA s | warm TTFA med s "
                         "| warm min–max | full med s | RAM steady MiB | RAM peak MiB "
                         "| VRAM Δsteady MiB | VRAM Δpeak MiB | notes |")
            lines.append("|---|---|---|---|---|---|---|---|---|---|---|---|")
            for row in rows:
                warm = row.get("warm") or {}
                notes = []
                if row.get("silent_cpu_fallback"):
                    notes.append("SILENT CPU FALLBACK")
                if row.get("partial_reason"):
                    notes.append(row["partial_reason"])
                if row.get("error"):
                    notes.append(str(row["error"])[:120])
                if row.get("providers"):
                    notes.append(row["providers"][-1][:80])
                lines.append("| " + " | ".join([
                    row.get("input", "-"),
                    row.get("status", "-"),
                    fmt(row.get("load_s")),
                    fmt((row.get("cold") or {}).get("ttfa_s")),
                    fmt(warm.get("ttfa_median_s")),
                    f"{fmt(warm.get('ttfa_min_s'))}–{fmt(warm.get('ttfa_max_s'))}",
                    fmt(warm.get("full_median_s")),
                    fmt((row.get("ram_mib") or {}).get("steady_mib")),
                    fmt((row.get("ram_mib") or {}).get("overall_peak_mib")),
                    fmt((row.get("vram_mib") or {}).get("delta_steady_mib")),
                    fmt((row.get("vram_mib") or {}).get("delta_peak_mib")),
                    "; ".join(notes),
                ]) + " |")
            lines.append("")
        skipped = [r for r in engine_result["rows"] if r.get("device") == "-"]
        for row in skipped:
            lines.append(f"- {row.get('status')}")
        lines.append("")
    (out_dir / "summary.md").write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--engines", default=",".join(ENGINE_ORDER),
                        help="Comma list from: " + ",".join(ENGINE_ORDER))
    parser.add_argument("--devices", default="cpu,cuda", help="cpu and/or cuda")
    parser.add_argument("--runs", type=int, default=5, help="Timed warm runs per input")
    parser.add_argument("--ready-timeout", type=float, default=300.0)
    parser.add_argument("--request-timeout", type=float, default=300.0,
                        help="Budget for the FIRST PCM byte per request, seconds")
    parser.add_argument("--end-timeout", type=float, default=600.0,
                        help="Budget for the rest of the stream after first audio, seconds; "
                             "exceeding it keeps the TTFA and marks the row partial")
    parser.add_argument("--inputs", default="sentence,fragment", help="sentence and/or fragment")
    parser.add_argument("--sample-interval", type=float, default=0.2)
    parser.add_argument("--dwell", type=float, default=1.5,
                        help="Steady-state sampling window after the last request, seconds")
    parser.add_argument("--out", type=Path, default=None,
                        help="Output dir (default docs_internal/egm/baseline-<timestamp>)")
    opts = parser.parse_args()

    vram = Vram()
    if vram.available:
        print(f"GPU: {vram.name}, driver {vram.driver}, cuda {vram.cuda_version_string()}, "
              f"{vram.total_mib:.0f} MiB total")
    else:
        print("GPU: NVML unavailable; VRAM rows will be null")
    print(f"Host RAM: {psutil.virtual_memory().total / (1024**3):.1f} GiB")

    engines = [e.strip() for e in opts.engines.split(",") if e.strip()]
    devices = [d.strip() for d in opts.devices.split(",") if d.strip()]
    inputs = [i.strip() for i in opts.inputs.split(",") if i.strip()]
    opts.inputs = inputs

    out_dir = opts.out or (REPO_ROOT / "docs_internal" / "egm"
                           / f"baseline-{datetime.now():%Y%m%d-%H%M%S}")
    (out_dir / "stderr").mkdir(parents=True, exist_ok=True)

    results: dict = {
        "meta": {
            "definitions": {
                "ttfa": "request write -> first PCM payload byte",
                "load": "spawn -> READY 2 handshake",
                "cold": "first request after READY 2",
                "warm": f"median of {opts.runs} timed runs after one settle request",
                "ram": "process-tree working set (psutil RSS), sampled "
                       f"every {opts.sample_interval}s",
                "vram": "device-wide NVML used-memory delta vs pre-spawn baseline",
            },
            "fragment_len": len(fragment_text()),
            "sentence_len": len(SENTENCE),
            "engines_requested": engines,
            "host": host_env(vram),
            "runs": opts.runs,
        },
        "engines": {},
    }

    exit_code = 0
    for engine in engines:
        spec = ENGINE_SPECS.get(engine)
        if spec is None:
            print(f"== {engine}: unknown engine, skipping")
            continue
        print(f"== {engine}", flush=True)
        if not engine_root_dir(engine).is_dir():
            print("   engine dir missing -> engine-not-installed")
            results["engines"][engine] = {
                "env": {"error": "engine dir missing"},
                "rows": [{"engine": engine, "device": "-", "input": "-",
                          "status": "engine-not-installed"}],
            }
            exit_code = 1
            continue
        results["engines"][engine] = run_engine(engine, spec, devices, opts, vram, out_dir)[0]

    (out_dir / "results.json").write_text(
        json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8", newline="\n"
    )
    write_summary(results, out_dir)
    print(f"\nResults: {out_dir / 'results.json'}")
    print(f"Summary: {out_dir / 'summary.md'}")
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
