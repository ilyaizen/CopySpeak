#!/usr/bin/env bash
#
# Installs Piper (piper1-gpl) for CopySpeak via uv (Linux port).
#
# Creates a uv-managed project under $XDG_DATA_HOME/CopySpeak/engines/piper,
# installs the `piper` package, and drops the stable CLI wrapper.
#
# Prompts the user to pick an English voice and downloads the matching
# (.onnx + .onnx.json) pair from HuggingFace into the engine's voices/ dir
# (use --skip-voice-download to skip). The chosen voice is used for the smoke
# test and baked into the emitted profile snippet.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeak-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeak-engine-install.sh"

FORCE=0
CUDA=0
SMOKE_TEST=0
SKIP_VOICE_DOWNLOAD=0
VOICES=()

usage() {
    echo "Usage: ./scripts/install-piper.sh [--force] [--cuda] [--smoke-test] [--skip-voice-download] [--voices <id> [<id>...]]"
}

while (( $# > 0 )); do
    case "$1" in
        --force) FORCE=1; shift ;;
        --cuda) CUDA=1; shift ;;
        --smoke-test) SMOKE_TEST=1; shift ;;
        --skip-voice-download) SKIP_VOICE_DOWNLOAD=1; shift ;;
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

write_engine_banner "Piper TTS Installer"

require_uv

# Interactive force prompt: --force bypasses; a blank Enter keeps the install.
# --voices (app-driven) is non-interactive: never destructively reinstall.
if (( FORCE )); then
    EFFECTIVE_FORCE=1
