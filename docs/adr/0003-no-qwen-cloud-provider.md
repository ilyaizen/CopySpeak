# 3. No Qwen cloud provider

Status: accepted (2026-10-25).

## Context

`TtsEngine::Qwen` is the local Qwen3-TTS engine (`Qwen/Qwen3-TTS-12Hz-*`, installed by `scripts/install-qwen.ps1`). Alibaba Cloud Model Studio separately offers `qwen3-tts-flash` as a hosted API. While scoping the xAI and Mistral providers, the Qwen cloud API was evaluated and rejected. Its constraints: input is capped at 600 characters (read-aloud texts regularly exceed that); the non-realtime response is a JSON envelope whose audio is an expiring signed OSS URL, forcing a second authenticated-adjacent fetch and cache step; and there is no batch streaming — realtime is a WebSocket-only model. The DashScope keys are also region-bound (Singapore vs Beijing), an extra credential failure mode.

## Decision

Do not add a Qwen cloud provider. `TtsEngine::Qwen` remains local-only, and the `Qwen` name stays with the local engine — no variant rename, no cloud sibling. The cloud providers added in this round are xAI and Mistral.

## Considered options

- **Chunking above 600 chars** — rejected: forced sentence-splitting costs quota and prosody seams, the same tradeoff already rejected for Fish (short reads are never force-split there).
- **`qwen3-tts-flash-realtime` (WebSocket)** — rejected: a WS client in the backend is a separate work stream, and ADR-0002 set the rule that streaming is only claimed with a verified incremental endpoint.

## Consequences

Re-propose only with a concrete need the local engine cannot meet (specific voices or languages unavailable locally on a no-VRAM machine). The 600-character wall is the structural reason and does not change with pricing.
