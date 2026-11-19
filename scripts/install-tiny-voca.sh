#!/usr/bin/env bash
#
# Installs tiny-voca for CopySpeech via uv (macOS and Linux port of
# install-tiny-voca.ps1). CPU-only, about 45 MB of weights, English text only.
#
# Creates a uv-managed project under $XDG_DATA_HOME/CopySpeech/engines/tiny-voca
# (or the macOS equivalent), installs ONNX Runtime and the dizione phonemizer
# (pinned commit), downloads the ONNX model and phonemizer data from
# n4ze3m/tiny-voca, checks the model's sha256, and drops the CLI wrapper.
# No PyTorch, no CUDA.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeech-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeech-engine-install.sh"

FORCE=0
SMOKE_TEST=0
VOICES=()

usage() {
    echo "Usage: ./scripts/install-tiny-voca.sh [--force] [--smoke-test] [--voices <id> [<id>...]]"
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

write_engine_banner "tiny-voca Installer"

require_uv

# Interactive force prompt: --force bypasses; a blank Enter keeps the install.
# --voices (app-driven) is non-interactive: never destructively reinstall.
if (( FORCE )); then
    EFFECTIVE_FORCE=1
elif (( ${#VOICES[@]} > 0 )); then
    EFFECTIVE_FORCE=0
else
    if get_confirmation "Reinstall tiny-voca from scratch? (deletes the existing engine dir)" 0; then
        EFFECTIVE_FORCE=1
    else
        EFFECTIVE_FORCE=0
    fi
fi

ENGINE_DIR="$(copyspeech_engine_root)/tiny-voca"
new_engine_project "$ENGINE_DIR" "$EFFECTIVE_FORCE"

# dizione is the phonemizer tiny-voca needs; pinned to a commit, not a branch.
DIZIONE="dizione @ git+https://github.com/n4ze3m/dizione@8cd889e4de7be6bd1dd2e6891344a0729fa14dd4"
# The model's ONNX file, sha256 from the Hugging Face LFS pointer.
MODEL_SHA256="fe6d918791a15b82e37465981d7f27a070e46b46635e5a3482fc8dff87a1ae46"

SCRIPTS_DIR="$ENGINE_DIR/scripts"
VOICES_DIR="$ENGINE_DIR/voices"
OUTPUT_DIR="$ENGINE_DIR/output"
WEIGHTS_DIR="$ENGINE_DIR/weights"
mkdir -p "$SCRIPTS_DIR" "$VOICES_DIR" "$OUTPUT_DIR" "$WEIGHTS_DIR"

echo ""
log C_YELLOW "  [STEP] engine"
log C_GRAY "  Installing ONNX Runtime, numpy, soundfile, dizione..."
# Newer onnxruntime releases ship no Intel macOS wheel (1.31.1 and 1.32 do not),
# so Intel Macs pin 1.23.2, the last checked release with a macosx x86_64 wheel.
# That release has no cp314 wheel either, so Intel Macs build on Python 3.13.
PY_FLAG=()
if [[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "x86_64" ]]; then
    PY_FLAG=(--python 3.13)
    # uv init writes the host Python as the floor; lower it to match 3.13.
    sed -i.bak 's/^requires-python = .*/requires-python = ">=3.13"/' "$ENGINE_DIR/pyproject.toml"
    rm -f "$ENGINE_DIR/pyproject.toml.bak"
fi
if ! invoke_uv add --project "$ENGINE_DIR" ${PY_FLAG[@]+"${PY_FLAG[@]}"} \
    "onnxruntime==1.23.2; sys_platform == 'darwin' and platform_machine == 'x86_64'" \
    "onnxruntime; sys_platform != 'darwin' or platform_machine != 'x86_64'" \
    "numpy" "soundfile" "huggingface-hub>=0.34,<2" "$DIZIONE"; then
    log C_RED "  [ERROR] engine (uv add failed)"
    exit 1
fi

log C_GRAY "  Downloading weights (about 45 MB)..."
invoke_uv run --project "$ENGINE_DIR" python "$SCRIPT_DIR/tiny-voca/fetch-weights.py" --dest "$WEIGHTS_DIR"

MODEL="$WEIGHTS_DIR/onnx/model.onnx"
if [[ ! -f "$MODEL" ]]; then
    log C_RED "  [ERROR] tiny-voca model missing after download: $MODEL"
    exit 1
fi
# macOS ships shasum, Linux ships sha256sum; both print the digest first.
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL_SHA="$(sha256sum "$MODEL" | awk '{print $1}')"
else
    ACTUAL_SHA="$(shasum -a 256 "$MODEL" | awk '{print $1}')"
fi
if [[ "$ACTUAL_SHA" != "$MODEL_SHA256" ]]; then
    log C_RED "  [ERROR] tiny-voca model sha256 mismatch: got $ACTUAL_SHA, expected $MODEL_SHA256"
    exit 1
fi
log C_GRAY "  Model sha256 verified."

DST_WRAPPER="$SCRIPTS_DIR/copyspeech-tiny-voca.py"
cp -f "$SCRIPT_DIR/tiny-voca/copyspeech-tiny-voca.py" "$DST_WRAPPER"
log C_GRAY "  Wrapper installed: $DST_WRAPPER"
log C_GREEN "  [DONE] engine"

# Both voices ship in the one model; F2 is the CLI default upstream.
TINY_IDS=(F2 M2)
CHOSEN_VOICE="M2"
if (( ${#VOICES[@]} > 0 )) && [[ "${VOICES[0]}" == "F2" || "${VOICES[0]}" == "M2" ]]; then
    CHOSEN_VOICE="${VOICES[0]}"
fi

if (( SMOKE_TEST )); then
    echo ""
    log C_YELLOW "  [STEP] smoke"
    TEST_OUT="$OUTPUT_DIR/test.wav"
    invoke_uv run --project "$ENGINE_DIR" python "$DST_WRAPPER" \
        --text "Hello from tiny voca." --voice "$CHOSEN_VOICE" --output "$TEST_OUT"
    if ! test_audio_file "$TEST_OUT"; then
        log C_RED "  [ERROR] smoke"
        exit 1
    fi
    log C_GREEN "  [DONE] smoke"
fi

PROFILE_JSON='{
  "schema_version": 1,
  "id": "tiny-voca-local",
  "name": "tiny-voca (Local, English)",
  "engine": "local",
  "voice": "'"$CHOSEN_VOICE"'",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "tiny-voca",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/tiny-voca", "python", "{engine_dir}/tiny-voca/scripts/copyspeech-tiny-voca.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}", "--speed", "{speed}"]
  }
}'

write_engine_manifest "$ENGINE_DIR" "${TINY_IDS[@]}"

echo ""
log C_GREEN "  tiny-voca installed at: $ENGINE_DIR"
write_profile_snippet "$PROFILE_JSON"
