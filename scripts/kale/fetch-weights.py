#!/usr/bin/env python3
"""Download kale-tts-1-nano weights, G2P data, and runtime files
(used by install-kale.ps1 and install-kale.sh).

Fetches from the public KaleComputer/kale-tts-1-nano repo (Apache-2.0) into
--dest:
    kale-tts-1-nano.onnx    int8 CPU model (the fp32 browser file is skipped)
    tokenizer.json, voices.json, voices/*.bin
    frontend/words/*        the G2P front-end data (US + British)
    python/kale_tts.py, python/kale_frontend.js   the runtime, copied by the
                            installer beside the wrapper

The token is disabled before huggingface_hub loads: a stale cached token
breaks even ungated downloads.

Licensing note carried from the model card: weights and code are Apache-2.0,
the training dataset CC BY 4.0. The training audio is Maya-1 model output;
the voices must never be presented as a recording of a human being.
"""

import argparse
import os

os.environ["HF_HUB_DISABLE_IMPLICIT_TOKEN"] = "1"

from huggingface_hub import snapshot_download  # noqa: E402

PATTERNS = [
    "kale-tts-1-nano.onnx",
    "tokenizer.json",
    "voices.json",
    "voices/*.bin",
    "frontend/words/*",
    "python/kale_tts.py",
    "python/kale_frontend.js",
]


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch kale-tts-1-nano weights")
    parser.add_argument("--dest", required=True, help="Folder to write weights into")
    args = parser.parse_args()
    snapshot_download(
        repo_id="KaleComputer/kale-tts-1-nano",
        local_dir=args.dest,
        allow_patterns=PATTERNS,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
