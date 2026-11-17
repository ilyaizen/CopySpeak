#!/usr/bin/env python3
"""Download the tiny-voca ONNX model and phonemizer data (used by install-tiny-voca.ps1).

Fetches onnx/model.onnx and phonemizer/* from the public n4ze3m/tiny-voca repo
into --dest. The token is disabled before huggingface_hub loads: a stale
cached token breaks even ungated downloads.
"""

import argparse
import os

os.environ["HF_HUB_DISABLE_IMPLICIT_TOKEN"] = "1"

from huggingface_hub import snapshot_download  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch tiny-voca weights")
    parser.add_argument("--dest", required=True, help="Folder to write weights into")
    args = parser.parse_args()
    snapshot_download(
        repo_id="n4ze3m/tiny-voca",
        local_dir=args.dest,
        allow_patterns=["onnx/model.onnx", "phonemizer/*"],
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
