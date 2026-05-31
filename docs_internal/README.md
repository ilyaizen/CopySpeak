# Internal Documentation

This directory is the repo-local internal docs surface for `copyspeak-tts`.

## Canonical locations

- Public GitHub repo: `ilyaizen/copyspeak-tts`
- Canonical checkout on the box: `/srv/apps/copyspeak-tts`
- Vault mirror: `/root/workspace/HyperVault/projects/copyspeak-tts`

## Read order

1. `major-rename.md` — what changed in the rename and what still matters.
2. `docs/` — public-facing or pipeline-facing docs.
3. `README.md` — product summary and quick start.

## Rule

Treat these docs as operational source of truth for the rename. The app was renamed from `CopySpeak` to `CopySpeak TTS`, the repo is now lowercase `copyspeak-tts`, and the updater/release URLs were switched to the new repo.

## Note

`docs_internal/` is ignored by Git on purpose. It lives on the box and in the vault, not on GitHub.