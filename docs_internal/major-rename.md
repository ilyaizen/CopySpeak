---
title: Major Rename: CopySpeak to CopySpeak TTS
status: active
created: 2026-05-31
updated: 2026-05-31
tags:
  - project/copyspeak-tts
  - rename
  - github
  - updater
---

# Major Rename: CopySpeak -> CopySpeak TTS

## What changed

- GitHub repository renamed from `ilyaizen/CopySpeak` to `ilyaizen/copyspeak-tts`.
- Product branding changed to `CopySpeak TTS`.
- Local repo name and clone path changed to lowercase `copyspeak-tts`.
- GitHub URLs and release URLs were updated across the repo.
- Tauri product/window/publisher strings now use `CopySpeak TTS`.

## Updater status

The updater endpoint in `src-tauri/tauri.conf.json` now points at:

`https://github.com/ilyaizen/copyspeak-tts/releases/latest/download/latest.json`

What that means:

- New builds will ship the correct updater URL.
- GitHub rename redirects help with old repo URLs, but the shipped binary still needs a rebuild to permanently embed the new endpoint.
- The updater should keep working once the next release is built and published from the renamed repo.

## GitHub Actions status

The main Windows release workflow still uses the repo context (`github.repository`) and `tauri-action`.
No workflow code depended on the old repo slug.

What to watch on the next release:

- release name should remain `CopySpeak TTS vX.Y.Z`
- release assets should still be uploaded to the renamed repo
- the new release should publish `latest.json` in the renamed repo so the updater has a live feed

## Local checkout

- Canonical app checkout on the box: `/srv/apps/copyspeak-tts`
- Default Git remote: `https://github.com/ilyaizen/copyspeak-tts.git`

## Verification checklist

- repo URL resolves to `https://github.com/ilyaizen/copyspeak-tts`
- app metadata says `CopySpeak TTS`
- release workflow still points at the current repo context
- updater URL is the new repo URL
- the next build publishes a fresh release from the renamed repo
