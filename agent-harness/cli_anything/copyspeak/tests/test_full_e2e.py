import json, os, shutil, subprocess, sys
from pathlib import Path


def _resolve_cli(name):
    force = os.environ.get("CLI_ANYTHING_FORCE_INSTALLED", "").strip() == "1"
    path = shutil.which(name)
    if path:
        print(f"[_resolve_cli] Using installed command: {path}")
        return [path]
    if force:
        raise RuntimeError(f"{name} not found in PATH. Install with: pip install -e .")
    module = name.replace("cli-anything-", "cli_anything.") + "." + name.split("-")[-1] + "_cli"
    print(f"[_resolve_cli] Falling back to: {sys.executable} -m {module}")
    return [sys.executable, "-m", module]


def _assert_audio(path):
    p = Path(path)
    assert p.exists() and p.stat().st_size > 0
    head = p.read_bytes()[:12]
    assert head[:4] == b"RIFF" or head[:3] == b"ID3" or head[:4] in (b"OggS", b"fLaC")
    print(f"\n  AUDIO: {p} ({p.stat().st_size:,} bytes)")


class TestCLISubprocess:
    CLI_BASE = _resolve_cli("cli-anything-copyspeak")

    def _run(self, args, check=True):
        return subprocess.run(self.CLI_BASE + args, capture_output=True, text=True, check=check)

    def test_help(self):
        result = self._run(["--help"])
        assert result.returncode == 0
        assert "project" in result.stdout

    def test_project_new_json(self, tmp_path):
        out = tmp_path / "test.json"
        result = self._run(["--json", "project", "new", "-o", str(out)])
        assert result.returncode == 0
        data = json.loads(result.stdout)
        assert Path(data["path"]).exists()

    def test_queue_workflow_json(self, tmp_path):
        proj = tmp_path / "test.json"
        self._run(["project", "new", "-o", str(proj)])
        self._run(["--project", str(proj), "queue", "add", "-t", "Hello", "--label", "hello"])
        result = self._run(["--project", str(proj), "--json", "queue", "list"])
        assert json.loads(result.stdout)[0]["label"] == "hello"

    def test_real_text_export(self, tmp_path):
        proj = tmp_path / "test.json"; wav = tmp_path / "hello.wav"
        self._run(["project", "new", "-o", str(proj)])
        self._run(["--project", str(proj), "export", "text", "-t", "Hello from CopySpeak TTS", "-o", str(wav), "--overwrite"])
        _assert_audio(wav)

    def test_real_queue_export(self, tmp_path):
        proj = tmp_path / "test.json"; out = tmp_path / "out"
        self._run(["project", "new", "-o", str(proj)])
        self._run(["--project", str(proj), "queue", "add", "-t", "First", "--label", "first"])
        self._run(["--project", str(proj), "queue", "add", "-t", "Second", "--label", "second"])
        self._run(["--project", str(proj), "export", "queue", "-o", str(out), "--overwrite"])
        files = sorted(out.glob("*.wav"))
        assert len(files) == 2
        for f in files: _assert_audio(f)
