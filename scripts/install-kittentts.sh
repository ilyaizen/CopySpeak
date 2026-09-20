#!/usr/bin/env bash
#
# Installs KittenTTS for CopySpeak via uv (CPU-only, 25-80MB; Linux port).
#
# Creates a uv-managed project under $XDG_DATA_HOME/CopySpeak/engines/kitten,
# installs the KittenTTS wheel + soundfile, and drops the stable CLI wrapper.
# The model auto-downloads on first synthesis. Prompts the user to pick one
# of the 8 built-in voices, which is baked into the profile snippet.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeak-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeak-engine-install.sh"

FORCE=0
CUDA=0
SMOKE_TEST=0
VOICES=()

usage() {
    echo "Usage: ./scripts/install-kittentts.sh [--force] [--cuda] [--smoke-test] [--voices <id> [<id>...]]"
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

write_engine_banner "Kitten TTS Installer"

require_uv

# Interactive force prompt: --force bypasses; a blank Enter keeps the install.
# --voices (app-driven) is non-interactive: never destructively reinstall.
if (( FORCE )); then
    EFFECTIVE_FORCE=1
elif (( ${#VOICES[@]} > 0 )); then
    EFFECTIVE_FORCE=0
else
    if get_confirmation "Reinstall KittenTTS from scratch? (deletes the existing engine dir)" 0; then
        EFFECTIVE_FORCE=1
    else
        EFFECTIVE_FORCE=0
    fi
fi

ENGINE_DIR="$(copyspeak_engine_root)/kitten"
new_engine_project "$ENGINE_DIR" "$EFFECTIVE_FORCE"

# KittenTTS is published as a GitHub release wheel (not on PyPI).
# Pin the URL; bump here when KittenML cuts a new release.
WHEEL_URL="https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl"

echo ""
log C_YELLOW "  [STEP] engine"
log C_GRAY "  Installing KittenTTS + soundfile..."
invoke_uv add --project "$ENGINE_DIR" "$WHEEL_URL" "soundfile"

SCRIPTS_DIR="$ENGINE_DIR/scripts"
VOICES_DIR="$ENGINE_DIR/voices"
OUTPUT_DIR="$ENGINE_DIR/output"
mkdir -p "$SCRIPTS_DIR" "$VOICES_DIR" "$OUTPUT_DIR"

SRC_WRAPPER="$SCRIPT_DIR/kitten/copyspeak-kitten.py"
DST_WRAPPER="$SCRIPTS_DIR/copyspeak-kitten.py"
cp -f "$SRC_WRAPPER" "$DST_WRAPPER"
log C_GRAY "  Wrapper installed: $DST_WRAPPER"
log C_GREEN "  [DONE] engine"

# KittenTTS ships 8 built-in English voices; the model auto-downloads on
# first synth. Voice ids are case-sensitive (capitalized first letter).
KITTEN_IDS=(Rosie Bella Luna Kiki Jasper Bruno Hugo Leo)
KITTEN_LABELS=(
    "Rosie (female)"
    "Bella (female)"
    "Luna (female)"
    "Kiki (female)"
    "Jasper (male)"
    "Bruno (male)"
    "Hugo (male)"
    "Leo (male)"
)
# All 8 voices ship with the shared model, so the manifest lists every voice
# once the package is installed (the model auto-downloads on first synth).
if (( ${#VOICES[@]} > 0 )); then
    CHOSEN_VOICE="${VOICES[0]}"
else
    CHOSEN_VOICE="$(select_voice_from_menu "Pick a default KittenTTS voice" "Rosie" \
        "${KITTEN_IDS[@]}" "${KITTEN_LABELS[@]}")"
fi

if (( CUDA )); then
    echo ""
    if add_cuda_runtime "$ENGINE_DIR" onnx; then
        test_cuda_synthesis "$ENGINE_DIR" "$DST_WRAPPER" "$CHOSEN_VOICE" || true
    fi
fi

if (( SMOKE_TEST )); then
    echo ""
    log C_YELLOW "  Running smoke test (first run downloads the model)..."
    TEST_OUT="$OUTPUT_DIR/test.wav"
    invoke_uv run --project "$ENGINE_DIR" python "$DST_WRAPPER" \
        --text "Hello from Kitten TTS" --voice "$CHOSEN_VOICE" --output "$TEST_OUT"
    if ! test_audio_file "$TEST_OUT"; then
        log C_RED "  Smoke test FAILED."
        exit 1
    fi
fi

PROFILE_JSON='{
  "schema_version": 1,
  "id": "kitten-local",
  "name": "Kitten TTS (Local)",
  "engine": "local",
  "voice": "'"$CHOSEN_VOICE"'",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "kitten-tts",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/kitten", "python", "{engine_dir}/kitten/scripts/copyspeak-kitten.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}", "--model", "{model}", "--speed", "{speed}"]
  }
}'

write_engine_manifest "$ENGINE_DIR" "${KITTEN_IDS[@]}"

echo ""
log C_GREEN "  Kitten TTS installed at: $ENGINE_DIR"
write_profile_snippet "$PROFILE_JSON"
