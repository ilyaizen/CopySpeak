#!/usr/bin/env bash
#
# Shared helpers for CopySpeak uv-based engine installers (Linux port).
#
# PROGRESS MARKERS
# ----------------
# Installers emit three machine-parseable line prefixes on stdout so the
# CopySpeak frontend can track state transitions through the streamed event
# pipe (everything else is free-form log noise):
#     [STEP] <name>   - a phase started (name = 'voice:<id>' for per-voice
#                       Piper downloads, or 'engine'/'model'/'cuda'/
#                       'cuda-check'/'manifest' for shared steps)
#     [DONE] <name>   - a phase completed
#     [ERROR] <name>  - a phase failed (trailing ': <detail>' optional)
#
# NONINTERACTIVE MODE
# -------------------
# COPYSPEAK_NONINTERACTIVE=1 means the app (not a human) is driving: the app
# spawns installers with stdin closed, so any `read` would fail or block
# forever. Every interactive helper below silently takes its default in that
# mode.
#
# Source this file from an engine installer:
#     source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/copyspeak-engine-install.sh"
#
# All Python engines are managed by `uv`. These helpers stay deliberately
# boring: require uv, locate the engine root, create a uv project, run uv,
# validate audio output, and print a CopySpeak profile snippet.

# ANSI color helpers. Only colorize when stdout is a real terminal; the app
# parses stdout, and ANSI codes in the event pipe would be pure noise.
# The colors are read only through log()'s indirect ${!color_name}, which the
# linter cannot see - hence the SC2034 suppressions below.
if [[ -t 1 ]]; then
    # shellcheck disable=SC2034  # read via log()'s indirect ${!color_name}
    C_RESET=$'\033[0m' C_RED=$'\033[31m' C_GREEN=$'\033[32m' C_YELLOW=$'\033[33m'
    # shellcheck disable=SC2034  # read via log()'s indirect ${!color_name}
    C_MAGENTA=$'\033[35m' C_CYAN=$'\033[36m' C_GRAY=$'\033[90m' C_DGRAY=$'\033[90m'
else
    # shellcheck disable=SC2034  # read via log()'s indirect ${!color_name}
    C_RESET="" C_RED="" C_GREEN="" C_YELLOW=""
    # shellcheck disable=SC2034  # read via log()'s indirect ${!color_name}
    C_MAGENTA="" C_CYAN="" C_GRAY="" C_DGRAY=""
fi

# log <color-var-name> <text...>
# Print text prefixed with the named color variable (C_GRAY, C_GREEN, ...).
# No-op coloring when stdout is not a tty.
log() {
    local color_name=$1
    shift
    local color="${!color_name}"
    printf '%s%s%s\n' "$color" "$*" "$C_RESET"
}

# Add uv's standard install locations to this process's PATH.
#
# Installers launched by the app inherit CopySpeak's PATH, which is a snapshot
# from app launch. Right after install-uv.sh runs, uv exists on disk but not
# in that snapshot, so every engine installer would fail until the app is
# restarted. Re-probing the well-known locations avoids that restart.
add_uv_to_path() {
    local candidates=(
        "$HOME/.local/bin"    # official Astral install script
        "$HOME/.cargo/bin"    # cargo install uv
        "/usr/local/bin"      # manual / root installs
    )
    local dir
    for dir in "${candidates[@]}"; do
        if [[ -x "$dir/uv" && ":$PATH:" != *":$dir:"* ]]; then
            PATH="$dir:$PATH"
            log C_GRAY "  Added to PATH for this run: $dir"
        fi
    done
}

# Require uv on PATH. Fails hard (engine installers cannot proceed without it).
require_uv() {
    if ! command -v uv >/dev/null 2>&1; then
        add_uv_to_path
    fi
    if ! command -v uv >/dev/null 2>&1; then
        log C_RED "ERROR: uv is not installed or not on PATH."
        log C_YELLOW "  Install it first:  ./scripts/install-uv.sh"
        exit 1
    fi
    log C_GREEN "  Found uv: $(uv --version)"
}

# "${XDG_DATA_HOME:-$HOME/.local/share}/CopySpeak/engines"
#
# Matches dirs::data_local_dir on Linux, which the Tauri app's engine_dir
# setting uses (XDG_DATA_HOME wins when set, else $HOME/.local/share).
copyspeak_engine_root() {
    printf '%s\n' "${XDG_DATA_HOME:-$HOME/.local/share}/CopySpeak/engines"
}

# Create (or reset with force=1) a uv project directory for one engine.
# Usage: new_engine_project <engine_dir> <0|1 force>
new_engine_project() {
    local engine_dir=$1
    local force=${2:-0}
    if [[ -e "$engine_dir" ]] && (( force )); then
        log C_YELLOW "  Removing existing engine dir (--force): $engine_dir"
        rm -rf "$engine_dir"
    fi
    if [[ ! -e "$engine_dir" ]]; then
        mkdir -p "$engine_dir"
        log C_GRAY "  Created: $engine_dir"
    fi
    if [[ ! -f "$engine_dir/pyproject.toml" ]]; then
        # --name avoids a collision when the directory basename equals a PyPI
        # package name (e.g. engines/piper + `uv add piper` -> "self-
        # dependencies are not permitted"). Prefix keeps the project name
        # unique.
        local project_name
        project_name="copyspeak-$(basename "$engine_dir")"
        # The target dir is positional. `uv --project X init` was accepted by
        # older uv and is a hard error from 0.12 on ("The `--project` option
        # cannot be used in `uv init`").
        log C_GRAY "  Running: uv init --bare --name $project_name ($engine_dir)"
        invoke_uv init --bare --name "$project_name" "$engine_dir"
    fi
}

