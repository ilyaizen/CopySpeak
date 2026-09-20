#!/usr/bin/env bash
#
# Installs Kokoro TTS for CopySpeak via uv (Linux port).
#
# Creates a uv-managed project under $XDG_DATA_HOME/CopySpeak/engines/kokoro,
# installs kokoro-onnx and Misaki English, drops the CLI wrapper, and exports
# a duration-capable ONNX model using the pinned upstream exporter. The first
# installation also downloads the Kokoro checkpoint and export dependencies.
#
# The third-party `kokoro-tts` CLI this used to install reloaded the 310 MB
# model on every invocation and gave no way to pick an execution provider, so
# neither the resident daemon nor GPU inference could reach it. Talking to
# kokoro-onnx through our own wrapper gets both.
#
# Prompts the user to pick a default English voice, baked into the profile
# snippet.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeak-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeak-engine-install.sh"

FORCE=0
CUDA=0
SMOKE_TEST=0
SKIP_MODEL_DOWNLOAD=0
VOICES=()

usage() {
    echo "Usage: ./scripts/install-kokoro.sh [--force] [--cuda] [--smoke-test] [--skip-model-download] [--voices <id> [<id>...]]"
}

while (( $# > 0 )); do
    case "$1" in
        --force) FORCE=1; shift ;;
        --cuda) CUDA=1; shift ;;
        --smoke-test) SMOKE_TEST=1; shift ;;
        --skip-model-download) SKIP_MODEL_DOWNLOAD=1; shift ;;
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

# Download <url> to <dest> through a ".download" partial file, so a crashed
# fetch never leaves a half-written file at the real path.
fetch_to() {
    local url=$1
    local dest=$2
    if [[ -f "$dest" ]]; then
        return 0
    fi
    local partial="$dest.download"
    if ! curl -fsSL --retry 3 -o "$partial" "$url"; then
        rm -f "$partial"
        return 1
    fi
    mv -f "$partial" "$dest"
}

write_engine_banner "Kokoro TTS Installer"

require_uv

# Interactive force prompt: --force bypasses; a blank Enter keeps the install.
# --voices (app-driven) is non-interactive: never destructively reinstall.
if (( FORCE )); then
    EFFECTIVE_FORCE=1
