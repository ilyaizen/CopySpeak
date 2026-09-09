"""Feedback loop for the CopySpeak native-host relay deadlock.

Launches copyspeak-browser-host.exe directly and feeds it hello + start frames
on stdin, exactly as Chrome's worker would. Desktop-side pass/fail signal:
%APPDATA%\\CopySpeak\\logs\\app_rCURRENT.log must show
"[Browser] Hello: automatic=false" then "[Browser] Start: request_id=harness1".

RED (deadlock): host hangs, never exits, desktop logs neither line.
GREEN (fixed):  host forwards both frames, relays the accepted frame to stdout,
                exits cleanly once stdin closes.
"""
import json
import struct
import subprocess
import sys
import time

HOST = r"D:\Projects\CopySpeak\browser-native-host\target\debug\copyspeak-browser-host.exe"
TEXT = "Hello harness world, this is the native host deadlock test passage."


def frame(obj: dict) -> bytes:
    b = json.dumps(obj).encode("utf-8")
    return struct.pack("<I", len(b)) + b


def main() -> int:
    p = subprocess.Popen(
        [HOST],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert p.stdin is not None

    verdict = "UNKNOWN"
    try:
        p.stdin.write(frame({"v": 1, "type": "hello", "automatic": False}))
        p.stdin.flush()
        time.sleep(3)
        alive = p.poll() is None
        print(f"host alive 3s after hello: {alive}")

        if alive:
            p.stdin.write(
                frame({"v": 1, "type": "start", "request_id": "harness1", "text": TEXT})
            )
            p.stdin.flush()
            time.sleep(5)
            print(f"host alive 5s after start: {p.poll() is None}")
            p.stdin.close()
    except (OSError, BrokenPipeError) as e:
        print(f"stdin I/O failed: {e} — host exited early")

    try:
        out, err = p.communicate(timeout=6)
        print(f"host exited rc={p.returncode}")
        verdict = "GREEN" if p.returncode == 0 else "RED (early exit)"
    except subprocess.TimeoutExpired:
        p.kill()
        out, err = p.communicate()
        print("VERDICT: host HUNG after stdin close — deadlock confirmed")
        verdict = "RED (hang)"

    print(f"stdout bytes: {len(out)}")
    while out:
        (n,) = struct.unpack("<I", out[:4])
        body, out = out[4 : 4 + n], out[4 + n :]
        print("  frame: " + body.decode("utf-8", "replace")[:300])
    if err:
        print("stderr: " + err.decode("utf-8", "replace")[:800])
    print(f"VERDICT: {verdict}")
    return 0 if verdict == "GREEN" else 1


if __name__ == "__main__":
    sys.exit(main())