# Run uv, echoing the exact command. Returns uv's exit code; under `set -e` an
# unhandled failure terminates the installer (the PowerShell original throws).
# Callers that want to recover wrap the call in `if ! invoke_uv ...`.
invoke_uv() {
    log C_DGRAY "  > uv $*"
    local rc=0
    uv "$@" || rc=$?
    if (( rc != 0 )); then
        printf 'uv exited with code %s\n' "$rc" >&2
        return "$rc"
    fi
}

# Install the GPU inference runtime into one engine's uv project.
# Usage: add_cuda_runtime <engine_dir> <onnx|torch>
#
# Deliberately NOT `pip install --user`: engines live in isolated uv projects,
# and a user-site onnxruntime-gpu would shadow whatever build the project
# resolved, for every engine at once.
add_cuda_runtime() {
    local engine_dir=$1
    local runtime=${2:-onnx}
    local failed=0
    log C_YELLOW "  [STEP] cuda"
    if [[ "$runtime" == "torch" ]]; then
        # On Linux the default PyPI torch 2.8.0 IS the CUDA build: the
        # manylinux wheels pull the CUDA user-space libraries in through
        # nvidia-* wheel dependencies, so no custom pytorch.org index is
        # needed (that hack is Windows-only, where PyPI torch has been
        # CPU-only since 2.4). torch and torchaudio keep matching version
        # pins: qwen_tts/pocket-tts import torchaudio, and the extension
        # module links the torch C++ ABI, so the pair must stay in version
        # lockstep on every OS.
        if ! invoke_uv add --project "$engine_dir" "torch==2.8.0" "torchaudio==2.8.0"; then
            failed=1
        fi
    else
        # The CPU wheel and the GPU wheel both provide the `onnxruntime`
        # module, so the CPU one has to go first or resolution is a coin flip.
        invoke_uv remove --project "$engine_dir" onnxruntime || true
        # Current onnxruntime-gpu builds target CUDA 13 (the provider opens
        # libcuda.so.1 / libcublas.so.13 / libcudart.so.13); paired with the
        # old cu12 wheels, session creation dies with "Invalid handle. Cannot
        # load symbol cudnnCreate". nvidia-cuda-runtime-cu13 is a deprecated
        # 0.0.1 placeholder whose build fails on purpose - the CUDA 13 runtime
        # publishes under nvidia-cuda-runtime. nvidia-cudnn-cu13 pulls
        # libcublas/libnvrtc along (libcudnn.so.9); the provider's FFT ops
        # still dlopen libcufft.so.12, so that one stays cu12.
        if ! invoke_uv add --project "$engine_dir" onnxruntime-gpu \
            nvidia-cuda-runtime nvidia-cudnn-cu13 nvidia-cufft-cu12; then
            failed=1
        fi
    fi
    if (( failed )); then
        log C_RED "  [ERROR] cuda (uv add failed)"
        return 1
    fi
    log C_GREEN "  [DONE] cuda"
}

# Prove the GPU actually works by synthesizing with --device cuda.
#
# `ort.get_available_providers()` lists CUDAExecutionProvider even when the
# shared objects are missing, so it can only ever produce a false green. Real
# audio out of a real CUDA session is the only honest check.
# Usage: test_cuda_synthesis <engine_dir> <wrapper> <voice>
test_cuda_synthesis() {
    local engine_dir=$1
    local wrapper=$2
    local voice=$3
    log C_YELLOW "  [STEP] cuda-check"
    local out="$engine_dir/output/cuda-check.wav"
    mkdir -p "$(dirname "$out")"
    rm -f "$out"
    if ! invoke_uv run --project "$engine_dir" python "$wrapper" \
        --text "GPU check" --voice "$voice" --output "$out" --device cuda; then
        log C_RED "  [ERROR] cuda-check (synthesis failed)"
        log C_YELLOW "  GPU synthesis failed; the engine still works on CPU."
        return 1
    fi
    if ! test_audio_file "$out"; then
        log C_RED "  [ERROR] cuda-check (no audio produced)"
        return 1
    fi
    log C_GREEN "  [DONE] cuda-check"
    log C_GRAY "  Enable 'GPU acceleration' in this engine's settings to use it."
}

# Validate that a synthesis smoke test produced a non-empty WAV.
test_audio_file() {
    local path=$1
    if [[ ! -f "$path" ]]; then
        log C_RED "  ERROR: expected audio file not found: $path"
        return 1
    fi
    local size
    size=$(wc -c < "$path")
    size="${size//[[:space:]]/}"
    if (( size <= 44 )); then
        log C_RED "  ERROR: audio file is empty/too small ($size bytes): $path"
        return 1
    fi
    log C_GREEN "  OK: audio file $path ($size bytes)"
}

