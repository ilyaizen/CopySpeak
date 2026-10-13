# 2. Streaming consolidation

Status: accepted (2026-10-13). Slice 1 implements D2, D4 and D7. The other decisions land in later Track 1 slices.

## Context

Each `TtsBackend` reports streaming through one `supports_streaming()` bool. The Track 1 audit found two overrides that were wrong for part of their inputs: OpenAI returned `true` for every model, and the CLI daemon reported `true` for kokoro and pocket, whose wrappers yield one chunk per utterance. The Gemini override is unverified against vendor docs.

## Decisions

- D1. Split the capability into a static claim (per backend or per model) and a runtime gate (daemon readiness). Call sites gate on static AND runtime_ready. Later slice.
- D2. OpenAI streaming is per model. `gpt-4o-mini-tts` and ids starting with it return `true`. `tts-1`, `tts-1-hd` and unknown ids return `false`. Approved by the user. The docs show chunked streaming only for the gpt-4o-mini-tts family.
- D3. Gemini stays `true`, unproven. The live probe needs the user's own key.
- D4. CLI engines kokoro and pocket return `false`. Their wrappers yield one whole-utterance chunk, so there is no time-to-first-audio gain. piper, kitten and qwen are unchanged and still gated on a warm daemon.
- D5. ElevenLabs and Cartesia stay `true`, matching their vendor docs. edge and microsoft stay `false`. http stays `false`.
- D6. Expand-contract for the remaining backends: one shared `block_on_async` helper and a per-engine producer closure. Not a request/decode trait. Later slice.
- D7. Doc fixes: the ElevenLabs comment says `pcm_24000` (F4), and the streaming doc comment documents `supports_streaming` (F7). Behaviour is unchanged.
- D8. F8, the possible double WAV header in `google.rs`, is reported only. It is not changed here.
- D9. Each streaming invariant needs a test: `is_final` only on the last fragment; the `allows_streaming_playback` gate; the effects gate; the OpenAI odd-byte remainder carried across chunks; `playback-finished` on every terminal path. Those tests are a later slice.

## Consequences

Users who set `tts-1` or `tts-1-hd` explicitly move to the batch path. Playback then starts after full synthesis, the same behaviour non-streaming engines already have. The config default (`tts-1`) is unchanged until the Track 2 PR.

## Open

- Gemini live probe: needs the user's own key. Then either keep `streamGenerateContent` or move to the documented Interactions API.
- tts-1 chunked-streaming confirmation: D2 relies on the docs listing only the gpt-4o-mini-tts family for streaming.
