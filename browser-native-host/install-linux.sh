#!/usr/bin/env bash
# Install the CopySpeak native-messaging host on Linux.
#
# 1. Writes two host manifests with the Linux host path
#    (build target overridable via COPYSPEAK_HOST_BIN):
#      com.copyspeak.browser.json          Chromium family (allowed_origins)
#      com.copyspeak.browser.firefox.json  Firefox (allowed_extensions)
# 2. Registers the Chrome manifest for Chromium-family browsers:
#    ~/.config/chromium/NativeMessagingHosts and ~/.config/google-chrome/...
#    (each directory is written only when its browser config dir exists).
# 3. Registers the Firefox manifest in the per-user directory
#    ~/.mozilla/native-messaging-hosts (MDN "Native manifests", Linux).
#
# The Chrome extension ID is pinned via the "key" field in its manifest, so
# allowed_origins survives folder moves and reinstalls. The Firefox ID is the
# browser_specific_settings.gecko.id in browser-extension/build.mjs.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
HOST_BIN="${COPYSPEAK_HOST_BIN:-$REPO_DIR/browser-native-host/target/debug/copyspeak-browser-host}"
EXT_ID="hojbmioeiccpgkpnpblkdbeegjkcccad"
FIREFOX_EXT_ID="copyspeak-companion@ilyaizen.github.io"

if [ ! -x "$HOST_BIN" ]; then
  echo "host binary not found or not executable: $HOST_BIN" >&2
  echo "build it first: (cd browser-native-host && cargo build)" >&2
  exit 1
fi

MANIFEST_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/copyspeak"
mkdir -p "$MANIFEST_DIR"
CHROME_MANIFEST="$MANIFEST_DIR/com.copyspeak.browser.json"
FIREFOX_MANIFEST="$MANIFEST_DIR/com.copyspeak.browser.firefox.json"

cat > "$CHROME_MANIFEST" <<JSON
{
  "name": "com.copyspeak.browser",
  "description": "CopySpeak browser companion native host",
  "path": "$HOST_BIN",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$EXT_ID/"]
}
JSON

cat > "$FIREFOX_MANIFEST" <<JSON
{
  "name": "com.copyspeak.browser",
  "description": "CopySpeak browser companion native host (Firefox)",
  "path": "$HOST_BIN",
  "type": "stdio",
  "allowed_extensions": ["$FIREFOX_EXT_ID"]
}
JSON
echo "manifest: $CHROME_MANIFEST"
echo "manifest: $FIREFOX_MANIFEST"

# Chromium + Google Chrome
registered=0
for browser_dir in \
  "$HOME/.config/chromium/NativeMessagingHosts" \
  "$HOME/.config/google-chrome/NativeMessagingHosts"; do
  if [ -d "${browser_dir%/*}" ]; then
    mkdir -p "$browser_dir"
    ln -sfn "$CHROME_MANIFEST" "$browser_dir/com.copyspeak.browser.json"
    echo "registered: $browser_dir"
    registered=1
  fi
done

# Firefox: the file name must match the name passed to connectNative.
if [ -d "$HOME/.mozilla" ]; then
  mkdir -p "$HOME/.mozilla/native-messaging-hosts"
  ln -sfn "$FIREFOX_MANIFEST" "$HOME/.mozilla/native-messaging-hosts/com.copyspeak.browser.json"
  echo "registered: $HOME/.mozilla/native-messaging-hosts"
  registered=1
fi

if [ "$registered" -eq 0 ]; then
  echo "no Chromium/Chrome/Firefox config dirs found; manifests written but not registered" >&2
  exit 2
fi