# Write <engine_root>/<engine>/manifest.json recording which voices are
# installed. The frontend reads it to pre-check the "add a voice later"
# dialog. Call once at the end of a successful install / voice re-run; the
# caller is the source of truth for the voice list (Piper: present .onnx
# basenames; Kitten/Kokoro: all catalog voices since the model is shared).
# Usage: write_engine_manifest <engine_dir> <voice...>
#
# Voice lists are static per engine, so this is plain printf - no jq
# dependency. Voice ids come from our own catalog tables and contain no
# characters that need JSON escaping.
write_engine_manifest() {
    local engine_dir=$1
    shift
    local version="1.0"
    local voices_json=""
    local v first=1
    for v in "$@"; do
        if (( first )); then
            first=0
        else
            voices_json+=", "
        fi
        voices_json+="\"$v\""
    done
    local manifest_path="$engine_dir/manifest.json"
    printf '{\n  "version": "%s",\n  "voices_installed": [%s],\n  "installed_at": "%s"\n}\n' \
        "$version" "$voices_json" "$(date -Iseconds)" > "$manifest_path"
    log C_GREEN "  [DONE] manifest ($# voices)"
}

# -- CLI chrome (shared banner + permission prompt) ---------------------------

# True when the app (not a human) is driving. Every interactive helper below
# silently takes its default in that mode.
is_noninteractive() {
    [[ "${COPYSPEAK_NONINTERACTIVE:-}" == "1" ]]
}

write_engine_banner() {
    local title=$1
    local bar="========================================" # 40 '='
    local pad=$((37 - ${#title}))
    if (( pad < 1 )); then
        pad=1
    fi
    log C_MAGENTA ""
    log C_MAGENTA "  $bar"
    log C_MAGENTA "  |  ${title}$(printf '%*s' "$pad" '')|"
    log C_MAGENTA "  $bar"
    log C_MAGENTA ""
}

# Ask the user before doing something destructive or network-heavy.
# Usage: get_confirmation <prompt> [0|1 default_yes]
# Defaults to No (so a blind Enter keeps the safe path). Returns 0 for yes.
get_confirmation() {
    local prompt=$1
    local default_yes=${2:-0}
    local default="N"
    if (( default_yes )); then
        default="Y"
    fi
    if is_noninteractive; then
        if (( default_yes )); then
            return 0
        fi
        return 1
    fi
    log C_YELLOW "  $prompt"
    local answer=""
    read -r -p "  Proceed? (y/N) [$default] " answer
    if [[ -z "${answer//[[:space:]]/}" ]]; then
        answer="$default"
    fi
    [[ "${answer^^}" == "Y" ]]
}

# Print a numbered menu of voices and echo the chosen Id.
# Usage: chosen=$(select_voice_from_menu <title> <default_id> <id> <label> [<id> <label>...])
# Defaults to the entry marked Default (or the first) on a blank Enter.
select_voice_from_menu() {
    local title=$1
    local default_id=$2
    shift 2
    local ids=() labels=()
    while (( $# > 1 )); do
        ids+=("$1")
        labels+=("$2")
        shift 2
    done
    local count=${#ids[@]}
    if (( count == 1 )); then
        printf '%s\n' "${ids[0]}"
        return 0
    fi
    if [[ -z "$default_id" ]]; then
        default_id="${ids[0]}"
    fi
    if is_noninteractive; then
        printf '%s\n' "$default_id"
        return 0
    fi
    # Menu goes to stderr: callers capture this function's stdout in $( ... ),
    # so anything printed there would be swallowed into the chosen voice.
    echo "" >&2
    log C_CYAN "  $title" >&2
    local i marker
    for ((i = 0; i < count; i++)); do
        marker=""
        if [[ "${ids[i]}" == "$default_id" ]]; then
            marker=" (default)"
        fi
        printf '    %2d. %s - %s%s\n' "$((i + 1))" "${ids[i]}" "${labels[i]}" "$marker" >&2
    done
    local choice
    while true; do
        read -r -p "  Pick a voice [1-$count] " choice
        if [[ -z "${choice//[[:space:]]/}" ]]; then
            printf '%s\n' "$default_id"
            return 0
        fi
        if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= count )); then
            printf '%s\n' "${ids[choice - 1]}"
            return 0
        fi
        log C_RED "  Invalid choice; try again."
    done
}

# Print a ready-to-paste CopySpeak profile snippet. Does NOT edit config (v1).
write_profile_snippet() {
    local json=$1
    echo ""
    log C_CYAN "  Paste this into a CopySpeak profile (Engine settings -> Import):"
    log C_DGRAY "  ----------------------------------------------------------------"
    local line
    while IFS= read -r line; do
        log C_GRAY "  $line"
    done <<< "$json"
    log C_DGRAY "  ----------------------------------------------------------------"
    echo ""
}
