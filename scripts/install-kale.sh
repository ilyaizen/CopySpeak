#!/usr/bin/env bash
#
# Installs kale-tts-1-nano for CopySpeech via uv (macOS and Linux port of
# install-kale.ps1). CPU-only, about 29 MB, English text only.
#
# Creates a uv-managed project under $XDG_DATA_HOME/CopySpeech/engines/kale
# (or the macOS equivalent), installs ONNX Runtime, numpy, and quickjs on
# Python 3.12 (the newest release quickjs 1.19.4 ships wheels for), downloads
# the int8 ONNX model, voice tables, G2P front-end data, and runtime files
# from KaleComputer/kale-tts-1-nano, checks the model's sha256, and drops the
# CLI wrapper. No PyTorch, no CUDA - the graph is CPU-only.
#
# Licensing: weights and code Apache-2.0, dataset CC BY 4.0. The training
# audio is Maya-1 model output (Maya-1 use policy): the voices must never be
# presented as a recording of a human being, and must not be used to
# impersonate anyone.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeech-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeech-engine-install.sh"

FORCE=0
SMOKE_TEST=0
VOICES=()

usage() {
    echo "Usage: ./scripts/install-kale.sh [--force] [--smoke-test] [--voices <id> [<id>...]]"
}

while (( $# > 0 )); do
    case "$1" in
        --force) FORCE=1; shift ;;
        --smoke-test) SMOKE_TEST=1; shift ;;
        --voices)
            shift
            # Voice ids never start with "--"; the next flag (or arg end)
            # terminates the list.
            while (( $# > 0 )) && [[ "$1" != --* ]]; do
                VOICES+=("$1")
                shift
            done
            ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

write_engine_banner "kale-tts-1-nano Installer"

require_uv

# Interactive force prompt: --force bypasses; a blank Enter keeps the install.
# --voices (app-driven) is non-interactive: never destructively reinstall.
if (( FORCE )); then
    EFFECTIVE_FORCE=1
elif (( ${#VOICES[@]} > 0 )); then
    EFFECTIVE_FORCE=0
else
    if get_confirmation "Reinstall kale from scratch? (deletes the existing engine dir)" 0; then
        EFFECTIVE_FORCE=1
    else
        EFFECTIVE_FORCE=0
    fi
fi

ENGINE_DIR="$(copyspeech_engine_root)/kale"
new_engine_project "$ENGINE_DIR" "$EFFECTIVE_FORCE"

# quickjs 1.19.4 ships wheels only up to cp312, so the project pins Python
# 3.12. uv init wrote the host Python as the floor; lower it to match.
sed -i.bak 's/^requires-python = .*/requires-python = ">=3.12"/' "$ENGINE_DIR/pyproject.toml"
rm -f "$ENGINE_DIR/pyproject.toml.bak"

# The int8 CPU ONNX file, sha256 from the Hugging Face SHA256SUMS.
MODEL_SHA256="4d5269974de4fba7a66f947608a9a5381b4e39c1a8a6073df6001ab7e7f76288"

SCRIPTS_DIR="$ENGINE_DIR/scripts"
VOICES_DIR="$ENGINE_DIR/voices"
OUTPUT_DIR="$ENGINE_DIR/output"
WEIGHTS_DIR="$ENGINE_DIR/weights"
FRONTEND_DIR="$ENGINE_DIR/frontend"
mkdir -p "$SCRIPTS_DIR" "$VOICES_DIR" "$OUTPUT_DIR"

echo ""
log C_YELLOW "  [STEP] engine"
log C_GRAY "  Installing ONNX Runtime, numpy, quickjs (Python 3.12)..."
uv python install 3.12 >/dev/null 2>&1 || true
if ! invoke_uv add --project "$ENGINE_DIR" --python 3.12 \
    "onnxruntime==1.23.2; sys_platform == 'darwin' and platform_machine == 'x86_64'" \
    "onnxruntime; sys_platform != 'darwin' or platform_machine != 'x86_64'" \
    "numpy" "quickjs" "huggingface-hub>=0.34,<2"; then
    log C_RED "  [ERROR] engine (uv add failed)"
    exit 1
fi

log C_GRAY "  Downloading weights and G2P data (about 29 MB)..."
invoke_uv run --project "$ENGINE_DIR" python "$SCRIPT_DIR/kale/fetch-weights.py" --dest "$WEIGHTS_DIR"
MODEL="$WEIGHTS_DIR/kale-tts-1-nano.onnx"
if [[ ! -f "$MODEL" ]]; then
    log C_RED "  [ERROR] kale model missing after download: $MODEL"
    exit 1
fi
# macOS ships shasum, Linux ships sha256sum; both print the digest first.
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_SHA="$(sha256sum "$MODEL" | awk '{print $1}')"
else
    ACTUAL_SHA="$(shasum -a 256 "$MODEL" | awk '{print $1}')"
fi
if [[ "$ACTUAL_SHA" != "$MODEL_SHA256" ]]; then
    log C_RED "  [ERROR] kale model sha256 mismatch: got $ACTUAL_SHA, expected $MODEL_SHA256"
    exit 1
fi
log C_GRAY "  Model sha256 verified."

# The runtime beside the wrapper (its _near walk then finds frontend/ one
# level up), and the G2P word data at the engine root.
mv -f "$WEIGHTS_DIR/python/kale_tts.py" "$SCRIPTS_DIR/kale_tts.py"
mv -f "$WEIGHTS_DIR/python/kale_frontend.js" "$SCRIPTS_DIR/kale_frontend.js"
rm -rf "$WEIGHTS_DIR/python"
mv -f "$WEIGHTS_DIR/frontend" "$FRONTEND_DIR"
if [[ ! -f "$FRONTEND_DIR/words/g2p.bin" ]]; then
    log C_RED "  [ERROR] kale front-end data missing after download: $FRONTEND_DIR/words"
    exit 1
fi

DST_WRAPPER="$SCRIPTS_DIR/copyspeech-kale.py"
cp -f "$SCRIPT_DIR/kale/copyspeech-kale.py" "$DST_WRAPPER"
log C_GRAY "  Wrapper installed: $DST_WRAPPER"
log C_GREEN "  [DONE] engine"

# All ten voices ship in the one model; Kenzie is the catalog's first voice.
KALE_IDS=(Kenzie Simone Amy Frankie Darlene Owen Toby Jack Earl Magnus)
CHOSEN_VOICE="Kenzie"
if (( ${#VOICES[@]} > 0 )); then
    for id in "${KALE_IDS[@]}"; do
        if [[ "${VOICES[0]}" == "$id" ]]; then
            CHOSEN_VOICE="${VOICES[0]}"
            break
        fi
    done
fi

if (( SMOKE_TEST )); then
    echo ""
    log C_YELLOW "  [STEP] smoke"
    TEST_OUT="$OUTPUT_DIR/test.wav"
    invoke_uv run --project "$ENGINE_DIR" python "$DST_WRAPPER" \
        --text "Hello from kale." --voice "$CHOSEN_VOICE" --output "$TEST_OUT"
    if ! test_audio_file "$TEST_OUT"; then
        log C_RED "  [ERROR] smoke"
        exit 1
    fi
    log C_GREEN "  [DONE] smoke"
fi

PROFILE_JSON='{
  "schema_version": 1,
  "id": "kale-local",
  "name": "kale (Local, English)",
  "engine": "local",
  "voice": "'"$CHOSEN_VOICE"'",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "kale",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/kale", "python", "{engine_dir}/kale/scripts/copyspeech-kale.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}", "--speed", "{speed}"]
  }
}'

write_engine_manifest "$ENGINE_DIR" "${KALE_IDS[@]}"

echo ""
log C_GREEN "  kale-tts-1-nano installed at: $ENGINE_DIR"
log C_GRAY "  Voice note (Maya-1 use policy): the voices are synthetic designs;"
log C_GRAY "  never present their output as a recording of a human being."
write_profile_snippet "$PROFILE_JSON"
