# 2. Streaming consolidation

Status: accepted (2026-10-13). Slice 1 implements D2, D4 and D7. Slice 2 implements D1 and the helper part of D6. Slice 3 implements the producer-driver part of D6 for OpenAI, Cartesia, Google and ElevenLabs, and adds the is_final rule test. The D9 gate and effects tests predate this slice, and the odd-byte split tests are in `pcm-stream.test.ts`. playback-finished after a failed or empty stream is open (see Open). Slice 4 (contract) removes the batch-wrap default from `TtsBackend::synthesize_streaming`: a backend without a native stream now refuses instead of wrapping its batch WAV. `chunk_stream_from_wav` is removed; its tests use `chunk_stream_from_speech`. The static `supports_streaming()` claim stays, because D1 needs it at the gate.

## Context

Each `TtsBackend` reports streaming through one `supports_streaming()` bool. The Track 1 audit found two overrides that were wrong for part of their inputs: OpenAI returned `true` for every model, and the CLI daemon reported `true` for kokoro and pocket, whose wrappers yield one chunk per utterance. The Gemini override is unverified against vendor docs.

## Decisions

- D1. Split the capability into a static claim (per backend or per model) and a runtime gate (daemon readiness). Call sites gate on static AND runtime_ready. Slice 2: `TtsBackend::runtime_ready` (default true) carries daemon warm-up; CLI `supports_streaming` is static only.
- D2. OpenAI streaming is per model. `gpt-4o-mini-tts` and ids starting with it return `true`. `tts-1`, `tts-1-hd` and unknown ids return `false`. Approved by the user. The docs' streaming examples use only gpt-4o-mini-tts; they do not state that tts-1 cannot stream.
- D3. Gemini stays `true`, unproven. The live probe needs the user's own key.
- D4. CLI engines kokoro and pocket return `false`. Their wrappers yield one whole-utterance chunk, so there is no time-to-first-audio gain. piper, kitten and qwen are unchanged and still gated on a warm daemon.
- D5. ElevenLabs and Cartesia stay `true`, matching their vendor docs. edge and microsoft stay `false`. http stays `false`.
- D6. Expand-contract for the remaining backends: one shared `block_on_async` helper and a per-engine producer closure. Not a request/decode trait. The helper landed in slice 2; the producer driver lands in a later slice.
- D7. Doc fixes: the ElevenLabs comment says `pcm_24000` (F4), and the streaming doc comment documents `supports_streaming` (F7). Behaviour is unchanged.
- D8. F8, the possible double WAV header in `google.rs`, is reported only. It is not changed here.
- D9. Each streaming invariant needs a test: `is_final` only on the last fragment; the `allows_streaming_playback` gate; the effects gate; the OpenAI odd-byte remainder carried across chunks; `playback-finished` on every terminal path. Those tests are a later slice.

## Consequences

Every OpenAI user on the default model (`tts-1`) and every user who sets `tts-1` or `tts-1-hd` moves to the batch path. Playback then starts after full synthesis, the same behaviour non-streaming engines already have. The config default is unchanged in this slice; Track 2 changes it to `gpt-4o-mini-tts`, which restores streaming for default users.

## Open

- Gemini live probe: needs the user's own key. Then either keep `streamGenerateContent` or move to the documented Interactions API.
- A failed or empty stream does not reach playback-finished. `drain_chunk_stream` returns `Err` on `ChunkItem::Failed` and on an empty stream, with no final chunk, `playback-stop` or `synthesis-aborted`. `/speak --wait` gets the error at once (`handle_speak` propagates it), so it does not hang. The webview scheduler stays stalled and `playback-finished` never fires. `audio_duck` is driven by those events, so ducking may not restore until the next reading ends. Needs a decision on the terminal event before fixing.
- The CLI daemon stream (`local_daemon::try_stream`) is a synchronous pipe protocol and stays outside the async producer driver.
- tts-1 chunked-streaming confirmation: D2 relies on the docs showing streaming examples only for gpt-4o-mini-tts. Whether tts-1 streams is unconfirmed.
