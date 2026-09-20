#!/usr/bin/env bash
#
# Installs uv (the Python package/environment manager) for CopySpeak local engines.
#
# uv is a hard requirement for all CopySpeak Python-based TTS engines. This is
# the ONLY installer allowed to run when uv is missing; every other engine
# installer fails fast and points here.
#
# On Linux there is no winget; install through the official Astral standalone
# installer (https://astral.sh/uv/install.sh), which drops a self-contained
# binary into ~/.local/bin.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/copyspeak-engine-install.sh
source "$SCRIPT_DIR/lib/copyspeak-engine-install.sh"

FORCE=0
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=1 ;;
        -h|--help)
            echo "Usage: ./scripts/install-uv.sh [--force]"
            echo "  --force   Reinstall even if uv is already present."
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            echo "Usage: ./scripts/install-uv.sh [--force]" >&2
            exit 2
            ;;
    esac
done

echo ""
log C_MAGENTA "  ========================================"
log C_MAGENTA "  |  uv Installer for CopySpeak          |"
log C_MAGENTA "  ========================================"
echo ""

# uv may already be on disk but missing from this shell's PATH snapshot.
if ! command -v uv >/dev/null 2>&1; then
    add_uv_to_path
fi

if command -v uv >/dev/null 2>&1 && (( ! FORCE )); then
    log C_GREEN "  uv already installed: $(uv --version)"
    log C_YELLOW "  Use --force to reinstall."
    exit 0
fi

log C_GRAY "  Installing uv via official Astral script..."
log C_DGRAY "  > curl -LsSf https://astral.sh/uv/install.sh | sh"
curl -LsSf https://astral.sh/uv/install.sh | sh

# The installer drops uv into ~/.local/bin without touching this shell's PATH.
add_uv_to_path

if command -v uv >/dev/null 2>&1; then
    log C_GREEN "  uv installed: $(uv --version)"
else
    log C_YELLOW "  uv installed. Restart your terminal so it appears on PATH."
fi
