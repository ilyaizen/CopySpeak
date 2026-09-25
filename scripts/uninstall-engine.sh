#!/usr/bin/env bash
#
# Uninstalls a CopySpeak local TTS engine (Linux port).
#
# The mirror of install-*.sh and the bash twin of uninstall-engine.ps1. Every
# local engine leaves at most two traces, so one script covers all of them:
#
#   * a uv-managed project dir under $XDG_DATA_HOME/CopySpeak/engines/<name>
#     (kitten, qwen, piper, kokoro, pocket)
#   * a `uv tool` install putting a binary on PATH (edge-tts; the Linux
#     installers use project installs, so this only matters for engines
#     installed by hand)
#
# Emits the same [STEP]/[DONE]/[ERROR] markers as the installers so the app
# can stream progress through the shared install-progress event pipe.
#
# uv itself is deliberately NOT uninstallable from here: it is a shared
# prerequisite that other engines (and possibly other apps) depend on.
#
# Usage: ./scripts/uninstall-engine.sh --engine <name> [--keep-models]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeak-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeak-engine-install.sh"

ENGINE=""
KEEP_MODELS=0

while (( $# > 0 )); do
    case "$1" in
        --engine) ENGINE="${2:?}"; shift 2 ;;
        --engine=*) ENGINE="${1#*=}"; shift ;;
        --keep-models) KEEP_MODELS=1; shift ;;
        -h|--help)
            echo "Usage: ./scripts/uninstall-engine.sh --engine <kitten|qwen|piper|kokoro|pocket|edge> [--keep-models]"
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done

# Dir is relative to the CopySpeak engine root; Tool is the `uv tool` name
# (engines installed as project deps have none). Mirrors the ps1 layout table.
case "$ENGINE" in
    kitten) TITLE="Kitten TTS"; DIR="kitten"; TOOL="" ;;
    qwen)   TITLE="Qwen3-TTS"; DIR="qwen";   TOOL="" ;;
    piper)  TITLE="Piper TTS"; DIR="piper";  TOOL="" ;;
    kokoro) TITLE="Kokoro TTS"; DIR="kokoro"; TOOL="kokoro-tts" ;;
    pocket) TITLE="Pocket TTS"; DIR="";      TOOL="pocket-tts" ;;
    edge)   TITLE="Edge-TTS";  DIR="";      TOOL="edge-tts" ;;
    *) echo "Unknown engine: $ENGINE" >&2; exit 2 ;;
esac

write_engine_banner "$TITLE Uninstaller"

FAILED=0

# -- uv tool -----------------------------------------------------------------
if [[ -n "$TOOL" ]]; then
    log C_YELLOW "  [STEP] tool"
    if ! command -v uv >/dev/null 2>&1; then
        # No uv means nothing was installed through it; not an error.
        log C_GRAY "  uv not on PATH; nothing to uninstall."
        log C_GREEN "  [DONE] tool"
    else
        log C_GRAY "  > uv tool uninstall $TOOL"
        # Indent uv's output to match the installers' step log style. uv exits
        # non-zero when the tool was never installed - that is a success for
        # us, so check the binary instead of the exit code.
        uv tool uninstall "$TOOL" 2>&1 | sed 's/^/    /' || true
        if command -v "$TOOL" >/dev/null 2>&1; then
            log C_RED "  [ERROR] tool: $TOOL is still on PATH after uninstall"
            FAILED=1
        else
            log C_GREEN "  [DONE] tool"
        fi
    fi
fi

# -- engine directory --------------------------------------------------------
if [[ -n "$DIR" ]]; then
    ENGINE_DIR="$(copyspeak_engine_root)/$DIR"
    log C_YELLOW "  [STEP] files"
    if [[ ! -e "$ENGINE_DIR" ]]; then
        log C_GRAY "  Nothing at $ENGINE_DIR."
        log C_GREEN "  [DONE] files"
    elif (( KEEP_MODELS )); then
        # Drop the venv/wrapper/manifest but keep the expensive downloads.
        log C_GRAY "  Keeping model files; removing project files in $ENGINE_DIR"
        for leaf in .venv scripts output pyproject.toml uv.lock manifest.json; do
            rm -rf "$ENGINE_DIR/$leaf"
        done
        log C_GREEN "  [DONE] files"
    else
        log C_GRAY "  Removing $ENGINE_DIR"
        if rm -rf "$ENGINE_DIR"; then
            log C_GREEN "  [DONE] files"
        else
            # Almost always a running python holding the venv open.
            log C_RED "  [ERROR] files: rm -rf $ENGINE_DIR failed"
            log C_YELLOW "  Close anything using the engine (a playing clip) and retry."
            FAILED=1
        fi
    fi
fi

echo ""
if (( FAILED )); then
    log C_RED "  $TITLE uninstall incomplete."
    exit 1
fi
log C_GREEN "  $TITLE uninstalled."
log C_GRAY "  Remove any CopySpeak profile that pointed at it from the Voices page."