elif (( ${#VOICES[@]} > 0 )); then
    EFFECTIVE_FORCE=0
else
    if get_confirmation "Reinstall Piper from scratch? (deletes the existing engine dir)" 0; then
        EFFECTIVE_FORCE=1
    else
        EFFECTIVE_FORCE=0
    fi
fi

ENGINE_DIR="$(copyspeak_engine_root)/piper"
new_engine_project "$ENGINE_DIR" "$EFFECTIVE_FORCE"

echo ""
log C_GRAY "  Installing Piper..."
# PyPI `piper` is an unrelated bioinformatics toolkit (databio/pypiper,
# module `pypiper`). The TTS engine ships as `piper-tts` (module `piper`).
invoke_uv add --project "$ENGINE_DIR" "piper-tts[alignment]==1.8.0"

SCRIPTS_DIR="$ENGINE_DIR/scripts"
VOICES_DIR="$ENGINE_DIR/voices"
OUTPUT_DIR="$ENGINE_DIR/output"
mkdir -p "$SCRIPTS_DIR" "$VOICES_DIR" "$OUTPUT_DIR"

SRC_WRAPPER="$SCRIPT_DIR/piper/copyspeak-piper.py"
DST_WRAPPER="$SCRIPTS_DIR/copyspeak-piper.py"
cp -f "$SRC_WRAPPER" "$DST_WRAPPER"
log C_GRAY "  Wrapper installed: $DST_WRAPPER"

# Curated English (en_US) voices from rhasspy/piper-voices on HuggingFace.
# The voice Id is also the model basename and the URL segment.
PIPER_IDS=(en_US-amy-medium en_US-lessac-medium en_US-ryan-medium en_US-joe-medium en_US-libritts-medium)
PIPER_LABELS=(
    "Amy (female, medium)"
    "Lessac (female, medium)"
    "Ryan (male, medium)"
    "Joe (male, medium)"
    "LibriTTS (mixed, medium)"
)
VOICE_BASE_URL="https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/en/en_US"

# Resolve the set of voices to install. --voices (app-driven) bypasses the
# interactive menu; otherwise the manual menu picks one. The first id is the
# profile-snippet default.
shopt -s nullglob
EXISTING_MODEL=""
for f in "$VOICES_DIR"/*.onnx; do
    EXISTING_MODEL="$f"
    break
done
shopt -u nullglob

WANTED_VOICES=()
if (( ${#VOICES[@]} > 0 )); then
    WANTED_VOICES=("${VOICES[@]}")
elif (( SKIP_VOICE_DOWNLOAD )) && [[ -n "$EXISTING_MODEL" ]]; then
    WANTED_VOICES=("$(basename "${EXISTING_MODEL%.onnx}")")
else
    WANTED_VOICES=("$(select_voice_from_menu "Pick an English Piper voice" "en_US-amy-medium" \
        "${PIPER_IDS[@]}" "${PIPER_LABELS[@]}")")
fi
CHOSEN_VOICE="${WANTED_VOICES[0]}"

# Download each wanted model pair if missing. Per-voice [STEP]/[DONE]/[ERROR]
# markers let the frontend track per-voice status through the event stream.
# A failed voice must fail the whole run, or the app reports a green install
# for an engine that cannot synthesize.
VOICE_FAILURES=0
for v in "${WANTED_VOICES[@]}"; do
    MODEL_PATH="$VOICES_DIR/$v.onnx"
    CONFIG_PATH="$VOICES_DIR/$v.onnx.json"
    if [[ -f "$MODEL_PATH" && -f "$CONFIG_PATH" ]]; then
        log C_GREEN "  [DONE] voice:$v (already present)"
        continue
    fi
    if (( SKIP_VOICE_DOWNLOAD )); then
        log C_RED "  [ERROR] voice:$v (skipped, --skip-voice-download)"
        VOICE_FAILURES=$((VOICE_FAILURES + 1))
        continue
    fi
    log C_YELLOW "  [STEP] voice:$v"
    # The voice Id is <locale>-<name>-<quality>, split on "-"; use segments 2
    # and 3 exactly like the PowerShell original (Count -ge 3).
    IFS='-' read -r -a PARTS <<< "$v"
    if (( ${#PARTS[@]} >= 3 )); then
        VOICE_NAME="${PARTS[1]}"
        QUALITY="${PARTS[2]}"
        ONNX_URL="$VOICE_BASE_URL/$VOICE_NAME/$QUALITY/$v.onnx"
        JSON_URL="$VOICE_BASE_URL/$VOICE_NAME/$QUALITY/$v.onnx.json"
        log C_GRAY "    -> $MODEL_PATH"
        DL_OK=1
        if [[ ! -f "$MODEL_PATH" ]]; then
            if ! curl -fsSL --retry 3 -o "$MODEL_PATH" "$ONNX_URL"; then
                DL_OK=0
            fi
        fi
        if (( DL_OK )) && [[ ! -f "$CONFIG_PATH" ]]; then
            if ! curl -fsSL --retry 3 -o "$CONFIG_PATH" "$JSON_URL"; then
                DL_OK=0
            fi
        fi
        if (( DL_OK )); then
            log C_GREEN "  [DONE] voice:$v"
        else
            # A half-written .onnx would look "present" on the next run and
            # then fail at synthesis time, so clear the partial download.
            rm -f "$MODEL_PATH" "$CONFIG_PATH"
            log C_RED "  [ERROR] voice:$v : download failed ($ONNX_URL)"
            VOICE_FAILURES=$((VOICE_FAILURES + 1))
        fi
    else
        log C_RED "  [ERROR] voice:$v (unrecognized id shape)"
        VOICE_FAILURES=$((VOICE_FAILURES + 1))
    fi
done

echo ""
log C_GRAY "  Voices directory: $VOICES_DIR"
log C_GRAY "  More voices:      https://github.com/OHF-Voice/piper1-gpl#voices"

if (( CUDA )); then
    echo ""
    if add_cuda_runtime "$ENGINE_DIR" onnx; then
        test_cuda_synthesis "$ENGINE_DIR" "$DST_WRAPPER" "$CHOSEN_VOICE" || true
    fi
fi

if (( SMOKE_TEST )); then
    SMOKE_MODEL="$VOICES_DIR/$CHOSEN_VOICE.onnx"
    if [[ ! -f "$SMOKE_MODEL" ]]; then
        log C_YELLOW "  Smoke test skipped: no .onnx model in $VOICES_DIR"
    else
        TEST_OUT="$OUTPUT_DIR/test.wav"
        log C_YELLOW "  Running smoke test with voice '$CHOSEN_VOICE'..."
        invoke_uv run --project "$ENGINE_DIR" python "$DST_WRAPPER" \
            --text "Hello from Piper" --voice "$CHOSEN_VOICE" --output "$TEST_OUT"
        if ! test_audio_file "$TEST_OUT"; then
            log C_RED "  Smoke test FAILED."
            exit 1
        fi
    fi
fi

PROFILE_JSON='{
  "schema_version": 1,
  "id": "piper-local",
  "name": "Piper (Local)",
  "engine": "local",
  "voice": "'"$CHOSEN_VOICE"'",
  "speed": 1.0,
  "pitch": 1.0,
  "effects": { "enabled": false, "active_effect": "none" },
  "engine_options": {
    "engine": "local",
    "preset": "piper",
    "command": "uv",
    "args_template": ["run", "--project", "{engine_dir}/piper", "python", "{engine_dir}/piper/scripts/copyspeak-piper.py", "--text-file", "{input}", "--voice", "{voice}", "--output", "{output}"]
  }
}'

# Record installed voices (source of truth = present .onnx basenames) so the
# frontend can pre-check the "add a voice later" dialog.
INSTALLED_VOICES=()
for f in "$VOICES_DIR"/*.onnx; do
    INSTALLED_VOICES+=("$(basename "${f%.onnx}")")
done
write_engine_manifest "$ENGINE_DIR" "${INSTALLED_VOICES[@]}"

echo ""
if (( VOICE_FAILURES > 0 )); then
    log C_RED "  [ERROR] engine ($VOICE_FAILURES voice download(s) failed)"
    log C_YELLOW "  Piper package is installed at $ENGINE_DIR, but retry the failed voices."
    exit 1
fi
log C_GREEN "  [DONE] engine"
log C_GREEN "  Piper installed at: $ENGINE_DIR"
write_profile_snippet "$PROFILE_JSON"
