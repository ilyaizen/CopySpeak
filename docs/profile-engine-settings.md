# Profile Engine Settings

CopySpeak profiles are the user-facing synthesis abstraction. Users should pick `Cartesia - Katie`, `Narrator`, or `Rachel`, not rebuild a long engine/voice/model/effect command every time.

## Boundary

### Profile-owned

Profiles may contain non-secret synthesis intent:

- engine/provider
- voice id/name and optional display label
- speed and pitch
- output format/model where it changes synthesis behavior
- engine-specific non-secret knobs
- profile effects
- profile text-processing policy

### Global-only

Global engine config owns credentials, smoke tests, docs links, and machine/account setup:

- API keys
- secret auth headers
- model/output-format fallback defaults for profiles that omit them
- locally installed engine paths when they are machine-specific
- cloud account defaults that should not be exported

Exported profiles must be safe to share. If a field can leak a secret, it does not belong in a profile export.

## Engine matrix

| Engine | Docs | Profile settings | Global settings |
| --- | --- | --- | --- |
| Local CLI | Existing wrappers and `src-tauri/src/tts/cli.rs` | preset, command, args template, voice, speed where wrapper supports it | machine-specific install paths if not portable |
| HTTP | `src-tauri/src/tts/http.rs` and target server docs | URL template, method, non-secret headers, body template, response format, timeout, voice, speed | secret headers/API keys |
| OpenAI | <https://platform.openai.com/docs/guides/text-to-speech> | model, voice, speed, response format, optional instructions | API key |
| ElevenLabs | <https://elevenlabs.io/docs/api-reference/text-to-speech/convert> | voice id/name, model id, output format, stability, similarity, style, speaker boost | API key |
| Cartesia | <https://docs.cartesia.ai/api-reference/tts/bytes> | model id, voice id/name, output format, encoding, sample rate | API key |
| Google Gemini TTS | <https://ai.google.dev/gemini-api/docs/speech-generation> | model, voice name, output format | API key |
| Microsoft / Azure | <https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech> | endpoint, model, voice name, output format | API key; endpoint if deployment-global |

## Documentation verification

- [ ] Verify current OpenAI TTS voice list and instructions support by model.
- [ ] Verify ElevenLabs voice settings and output format names.
- [ ] Verify Cartesia bytes endpoint output format fields.
- [ ] Verify Gemini prebuilt voice list.
- [ ] Verify Microsoft MAI/Azure speech endpoint variants.

Static catalog entries are fallbacks. Provider APIs win when available.

## UX

- The footer is a profile switcher. Switching profiles updates `active_profile_id`; `active_backend` is only a compatibility mirror.
- The Engine page is for API keys, model/output fallback defaults, engine smoke tests, docs, and future installer links. Voice selection belongs to Profiles.
- New and migrated default profiles keep `id: "default"` but display as `Engine - Voice` (for example, `Cartesia - Katie`).

## HTTP and CLI semantics

`POST /speak` with a profile is request-local by default:

```json
{ "profile": "pi", "text": "hello" }
```

It must not silently change the active desktop profile. Persisting active profile is explicit:

```json
{ "profile": "pi", "text": "hello", "persist_selection": true }
```

or:

```http
POST /profiles/active
{ "profile": "pi" }
```

The CLI is a thin wrapper over the localhost control server:

```bash
node scripts/copyspeak.mjs health
node scripts/copyspeak.mjs speak --profile pi "hello"
node scripts/copyspeak.mjs speak -p narrator --stdin
node scripts/copyspeak.mjs profiles list
node scripts/copyspeak.mjs profiles use pi
node scripts/copyspeak.mjs profiles show pi
node scripts/copyspeak.mjs engines list
node scripts/copyspeak.mjs voices list --engine elevenlabs
```

The server is trusted localhost automation. Do not bind it to `0.0.0.0` without adding authentication first.
