# Captions assessment (Hermes Desktop plugin)

Verdict: GO, narrow scope. Paint runs on source-read DOM hooks only. No live DOM probe was possible (CDP port 9222 closed).

## Checks

1. **/status reachable from the renderer: yes, CORS open.**
   - Route at `src-tauri/src/control_server.rs:167`. Parse at `:584`.
   - Reply uses `http_response_cors` with `Access-Control-Allow-Origin: *` (`:791-799`). Live curl confirmed the header and body `{"active":false}`.
   - Body comes from `current_caption()` (`src-tauri/src/playback_signal.rs:116-119`). It serves the last report while younger than 1 s.
   - Shape (from `src/lib/stores/playback-store.svelte.ts:576` and `src/lib/types/hud.ts:55-65`): `active`, `paused`, `text`, `captions:{text, words:[{text_start,text_end,start_ms,end_ms}]}`, `position_ms`, `duration_ms`, `fragment_index`.
   - Inactive report at `playback-store.svelte.ts:518`. Reports throttle to ~100 ms (`:571-578`).
   - The failure log says `/status` is the only CORS-open route (`docs/agents/failure-log.md:44-63`). Code agrees.

2. **Correlation with our request: NO.**
   - `/status` carries no `request_id`. The plugin cannot tell whether the live reading is its own.
   - Only link: the caption text. Alignment must match the caption words to our rendered message. A 50% floor rejects foreign readings.
   - A reading that the user starts elsewhere that speaks the same text would also paint. Accepted.

3. **DOM hooks (source only, not run):**
   - (a) Assistant turn root: `data-role="assistant"` and `data-slot="aui_assistant-message-root"` (`apps/desktop/src/components/assistant-ui/thread/assistant-message.tsx:313-314`). Rank: data-slot, then role.
   - (b) Rendered Markdown body: class `aui-md` (`apps/desktop/src/components/assistant-ui/markdown-text.tsx:556`). Literal class, not hashed. Rank: class (fixed string).
   - Exclude reasoning: `data-slot="aui_reasoning-text"` (`thread/message-parts.tsx:18, 461`), `aui_thinking-disclosure` (`:334`), `aui_thinking-body` (`:360`).
   - Tool blocks: not verified. Assumed to hold no `aui-md` text.
   - Message id: no `data-message-id` setter found in desktop `src`. Timeline code reads it (`thread/timeline.tsx:202`), so a library may set it. UNVERIFIED.
   - Link to `message.complete`: only "last assistant root in the focused session". `persisted_turn.final_assistant_row_id` is not in the DOM.

4. **CSS Custom Highlight API: available by version.**
   - Electron `44.10.2` (`apps/desktop/package.json:195`). Chromium supports `CSS.highlights` and `Highlight` well before that. Known from Chromium history, not probed in this runtime.
   - Pitfalls from `docs/agents/failure-log.md:242-253`: never shadow `CSS`; hold on index -1; group per-character entries; resync across the excerpt gap.

5. **Reference port size (`copyspeak-dsh/client.js`, read only):**
   - Poll loop: `:1503-1557`. Status read: `:1589-1600`. Caption parse: `:1606-1624`.
   - Alignment: `:472-745`. Caption grouping: `:715-738`. Gap resync: `:631-645`.
   - Paint: `:1093-1175` (range rects, glide). Glide, pill, scroll follow: not ported.
   - Minimal port estimate: ~170-200 lines. Under the 250-line limit.

## Verdict against the four GO gates

- /status reachable from renderer: PASS (source and live curl).
- Stable DOM hook for body: PASS (`data-slot` and `aui-md`; not hashed).
- CSS Highlight API: PASS by version (not run in app).
- Port under ~250 lines: PASS (estimate ~170-200).

## Risks

- Excerpt gap is the default path (`excerpt: true`). The resync search must run; it is not an edge case.
- Common words can match far ahead. A run check (3 words in order) guards the resync.
- No in-app run possible from my side. Paint is UNVERIFIED in app until the owner checks.
- Alignment uses no request_id. Foreign readings with different text fail the floor and do not paint.

## Open questions

- Does the Hermes renderer allow `fetch` to `127.0.0.1:43117` in `mode: 'cors'`? No CSP `connect-src` found in the renderer. Not run.
- Are tool blocks free of `aui-md` text? Not checked.