elif (( ${#VOICES[@]} > 0 )); then
    EFFECTIVE_FORCE=0
else
    if get_confirmation "Reinstall Kokoro from scratch? (deletes the existing engine dir)" 0; then
        EFFECTIVE_FORCE=1
    else
        EFFECTIVE_FORCE=0
    fi
fi

ENGINE_DIR="$(copyspeak_engine_root)/kokoro"
# models/ sits inside the engine dir, so --force would re-download 335 MB.
# Stash the model files across the reset and put them back.
MODELS_DIR="$ENGINE_DIR/models"
STASH=""
if (( EFFECTIVE_FORCE )) && [[ -e "$MODELS_DIR" ]]; then
    STASH="$(mktemp -d)/kokoro-models"
    mv "$MODELS_DIR" "$STASH"
fi
new_engine_project "$ENGINE_DIR" "$EFFECTIVE_FORCE"
if [[ -n "$STASH" ]]; then
    mv "$STASH" "$MODELS_DIR"
    log C_GRAY "  Kept existing model files (no re-download)."
fi

echo ""
log C_YELLOW "  [STEP] engine"
ENGINE_OK=1
log C_GRAY "  Installing Kokoro and its English caption frontend..."
if ! invoke_uv add --project "$ENGINE_DIR" "kokoro-onnx==0.6.1" "misaki[en]==0.9.4" \
    "en-core-web-sm @ https://github.com/explosion/spacy-models/releases/download/en_core_web_sm-3.8.0/en_core_web_sm-3.8.0-py3-none-any.whl"; then
    log C_RED "  [ERROR] engine (uv add failed)"
    ENGINE_OK=0
fi

SCRIPTS_DIR="$ENGINE_DIR/scripts"
OUTPUT_DIR="$ENGINE_DIR/output"
mkdir -p "$SCRIPTS_DIR" "$OUTPUT_DIR" "$MODELS_DIR"

SRC_WRAPPER="$SCRIPT_DIR/kokoro/copyspeak-kokoro.py"
DST_WRAPPER="$SCRIPTS_DIR/copyspeak-kokoro.py"
cp -f "$SRC_WRAPPER" "$DST_WRAPPER"
log C_GRAY "  Wrapper installed: $DST_WRAPPER"
if (( ENGINE_OK )); then
    log C_GREEN "  [DONE] engine"
fi

# Model files. kokoro-onnx requires these and does not auto-download them.
# Stable home is <engine_dir>/models/ so the wrapper resolves them relative to
# itself, the same way piper resolves its voices/.
MODEL_FILE="$MODELS_DIR/kokoro-v1.0-duration.onnx"
VOICES_FILE="$MODELS_DIR/voices-v1.0.bin"
VOICES_URL="https://github.com/nazdridoy/kokoro-tts/releases/download/v1.0.0/voices-v1.0.bin"

if (( ! SKIP_MODEL_DOWNLOAD )); then
    log C_YELLOW "  [STEP] model"
    MODEL_OK=1
    if [[ -f "$VOICES_FILE" ]]; then
        log C_GREEN "  Already present: $VOICES_FILE"
    else
        echo ""
        log C_YELLOW "  Downloading voices-v1.0.bin (~25 MB)..."
        log C_GRAY "    -> $VOICES_FILE"
        if ! curl -fsSL --retry 3 -o "$VOICES_FILE" "$VOICES_URL"; then
            # Drop the partial file; otherwise the next run sees it as present.
            rm -f "$VOICES_FILE"
            log C_RED "  WARNING: download failed"
            log C_GRAY "  Re-run with --force, or download manually from $VOICES_URL"
            MODEL_OK=0
        fi
    fi
    if (( ENGINE_OK )) && [[ ! -f "$MODEL_FILE" ]]; then
        # Keep the old audio-only model intact. Promote the new artifact only
        # after upstream's export and native-duration verification both succeed.
        EXPORT_DIR="$MODELS_DIR/native-export"
        mkdir -p "$EXPORT_DIR"
        EXPORT_SCRIPT="$EXPORT_DIR/export.py"
        EXPORT_CONFIG="$EXPORT_DIR/config.json"
        CHECKPOINT="$EXPORT_DIR/kokoro-v1_0.pth"
        PENDING_MODEL="$EXPORT_DIR/kokoro-v1.0-duration.onnx"
        CHECKPOINT_BASE="https://huggingface.co/hexgrad/Kokoro-82M/resolve/f3ff3571791e39611d31c381e3a41a3af07b4987"
        EXPORT_OK=1
        if fetch_to "https://raw.githubusercontent.com/thewh1teagle/kokoro-onnx/3596b26764286a7de9d90c363e988d50578918e5/scripts/export.py" "$EXPORT_SCRIPT" \
            && fetch_to "$CHECKPOINT_BASE/config.json" "$EXPORT_CONFIG" \
            && fetch_to "$CHECKPOINT_BASE/kokoro-v1_0.pth" "$CHECKPOINT"; then
            # Kokoro's export package requires NumPy 1.x, which has no Python
            # 3.13 wheel. Keep this build environment separate from inference
            # (--no-project + a managed 3.12 interpreter).
            # Pass exporter options by their long names (--config/--checkpoint/
            # --output); short flags are ambiguous in some tool wrappers.
            if invoke_uv run --no-project --python 3.12 \
                --with "onnx==1.22.0" --with "onnxscript==0.7.2" \
                --with "onnxruntime==1.29.0" --with "torch==2.14.0" \
                --with "transformers==5.17.0" \
                "$EXPORT_SCRIPT" --config "$EXPORT_CONFIG" --checkpoint "$CHECKPOINT" --output "$PENDING_MODEL"; then
                mv -f "$PENDING_MODEL" "$MODEL_FILE"
            else
                EXPORT_OK=0
            fi
        else
            EXPORT_OK=0
        fi
        if (( ! EXPORT_OK )); then
            log C_RED "  [ERROR] native caption model export (download or export step failed)"
            MODEL_OK=0
        fi
    fi
    if (( MODEL_OK )) && [[ -f "$MODEL_FILE" && -f "$VOICES_FILE" ]]; then
        log C_GREEN "  [DONE] model"
    else
        log C_RED "  [ERROR] model (one or more files missing)"
    fi
fi

# Kokoro ships many built-in voices; en_* voice ids: af_* = American female,
# am_* = American male, bf_* = British female, bm_* = British male.
KOKORO_IDS=(af_heart af_bella af_nicole af_sarah am_adam am_michael bf_emma bm_george)
KOKORO_LABELS=(
    "Heart (American female, flagship)"
    "Bella (American female)"
    "Nicole (American female)"
    "Sarah (American female)"
    "Adam (American male)"
    "Michael (American male)"
    "Emma (British female)"
    "George (British male)"
)
# All listed voices share the one model file; the default is just the
# profile-snippet pick.
if (( ${#VOICES[@]} > 0 )); then
    CHOSEN_VOICE="${VOICES[0]}"
else
    CHOSEN_VOICE="$(select_voice_from_menu "Pick a default Kokoro voice" "af_heart" \
        "${KOKORO_IDS[@]}" "${KOKORO_LABELS[@]}")"
fi

KOKORO_OK=0
if (( ENGINE_OK )) && [[ -f "$MODEL_FILE" && -f "$VOICES_FILE" ]]; then
    KOKORO_OK=1
fi

if (( CUDA && KOKORO_OK )); then
    echo ""
    if add_cuda_runtime "$ENGINE_DIR" onnx; then
        test_cuda_synthesis "$ENGINE_DIR" "$DST_WRAPPER" "$CHOSEN_VOICE" || true
    fi
fi

if (( SMOKE_TEST && KOKORO_OK )); then
    echo ""
    log C_YELLOW "  [STEP] smoke"
    TEST_OUT="$OUTPUT_DIR/test.wav"
    invoke_uv run --project "$ENGINE_DIR" python "$DST_WRAPPER" \
        --text "Hello from Kokoro" --voice "$CHOSEN_VOICE" --output "$TEST_OUT"
    if ! test_audio_file "$TEST_OUT"; then
        log C_RED "  [ERROR] smoke"
        exit 1
    fi
    log C_GREEN "  [DONE] smoke"
fi

PROFILE_JSON='{
  "schema_version": 1,
  "id": "kokoro-local",
  "name": "Kokoro (Local)",
  "engine": "kokoro",
  "voice": "'"$CHOSEN_VOICE"'",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": { "engine": "kokoro", "cuda": false }
}'

if (( KOKORO_OK )); then
    write_engine_manifest "$ENGINE_DIR" "${KOKORO_IDS[@]}"
fi

echo ""
if (( KOKORO_OK )); then
    log C_GREEN "  Kokoro installed at: $ENGINE_DIR"
    write_profile_snippet "$PROFILE_JSON"
else
    log C_RED "  [ERROR] engine (kokoro-onnx or model files missing - re-run without --skip-model-download)"
    exit 1
fi
