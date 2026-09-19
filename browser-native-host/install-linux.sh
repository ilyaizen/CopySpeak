#!/usr/bin/env bash
# Install the CopySpeak native-messaging host on Linux.
#
# 1. Writes com.copyspeak.browser.json with a Linux host path
#    (build target overridable via COPYSPEAK_HOST_BIN).
# 2. Registers it for Chromium-family browsers:
#    ~/.config/chromium/NativeMessagingHosts and ~/.config/google-chrome/...
#    (Chromium reads ~/.config/chromium; the google-chrome dir is written
#    when it exists, for Google Chrome builds.)
# 3. Registers for Firefox-family browsers when a profile exists:
#    ~/.mozilla/native-messaging (Firefox 150+ global dir).
#
# The extension ID is pinned via the "key" field in the extension manifest,
# so allowed_origins survives folder moves and reinstalls.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
HOST_BIN="${COPYSPEAK_HOST_BIN:-$REPO_DIR/browser-native-host/target/debug/copyspeak-browser-host}"
EXT_ID="hojbmioeiccpgkpnpblkdbeegjkcccad"

if [ ! -x "$HOST_BIN" ]; then
  echo "host binary not found or not executable: $HOST_BIN" >&2
  echo "build it first: (cd browser-native-host && cargo build)" >&2
  exit 1
fi

MANIFEST_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/copyspeak"
mkdir -p "$MANIFEST_DIR"

cat > "$MANIFEST_DIR/com.copyspeak.browser.json" <<EOF
{
  "name": "com.copyspeak.browser",
  "description": "CopySpeak browser companion native host",
  "path": "$HOST_BIN",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$EXT_ID/"]
}
EOF
echo "manifest: $MANIFEST_DIR/com.copyspeak.browser.json"

# Chromium + Google Chrome
registered=0
for browser_dir in \
  "$HOME/.config/chromium/NativeMessagingHosts" \
  "$HOME/.config/google-chrome/NativeMessagingHosts"; do
  if [ -d "${browser_dir%/*}" ]; then
    mkdir -p "$browser_dir"
    ln -sfn "$MANIFEST_DIR/com.copyspeak.browser.json" "$browser_dir/com.copyspeak.browser.json"
    echo "registered: $browser_dir"
    registered=1
  fi
done

# Firefox 150+ global native-messaging directory
if [ -d "$HOME/.mozilla" ]; then
  mkdir -p "$HOME/.mozilla/native-messaging"
  ln -sfn "$MANIFEST_DIR/com.copyspeak.browser.json" "$HOME/.mozilla/native-messaging/com.copyspeak.browser.json"
  echo "registered: $HOME/.mozilla/native-messaging (Firefox: also add to the extension's manifest)"
  registered=1
fi

if [ "$registered" -eq 0 ]; then
  echo "no Chromium/Chrome/Firefox config dirs found; manifest written but not registered" >&2
  exit 2
fi
