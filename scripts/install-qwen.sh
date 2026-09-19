#!/usr/bin/env bash
#
# Installs Qwen3-TTS CustomVoice for CopySpeak via uv (Linux port).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeak-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeak-engine-install.sh"

FORCE=0
CUDA=0
SMOKE_TEST=0
VOICES=()

usage() {
    echo "Usage: ./scripts/install-qwen.sh [--force] [--cuda] [--smoke-test] [--voices <id> [<id>...]]"
}

while (( $# > 0 )); do
    case "$1" in
        --force) FORCE=1; shift ;;
        --cuda) CUDA=1; shift ;;
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

write_engine_banner "Qwen3-TTS Installer"

require_uv

# Interactive force prompt: --force bypasses; a blank Enter keeps the install.
# --voices (app-driven) is non-interactive: never destructively reinstall.
if (( FORCE )); then
    EFFECTIVE_FORCE=1
elif (( ${#VOICES[@]} > 0 )); then
    EFFECTIVE_FORCE=0
else
    if get_confirmation "Reinstall Qwen3-TTS from scratch? (deletes the existing engine dir)" 0; then
        EFFECTIVE_FORCE=1
    else
        EFFECTIVE_FORCE=0
    fi
fi

ENGINE_DIR="$(copyspeak_engine_root)/qwen"
new_engine_project "$ENGINE_DIR" "$EFFECTIVE_FORCE"

echo ""
log C_YELLOW "  [STEP] engine"
log C_GRAY "  Installing qwen-tts + soundfile..."
invoke_uv add --project "$ENGINE_DIR" "qwen-tts" "soundfile"

SCRIPTS_DIR="$ENGINE_DIR/scripts"
OUTPUT_DIR="$ENGINE_DIR/output"
mkdir -p "$SCRIPTS_DIR" "$OUTPUT_DIR"
SRC_WRAPPER="$SCRIPT_DIR/qwen/copyspeak-qwen.py"
DST_WRAPPER="$SCRIPTS_DIR/copyspeak-qwen.py"
cp -f "$SRC_WRAPPER" "$DST_WRAPPER"
log C_GRAY "  Wrapper installed: $DST_WRAPPER"
log C_GREEN "  [DONE] engine"

# Qwen3-TTS CustomVoice catalog.
QWEN_IDS=(Aiden Ryan Vivian Serena Uncle_Fu Dylan Eric Ono_Anna Sohee)
QWEN_LABELS=(
    "Aiden (English male)"
    "Ryan (English male)"
    "Vivian (Chinese female)"
    "Serena (Chinese female)"
    "Uncle Fu (Chinese male)"
    "Dylan (Beijing male)"
    "Eric (Chengdu male)"
    "Ono Anna (Japanese female)"
    "Sohee (Korean female)"
)

if (( ${#VOICES[@]} > 0 )); then
    CHOSEN_VOICE="${VOICES[0]}"
else
    CHOSEN_VOICE="$(select_voice_from_menu "Pick a default Qwen3-TTS voice" "Aiden" \
        "${QWEN_IDS[@]}" "${QWEN_LABELS[@]}")"
fi

if (( CUDA )); then
    echo ""
    if add_cuda_runtime "$ENGINE_DIR" torch; then
        test_cuda_synthesis "$ENGINE_DIR" "$DST_WRAPPER" "$CHOSEN_VOICE" || true
    fi
fi

if (( SMOKE_TEST )); then
    echo ""
    log C_YELLOW "  Running smoke test (first run downloads the model)..."
    TEST_OUT="$OUTPUT_DIR/test.wav"
    invoke_uv run --project "$ENGINE_DIR" python "$DST_WRAPPER" \
        --text "Hello from Qwen3 TTS" --voice "$CHOSEN_VOICE" --output "$TEST_OUT"
    if ! test_audio_file "$TEST_OUT"; then
        log C_RED "  Smoke test FAILED."
        exit 1
    fi
fi

write_engine_manifest "$ENGINE_DIR" "${QWEN_IDS[@]}"
echo ""
log C_GREEN "  Qwen3-TTS installed at: $ENGINE_DIR"
