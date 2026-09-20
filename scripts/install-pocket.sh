#!/usr/bin/env bash
#
# Installs Kyutai Pocket TTS for CopySpeak via uv (Linux port).
#
# Creates a uv-managed project under $XDG_DATA_HOME/CopySpeak/engines/pocket,
# installs pocket-tts, and drops the stable CLI wrapper. Model weights
# auto-download from Hugging Face on first synthesis.
#
# The `pocket-tts generate` CLI this used to install reloaded the model and
# re-derived the voice state on every invocation — the two slow steps. Talking
# to the Python API through our own wrapper keeps both resident.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeak-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeak-engine-install.sh"

FORCE=0
CUDA=0
SMOKE_TEST=0
VOICES=()

usage() {
    echo "Usage: ./scripts/install-pocket.sh [--force] [--cuda] [--smoke-test] [--voices <id> [<id>...]]"
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

write_engine_banner "Pocket TTS Installer"

require_uv

# Interactive force prompt: --force bypasses; a blank Enter keeps the install.
# --voices (app-driven) is non-interactive: never destructively reinstall.
if (( FORCE )); then
    EFFECTIVE_FORCE=1
elif (( ${#VOICES[@]} > 0 )); then
    EFFECTIVE_FORCE=0
else
    if get_confirmation "Reinstall Pocket from scratch? (deletes the existing engine dir)" 0; then
        EFFECTIVE_FORCE=1
    else
        EFFECTIVE_FORCE=0
    fi
fi

ENGINE_DIR="$(copyspeak_engine_root)/pocket"
new_engine_project "$ENGINE_DIR" "$EFFECTIVE_FORCE"

echo ""
log C_YELLOW "  [STEP] engine"
log C_GRAY "  Installing pocket-tts..."
# Linux PyPI torch wheels are the CUDA build, so no extra index is needed
# here; --cuda swaps in pinned CUDA builds below.
invoke_uv add --project "$ENGINE_DIR" "pocket-tts"

SCRIPTS_DIR="$ENGINE_DIR/scripts"
OUTPUT_DIR="$ENGINE_DIR/output"
mkdir -p "$SCRIPTS_DIR" "$OUTPUT_DIR"

SRC_WRAPPER="$SCRIPT_DIR/pocket/copyspeak-pocket.py"
DST_WRAPPER="$SCRIPTS_DIR/copyspeak-pocket.py"
cp -f "$SRC_WRAPPER" "$DST_WRAPPER"
log C_GRAY "  Wrapper installed: $DST_WRAPPER"
log C_GREEN "  [DONE] engine"

# Kyutai's built-in voice catalog. Weights (and the voice prompts) download
# from Hugging Face on first synthesis.
POCKET_IDS=(alba anna eve jane mary charles george michael paul estelle lola giovanni juergen rafael)
POCKET_LABELS=(
    "Alba (English, female)"
    "Anna (English, female)"
    "Eve (English, female)"
    "Jane (English, female)"
    "Mary (English, female)"
    "Charles (English, male)"
    "George (English, male)"
    "Michael (English, male)"
    "Paul (English, male)"
    "Estelle (French, female)"
    "Lola (Spanish, female)"
    "Giovanni (Italian, male)"
    "Juergen (German, male)"
    "Rafael (Portuguese, male)"
)
if (( ${#VOICES[@]} > 0 )); then
    CHOSEN_VOICE="${VOICES[0]}"
else
    CHOSEN_VOICE="$(select_voice_from_menu "Pick a default Pocket voice" "alba" \
        "${POCKET_IDS[@]}" "${POCKET_LABELS[@]}")"
fi

if (( CUDA )); then
    echo ""
    if add_cuda_runtime "$ENGINE_DIR" torch; then
        test_cuda_synthesis "$ENGINE_DIR" "$DST_WRAPPER" "$CHOSEN_VOICE" || true
    fi
fi

if (( SMOKE_TEST )); then
    echo ""
    log C_YELLOW "  [STEP] smoke"
    log C_YELLOW "  Running smoke test (first run downloads the weights)..."
    TEST_OUT="$OUTPUT_DIR/test.wav"
    invoke_uv run --project "$ENGINE_DIR" python "$DST_WRAPPER" \
        --text "Hello from Pocket TTS" --voice "$CHOSEN_VOICE" --output "$TEST_OUT"
    if ! test_audio_file "$TEST_OUT"; then
        log C_RED "  [ERROR] smoke"
        exit 1
    fi
    log C_GREEN "  [DONE] smoke"
fi

PROFILE_JSON='{
  "schema_version": 1,
  "id": "pocket-local",
  "name": "Pocket (Local)",
  "engine": "pocket",
  "voice": "'"$CHOSEN_VOICE"'",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": { "engine": "pocket", "cuda": false }
}'

write_engine_manifest "$ENGINE_DIR" "${POCKET_IDS[@]}"

echo ""
log C_GREEN "  Pocket TTS installed at: $ENGINE_DIR"
write_profile_snippet "$PROFILE_JSON"
