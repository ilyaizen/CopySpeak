# CopySpeak Glossary

CopySpeak reads clipboard text aloud. This is the project's single ubiquitous language: use these terms exactly in code, docs, issues, and discussion.

## Language

**Local engine**:
A TTS engine that runs on-device, installed as a uv-managed project under the engines directory (Kokoro, Kitten, Piper, Pocket, Qwen).
_Avoid_: offline engine, on-device provider

**Cloud engine**:
A TTS engine backed by a hosted API and an API key (OpenAI, ElevenLabs, Fish, Cartesia, Google, Microsoft, xAI, Mistral).
_Avoid_: online engine, remote engine

**Qwen**:
The local Qwen3-TTS engine (`Qwen/Qwen3-TTS-12Hz-*`). Never refers to a cloud service; Alibaba's hosted TTS was rejected (ADR-0003).
_Avoid_: Qwen cloud, DashScope (except when discussing the rejected provider)

**Emote tags**:
Markup inside the spoken text that an engine interprets as delivery (laugh, pause, whisper) instead of reading aloud. Each engine has its own dialect; the syntax belongs to the engine, not to CopySpeak.
_Avoid_: speech tags, expressive markup (as a noun for the feature)

**Dramatize**:
The LLM post-process that rewrites text to include emote tags before synthesis. Gated per (engine, model) by `supports_emote_tags`, and only for engines whose dialect the prompt can emit.
_Avoid_: post-processing (the broader pipeline), condense (the older name)

**Captions**:
Word-level timings aligned to the synthesized audio, used for playback highlighting. Either **native** (the provider returns timings) or **estimated** (CopySpeak derives them from audio duration).
_Avoid_: subtitles, transcript
