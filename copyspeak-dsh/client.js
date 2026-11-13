/**
 * Browser half of the CopySpeak voice bundle.
 *
 * Adds six surfaces that share one transport to the CopySpeak control server:
 *
 *   POST http://127.0.0.1:43117/speak
 *   {"text": "...", "request_id": "dsh.<stamp>.<n>", "wait": true}
 *
 * - A CopySpeak panel in the sidebar, holding the three options and the app status.
 * - A speaker action in the action row of each finished assistant message.
 * - A per-session reader that speaks the final reply of each completed turn,
 *   and highlights the word CopySpeak plays inside the reply text itself.
 * - A read-selection action that sits beside the host Quote action, speaks
 *   only the selected words, and highlights them as CopySpeak plays.
 * - A frame-wide icon-only playback bar. It pauses, resumes, rewinds and
 *   skips, stops the reading, and scrolls back to the spoken word when the
 *   message action scrolled out of view.
 *
 * The request uses `mode: 'no-cors'` and a `text/plain` content type, so the
 * browser sends no CORS preflight; CopySpeak parses the JSON body without
 * reading the content type. The response is opaque, so a success only proves
 * the request reached the app. `wait: true` makes it resolve at playback end,
 * which restores the idle action state. A paused reading keeps that request
 * open until it resumes and ends, or until it stops.
 *
 * The control routes address one reading by the same request id:
 *
 *   POST /pause  {"request_id": "..."}
 *   POST /resume {"request_id": "..."}
 *   POST /seek   {"request_id": "...", "delta_ms": -5000}
 *
 * Captions and the bar poll `GET http://127.0.0.1:43117/status`, which answers
 * the caption snapshot and the pause state the CopySpeak webview already emits.
 * That read-only route is CORS-open. Polling runs only while this plugin's own
 * reading plays, and the highlight hides when the route is absent.
 */
window.__ModuleLoader__.load({
  id: "@local/copyspeak-voice",
  factory(require) {
    const React = require("react");
    const h = React.createElement;

    const NS = "copyspeak-voice";
    const PACKAGE = "@local/copyspeak-voice";
    const PANEL_ID = "copyspeak";
    /* The read-only selection interaction. The host Quote id owns a writable
       selection, so this id admits the read action where Quote cannot open. */
    const SELECTION_ID = "copyspeak-read-selection";
    const CONTROL_URL = "http://127.0.0.1:43117";
    const STYLE_KEY = PACKAGE + "/client.css";
    /* Custom-highlight names. Namespace them: other plugins may paint
       highlights. A normal word uses the first name plus the rounded mark
       behind it. A word inside a code chip or code block uses the same name:
       the plugin makes chip backgrounds transparent, so the mark shows. */
    const HIGHLIGHT_NAME = "copyspeak-read-word";
    /* Tween-only names: the outgoing and incoming word during one glide. */
    const HIGHLIGHT_NAME_OUT = "copyspeak-read-word-out";
    const HIGHLIGHT_NAME_IN = "copyspeak-read-word-in";
    const STORAGE = {
      autoRead: "dsh.copyspeak-voice.auto-read",
      captions: "dsh.copyspeak-voice.captions",
      follow: "dsh.copyspeak-voice.follow",
      animation: "dsh.copyspeak-voice.animation"
    };
    /* The webview reports its audible clock every ~25 ms and throttles the
     * control-server report to ~100 ms, so one poll per 100 ms stays current. */
    const STATUS_POLL_MS = 100;
    /* Stop polling when the status route never answers (older CopySpeak). */
    const STATUS_ATTEMPTS = 3;
    /* The seek controls move the reading by this many milliseconds. */
    const SEEK_STEP_MS = 5000;
    /* A status position that drops by more than this marks a rewind, because
     * the app reports the position inside one fragment and playback only
     * advances while it plays. */
    const REWIND_SLOP_MS = 300;
    /* An optimistic pause survives this long before a poll may override it, so
     * a poll that started before the app applied the command cannot flicker
     * the glyph back. */
    const PAUSE_RECONCILE_MS = 350;
    /* The follow scroll moves at most once per 400 ms. */
    const SCROLL_INTERVAL_MS = 400;
    /* Follow keeps the spoken word between these two fractions of the chat
     * scrollport. Outside the band, its line moves to the anchor. */
    const FOLLOW_BAND_TOP = 0.15;
    const FOLLOW_BAND_BOTTOM = 0.82;
    const FOLLOW_ANCHOR = 0.34;
    /* A manual scroll pauses following for a few seconds. */
    const FOLLOW_PAUSE_MS = 5000;
    /* The rounded word mark sits behind the spoken word. The CSS Custom
     * Highlight API cannot round its background, so a positioned mark paints
     * the brand pill behind the text while the highlight recolors the word. */
    const MARK_PADDING_X = 2;
    const MARK_PADDING_Y = 1;
    /* The mark's glide duration. The word's text recolor must land when the
     * pill arrives, not when the word becomes active, so the pending recolor
     * waits this long plus a frame. */
    const MARK_TRANSITION_MS = 120;
    /* Host Tooltip: the bubble sits this far from its anchor. */
    const TOOLTIP_GAP = 8;
    /* Keep the centered bubble this far from the viewport edge. */
    const TOOLTIP_MARGIN = 12;
    /* Token look-ahead while aligning a caption fragment to rendered words. */
    const ALIGN_WINDOW = 24;
    const ALIGN_ATTEMPTS = 8;

    /*
     * CopySpeak app-logo mark: the speaker and its two sound waves. It is one
     * currentColor silhouette, so the playing action follows the host theme.
     * The idle action and the sidebar icon share the outlined variant below.
     */
    const GLYPH_VIEW_BOX = "14.86 14.717 48.611 48.611";
    const GLYPH_PATHS = [
      "m37.35 20.74c-0.74-0.04-1.46 0.21-2.02 0.71l-7.68 7.13c-0.24 0.23-0.56 0.37-0.9 0.39l-5.58 0.21c-2.55 0.1-4.51 2.23-4.51 5v7.99c0 2.81 2.1 4.95 4.74 5.09l5.3 0.28c0.28 0.02 0.54 0.15 0.74 0.35l7.94 8.12c0.71 0.71 1.67 1.04 2.55 1.02 1.52-0.04 2.65-1.2 2.65-2.76v-30.18c0-1.84-1.65-3.27-3.23-3.35z",
      "m49.61 19.43c-1.5-0.74-3.32-0.37-4.11 1.04-0.8 1.45-0.21 3.13 1.27 3.97 5.25 2.81 8.39 8.04 8.39 14.44 0 6.47-3.45 12.05-8.57 14.61-1.47 0.81-2 2.47-1.33 3.85 0.72 1.4 2.78 2.03 4.44 1.41 7.13-3.14 11.97-10.79 11.97-19.57 0-8.67-5.02-16.48-12.06-19.75z",
      "m46.38 28.77c-1.27-0.75-2.96-0.42-3.67 0.89-0.75 1.28-0.3 2.86 0.86 3.78 1.73 1.33 2.77 3.26 2.77 5.48 0 2.27-1.14 4.28-2.83 5.45-1.2 0.9-1.48 2.53-0.7 3.78 0.84 1.23 2.55 1.74 3.95 1.16 3.49-1.78 5.54-5.56 5.54-9.89 0-4.62-2.49-8.79-5.92-10.65z"
    ];
    /* The idle action and the sidebar icon draw the same mark as line art. The
       wave arcs are the centerlines of the two filled crescents in the app
       logo, fitted to circles, so the pair stays evenly nested and symmetric.
       The waves are drawn as centerline arcs, which stay crisp at 16-17 px. */
    const GLYPH_STROKE = 3.3;
    const GLYPH_ARC_OUTER = "M48.17 21.93A19.44 19.44 0 0 1 48.17 56.13";
    const GLYPH_ARC_INNER = "M45.06 31.13A9.43 9.43 0 0 1 45.06 46.87";
    /* The colored app logo (copyspeak-dsh/icon.svg) as a data URI, because a
       linked plugin cannot fetch its own package files at runtime. The bar
       shows this mark at its head; the message action keeps the theme glyphs. */
    const APP_LOGO_URL =
      "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjIuODQ5IDEuOTc0IDczLjg3MiA3My44NzIiIHdpZHRoPSI2NCIgaGVpZ2h0PSI2NCIgcm9sZT0iaW1nIiBhcmlhLWxhYmVsPSJDb3B5U3BlYWsgYXBwIGxvZ28iPgogIDxkZWZzPgogICAgPGxpbmVhckdyYWRpZW50IGlkPSJwYWludDFfbGluZWFyXzIzMTJfNTQ4MiIgeDE9IjQwIiB4Mj0iNjkuNzIiIHkxPSIzLjQyIiB5Mj0iNzIuMzIiIGdyYWRpZW50VW5pdHM9InVzZXJTcGFjZU9uVXNlIj4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzYwQjNBMSIgb2Zmc2V0PSIwIi8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM2MEFFQTciIG9mZnNldD0iLjE0NTgiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzQxODc4QSIgb2Zmc2V0PSIuMjg2NSIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjM0M4MDgzIiBvZmZzZXQ9Ii44NTQyIi8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiMzMjZENzEiIG9mZnNldD0iMSIvPgogICAgPC9saW5lYXJHcmFkaWVudD4KICAgIDxsaW5lYXJHcmFkaWVudCBpZD0icGFpbnQyX2xpbmVhcl8yMzEyXzU0ODIiIHgxPSIxOS4yNCIgeDI9IjQ5LjMzIiB5MT0iNi44NjQiIHkyPSI3My4wNyIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjODVEMEE1IiBvZmZzZXQ9IjAiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzY2QkM5NSIgb2Zmc2V0PSIuNDY4OCIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNTVBQzk1IiBvZmZzZXQ9IjEiLz4KICAgIDwvbGluZWFyR3JhZGllbnQ+CiAgICA8bGluZWFyR3JhZGllbnQgaWQ9InBhaW50M19saW5lYXJfMjMxMl81NDgyIiB4MT0iMzcuNjUiIHgyPSIzNy42NSIgeTE9IjEwLjI2IiB5Mj0iNjcuMTQiIGdyYWRpZW50VW5pdHM9InVzZXJTcGFjZU9uVXNlIj4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzNEODM4MCIgb2Zmc2V0PSIwIi8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM0QTk3OTEiIG9mZnNldD0iLjIyOTIiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzNEODI4NSIgb2Zmc2V0PSIuNzI0Ii8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM1QkFFQTYiIG9mZnNldD0iMSIvPgogICAgPC9saW5lYXJHcmFkaWVudD4KICAgIDxsaW5lYXJHcmFkaWVudCBpZD0icGFpbnQ1X2xpbmVhcl8yMzEyXzU0ODIiIHgxPSIyOC40MyIgeDI9IjQ0LjA1IiB5MT0iMjAuNzQiIHkyPSI1NC42NSIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNzVDMjlDIiBvZmZzZXQ9IjAiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzYwQjY5OCIgb2Zmc2V0PSIuNDc0Ii8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM1MEE3OTciIG9mZnNldD0iMSIvPgogICAgPC9saW5lYXJHcmFkaWVudD4KICAgIDxsaW5lYXJHcmFkaWVudCBpZD0icGFpbnQ2X2xpbmVhcl8yMzEyXzU0ODIiIHgxPSIyOC40NSIgeDI9IjQyLjAyIiB5MT0iMjAuNzQiIHkyPSI1NS45IiBncmFkaWVudFVuaXRzPSJ1c2VyU3BhY2VPblVzZSI+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM1MEEyOTYiIG9mZnNldD0iMCIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNDI4Qjg4IiBvZmZzZXQ9Ii4zNDkiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzNEODI4NSIgb2Zmc2V0PSIuNjgyMyIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjMzU3Mjc1IiBvZmZzZXQ9IjEiLz4KICAgIDwvbGluZWFyR3JhZGllbnQ+CiAgICA8bGluZWFyR3JhZGllbnQgaWQ9InBhaW50N19saW5lYXJfMjMxMl81NDgyIiB4MT0iMjkuMjMiIHgyPSIyOS4yMyIgeTE9IjIxLjI3IiB5Mj0iNTMuMzQiIGdyYWRpZW50VW5pdHM9InVzZXJTcGFjZU9uVXNlIj4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzg4RDBBNyIgb2Zmc2V0PSIwIi8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM4Q0QxQTciIG9mZnNldD0iLjIyNCIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNzNDMjk4IiBvZmZzZXQ9Ii40NDI3Ii8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM2Q0MwOTYiIG9mZnNldD0iLjY3MTkiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzRDOUU5NyIgb2Zmc2V0PSIuODI4MSIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNEM5RDkzIiBvZmZzZXQ9IjEiLz4KICAgIDwvbGluZWFyR3JhZGllbnQ+CiAgICA8bGluZWFyR3JhZGllbnQgaWQ9InBhaW50OF9saW5lYXJfMjMxMl81NDgyIiB4MT0iNTMuOCIgeDI9IjYzLjc2IiB5MT0iMTkuMDYiIHkyPSI1Ni4xMSIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNUFBQ0EzIiBvZmZzZXQ9IjAiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzQyOEI4RiIgb2Zmc2V0PSIuMzM4NSIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjM0I3RjgyIiBvZmZzZXQ9Ii42OTI3Ii8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiMzNTcyNzYiIG9mZnNldD0iMSIvPgogICAgPC9saW5lYXJHcmFkaWVudD4KICAgIDxsaW5lYXJHcmFkaWVudCBpZD0icGFpbnQ5X2xpbmVhcl8yMzEyXzU0ODIiIHgxPSI1Mi41NiIgeDI9IjUyLjU2IiB5MT0iMTkuMjQiIHkyPSI1OC43OSIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjN0FDODlEIiBvZmZzZXQ9IjAiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzVEQjM5NSIgb2Zmc2V0PSIuNDU4MyIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNTRBQTk3IiBvZmZzZXQ9IjEiLz4KICAgIDwvbGluZWFyR3JhZGllbnQ+CiAgICA8bGluZWFyR3JhZGllbnQgaWQ9InBhaW50MTBfbGluZWFyXzIzMTJfNTQ4MiIgeDE9IjQ3LjYyIiB4Mj0iNTIuNjIiIHkxPSIyOC4zOCIgeTI9IjQ4LjU4IiBncmFkaWVudFVuaXRzPSJ1c2VyU3BhY2VPblVzZSI+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiM0QzlDOTMiIG9mZnNldD0iMCIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjM0Y4NTg4IiBvZmZzZXQ9Ii40MzIzIi8+CiAgICAgIDxzdG9wIHN0b3AtY29sb3I9IiMzQjdGODIiIG9mZnNldD0iLjczNDQiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzM0NzE3NSIgb2Zmc2V0PSIxIi8+CiAgICA8L2xpbmVhckdyYWRpZW50PgogICAgPGxpbmVhckdyYWRpZW50IGlkPSJwYWludDExX2xpbmVhcl8yMzEyXzU0ODIiIHgxPSI0Ni44MiIgeDI9IjQ2LjgyIiB5MT0iMjguNjIiIHkyPSI0OC41NCIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNkZDMTlDIiBvZmZzZXQ9IjAiLz4KICAgICAgPHN0b3Agc3RvcC1jb2xvcj0iIzVEQjI5NSIgb2Zmc2V0PSIuNTE1NiIvPgogICAgICA8c3RvcCBzdG9wLWNvbG9yPSIjNTZBRDk2IiBvZmZzZXQ9IjEiLz4KICAgIDwvbGluZWFyR3JhZGllbnQ+CiAgPC9kZWZzPgogIDxnIGlkPSJhcnQiPgogICAgPHBhdGggZD0ibTU3LjgzIDMuNDItNDAuNjIgMy40NmMtNi44MyAwLjU5LTExLjkyIDYuMDMtMTEuOTIgMTMuNTh2MzYuODVjMCA4LjMgNS44IDE0IDEyLjk2IDE0LjQ0bDM4LjY1IDIuNjJjMTEuMDEgMC43MiAxNy4zOC01LjY2IDE3LjM4LTEzLjk4di00MC44OGMwLTEwLjAzLTcuNzktMTYuNTgtMTYuNDUtMTYuMDl6bTcuODYgNTUuNDVjMCA1LjA5LTMuNzIgOC40OC04LjE4IDguMjJsLTM3Ljk2LTIuMzdjLTQtMC4yNS02LjgzLTMuMzQtNi44My03LjU3di0zNS44OWMwLTQuNyAzLTcuNzUgNy4xOC04LjA3bDM4LjI2LTIuOTFjNC4yOS0wLjMzIDcuNTMgMy44IDcuNTMgOS4yM3YzOS4zNnoiIGZpbGw9InVybCgjcGFpbnQxX2xpbmVhcl8yMzEyXzU0ODIpIi8+CiAgICA8cGF0aCBkPSJtNTYuNzEgMy42OS0zOS41IDMuMzdjLTYuNjcgMC41NS0xMS42OSA1Ljk5LTExLjY5IDEzLjMydjM2LjcxYzAgNy45IDUuNCAxMy4xOSAxMi43MyAxMy41NWwzOC42NSAyLjM1YzguNTcgMC42MiAxNC4wOC01LjI3IDE0LjA4LTEyLjk0di00MS4yM2MwLTkuMzQtNi43Ny0xNS41My0xNC4yNy0xNS4xM3ptOC45OCA1NS4xOGMwIDUuMDktMy43MiA4LjQ4LTguMTggOC4yMmwtMzcuOTYtMi4zN2MtNC0wLjI1LTYuODMtMy4zNC02LjgzLTcuNTd2LTM1Ljg5YzAtNC43IDMtNy43NSA3LjE4LTguMDdsMzguMjYtMi45MWM0LjI5LTAuMzMgNy41MyAzLjggNy41MyA5LjIzdjM5LjM2eiIgZmlsbD0idXJsKCNwYWludDJfbGluZWFyXzIzMTJfNTQ4MikiLz4KICAgIDxwYXRoIGQ9Im01Ni4wMyA4LjctMzguNDIgMi43NWMtNC43MiAwLjMzLTguMTYgNC4xNC04LjE2IDkuNjd2MzUuMjVjMCA1LjUgMy42NCA5LjI4IDguNjEgOS41NGwzOC45MiAyLjM4YzUuNDUgMC4zOCA5LjU0LTMuNTUgOS41NC05LjM0di0zOS4yYzAtNi42OS00LjczLTExLjQtMTAuNDktMTEuMDV6bTkuNjYgNTAuMTdjMCA1LjA5LTMuNzIgOC40OC04LjE4IDguMjJsLTM3Ljk2LTIuMzdjLTQtMC4yNS02LjgzLTMuMzQtNi44My03LjU3di0zNS44OWMwLTQuNyAzLTcuNzUgNy4xOC04LjA3bDM4LjI2LTIuOTFjNC4yOS0wLjMzIDcuNTMgMy44IDcuNTMgOS4yM3YzOS4zNnoiIGZpbGw9InVybCgjcGFpbnQzX2xpbmVhcl8yMzEyXzU0ODIpIi8+CiAgICA8cGF0aCBkPSJtNTYuNzEgNC4yN2M3LjYyLTAuMzEgMTMuNTMgNS41MiAxMy42NyAxMy44IDAtOC41Ni02LjQxLTE0LjQ2LTEzLjY3LTE0LjE3bC0zOS41IDMuMmMtMi45MyAwLjI0LTUuNDQgMS40MS03LjM1IDMuMTggMS44NC0xLjUgNC4yMy0yLjUgNy4wOC0yLjcxbDM5Ljc3LTMuM3oiIGZpbGw9IiNBMUQ3QkIiIG9wYWNpdHk9Ii41Ii8+CiAgICA8cGF0aCBkPSJtNTcuMDcgNjcuNTItMzguODYtMi40M2MtNS4wMy0wLjItOC4zMi0zLjg1LTguNDYtOC45MSAwIDUuNDMgMy40IDkuNDEgOC41IDkuNjVsMzguNzMgMi40M2M1LjQxIDAuMzUgOS4zNS0zLjQ1IDkuNTItOC44OS0wLjY1IDUuMS00LjYyIDguMzItOS40MyA4LjE1eiIgZmlsbD0iIzkzRDZBQiIgb3BhY2l0eT0iLjUiLz4KICAgIDxwYXRoIGQ9Im0zNy4zNSAyMC43NGMtMC43NC0wLjA0LTEuNDYgMC4yMS0yLjAyIDAuNzFsLTcuNjggNy4xM2MtMC4yNCAwLjIzLTAuNTYgMC4zNy0wLjkgMC4zOWwtNS41OCAwLjIxYy0yLjU1IDAuMS00LjUxIDIuMjMtNC41MSA1djcuOTljMCAyLjgxIDIuMSA0Ljk1IDQuNzQgNS4wOWw1LjMgMC4yOGMwLjI4IDAuMDIgMC41NCAwLjE1IDAuNzQgMC4zNWw3Ljk0IDguMTJjMC43MSAwLjcxIDEuNjcgMS4wNCAyLjU1IDEuMDIgMS41Mi0wLjA0IDIuNjUtMS4yIDIuNjUtMi43NnYtMzAuMThjMC0xLjg0LTEuNjUtMy4yNy0zLjIzLTMuMzV6IiBmaWxsPSJ1cmwoI3BhaW50NV9saW5lYXJfMjMxMl81NDgyKSIvPgogICAgPHBhdGggZD0ibTM4LjIxIDIwLjkxYzAuMTggMC40MSAwLjI2IDAuODQgMC4yNiAxLjM0djMxLjY5YzAgMS4zNy0wLjk2IDIuMzctMi4zIDIuNDUgMC41NCAwLjQxIDEuMjIgMC42MyAxLjg1IDAuNiAxLjQ5LTAuMDYgMi41Ni0xLjIyIDIuNTYtMi43MnYtMzAuMThjMC0xLjU4LTEuMDgtMi44LTIuMzctMy4xOHoiIGZpbGw9InVybCgjcGFpbnQ2X2xpbmVhcl8yMzEyXzU0ODIpIi8+CiAgICA8cGF0aCBkPSJtMzcuNTggMjEuMThjLTAuNjctMC4yMi0xLjUxLTAuMDMtMi4xMSAwLjUxbC03LjU5IDcuMjJjLTAuMjggMC4yNS0wLjY1IDAuMzctMS4wMiAwLjM4bC01LjY3IDAuMmMtMi40NSAwLjA5LTQuNCAyLjA5LTQuNCA0Ljc0djcuODljMCAxLjU4IDAuNzIgMyAxLjggMy44OS0wLjU3LTAuODEtMC45LTEuODItMC45LTIuOTd2LTguODNjMC0xLjE2IDAuNjktMS43NyAxLjUzLTEuNjYgMC42NiAwLjExIDEuMTctMC4yMyAxLjc5LTAuMTZsNy4zMy0wLjA4YzAuNTQgMC4wMSAxLjAyLTAuMTcgMS4zOC0wLjUzbDcuOS04LjU5YzAuODItMC44MSAwLjgtMS43Mi0wLjA0LTIuMDF6IiBmaWxsPSJ1cmwoI3BhaW50N19saW5lYXJfMjMxMl81NDgyKSIgb3BhY2l0eT0iLjUiLz4KICAgIDxwYXRoIGQ9Im0yOC4wNiA0My41MS03LjA1LTAuNGMtMC44OS0wLjA1LTEuNTItMC43NC0xLjUyLTEuNzV2MC42OWMwIDIuMDcgMC42IDMuOTYgMS44NyA0LjZsNS41NiAwLjMyYzAuNDYgMC4wNiAwLjgzIDAuMjggMS4xNSAwLjU2bDcuNjkgNy45OGMwLjg5LTAuNTIgMS4yMi0xLjgyIDAuNjMtMi42MWwtNi45LTguNzVjLTAuMzctMC4zOC0wLjg3LTAuNjItMS40My0wLjY0eiIgZmlsbD0iIzREQTA5NyIgb3BhY2l0eT0iLjUiLz4KICAgIDxwYXRoIGQ9Im00OS42MSAxOS40M2MtMS41LTAuNzQtMy4zMi0wLjM3LTQuMTEgMS4wNC0wLjggMS40NS0wLjIxIDMuMTMgMS4yNyAzLjk3IDUuMjUgMi44MSA4LjM5IDguMDQgOC4zOSAxNC40NCAwIDYuNDctMy40NSAxMi4wNS04LjU3IDE0LjYxLTEuNDcgMC44MS0yIDIuNDctMS4zMyAzLjg1IDAuNzIgMS40IDIuNzggMi4wMyA0LjQ0IDEuNDEgNy4xMy0zLjE0IDExLjk3LTEwLjc5IDExLjk3LTE5LjU3IDAtOC42Ny01LjAyLTE2LjQ4LTEyLjA2LTE5Ljc1eiIgZmlsbD0idXJsKCNwYWludDhfbGluZWFyXzIzMTJfNTQ4MikiLz4KICAgIDxwYXRoIGQ9Im00OS4yOCAxOS41OWMtMS4zNy0wLjY5LTMtMC4zMy0zLjczIDAuOTQtMC43MSAxLjI3LTAuMjEgMi43NSAxLjE2IDMuNDYgNS41IDIuODkgOC44NyA4LjQyIDguODcgMTQuODkgMCA2LjUzLTMuNDUgMTIuMzctOC45MSAxNC44Mi0xLjQxIDAuNzctMS45IDIuMjktMS4yNCAzLjQ5IDAuNjkgMS4yNCAyLjQxIDEuNTggMy44MiAwLjkgNi40Ny0zLjUxIDEwLjQtMTAuOTIgMTAuNC0xOS4zNSAwLTguNDItNC4zOC0xNS43My0xMC4zNy0xOS4xNXoiIGZpbGw9InVybCgjcGFpbnQ5X2xpbmVhcl8yMzEyXzU0ODIpIi8+CiAgICA8cGF0aCBkPSJtNDkuMTUgMTkuNzhjLTEuMTItMC41OS0yLjU1LTAuMjUtMy4xOCAwLjc1LTAuNTkgMC45OS0wLjMgMi4yNCAwLjY3IDIuODUtMC45Ny0wLjcxLTEuMDUtMi0wLjMyLTIuODYgMC42Ni0wLjgyIDEuODktMS4wMyAyLjk5LTAuNTYgNS41NCAyLjkyIDkuMTggOC42OCAxMC4wNyAxNC45MS0wLjY2LTYuOTEtNC42OS0xMi40LTEwLjIzLTE1LjA5eiIgZmlsbD0iIzk3RDZCNyIgb3BhY2l0eT0iLjUiLz4KICAgIDxwYXRoIGQ9Im01NS41MSAzOC40NmMwLTYuNDgtMy4zMi0xMi4xOS04Ljg0LTE0Ljk0IDUuMTIgMi44MSA4LjQ1IDguNDkgOC40NSAxNS4xMyAwIDYuNS0zLjQyIDEyLjQ0LTguNTIgMTUuMDIgNS4zNy0yLjQ1IDkuMDgtOC4yNiA4LjkxLTE1LjIxeiIgZmlsbD0iIzk3RDZCNyIgb3BhY2l0eT0iLjUiLz4KICAgIDxwYXRoIGQ9Im00Ni43NiA1My43OGMtMS4xNiAwLjYxLTEuNiAxLjkyLTEuMDcgMy4wMS0wLjMyLTEuMDUgMC4xLTIuMTggMS4yMy0yLjg0LTAuMDYtMC4wNy0wLjEyLTAuMTMtMC4xNi0wLjE3eiIgZmlsbD0iIzk3RDZCNyIgb3BhY2l0eT0iLjUiLz4KICAgIDxwYXRoIGQ9Im00Ni4zOCAyOC43N2MtMS4yNy0wLjc1LTIuOTYtMC40Mi0zLjY3IDAuODktMC43NSAxLjI4LTAuMyAyLjg2IDAuODYgMy43OCAxLjczIDEuMzMgMi43NyAzLjI2IDIuNzcgNS40OCAwIDIuMjctMS4xNCA0LjI4LTIuODMgNS40NS0xLjIgMC45LTEuNDggMi41My0wLjcgMy43OCAwLjg0IDEuMjMgMi41NSAxLjc0IDMuOTUgMS4xNiAzLjQ5LTEuNzggNS41NC01LjU2IDUuNTQtOS44OSAwLTQuNjItMi40OS04Ljc5LTUuOTItMTAuNjV6IiBmaWxsPSJ1cmwoI3BhaW50MTBfbGluZWFyXzIzMTJfNTQ4MikiLz4KICAgIDxwYXRoIGQ9Im00Ni4wNCAyOC44OWMtMS4xOC0wLjctMi42NS0wLjM1LTMuMzQgMC44My0wLjY3IDEuMTUtMC4yNyAyLjU5IDAuODEgMy40MyAxLjgxIDEuMzYgMi44NSAzLjM5IDIuODUgNS43MyAwIDIuMzYtMS4xNyA0LjQ4LTIuODcgNS42Ni0xLjExIDAuODEtMS4zNiAyLjI2LTAuNjQgMy4yNSAwLjczIDAuOTEgMi4wNCAxLjA5IDMuMDkgMC40NiAzLjA0LTIuMDYgNC43NS01LjYzIDQuNzUtOS42NiAwLTQuMDYtMS44Ni03LjgyLTQuNjUtOS43eiIgZmlsbD0idXJsKCNwYWludDExX2xpbmVhcl8yMzEyXzU0ODIpIi8+CiAgICA8cGF0aCBkPSJtNDYuMTEgMjkuMjFjLTEuMDctMC43MS0yLjQ2LTAuNDQtMy4xMiAwLjU3LTAuNTYgMC45NS0wLjM0IDIuMTkgMC41MiAyLjg5LTAuNzMtMC43NS0wLjc2LTItMC4xLTIuODMgMC42OC0wLjgyIDEuOC0wLjk2IDIuODQtMC40NiAyLjY2IDEuODQgNC4yNyA1LjA3IDQuNDIgOC41MSAwLTMuNTMtMS43My02Ljk5LTQuNTYtOC42OHoiIGZpbGw9IiM5N0Q2QjciIG9wYWNpdHk9Ii41Ii8+CiAgICA8cGF0aCBkPSJtNDYuODUgMzguNzdjMCAyLjcxLTEuMzMgNC45My0zLjMyIDYuMTktMC45NCAwLjc1LTEuMDYgMS45NC0wLjQ0IDIuODEtMC41OC0wLjk3LTAuMTgtMi4yNiAwLjc5LTIuOTQgMS45Ni0xLjM1IDMuMTEtMy41NiAyLjk3LTYuMDZ6IiBmaWxsPSIjOTdENkI3IiBvcGFjaXR5PSIuNSIvPgogIDwvZz4KPC9zdmc+Cg==";

    const en = {
      "panel.label": "CopySpeak",
      "panel.title": "CopySpeak",
      "panel.intro": "Reads assistant replies aloud through the CopySpeak desktop app.",
      "status.checking": "Checking for CopySpeak…",
      "status.online": "CopySpeak is running.",
      "status.offline": "CopySpeak is not running. Start it to read replies aloud.",
      "autoRead.title": "Read new replies aloud",
      "autoRead.description":
        "CopySpeak reads each reply in the open session once its turn completes.",
      "captions.title": "Show captions while reading",
      "captions.description": "Highlights each word in the reply as CopySpeak plays it.",
      "animation.title": "Animate caption highlight",
      "animation.description":
        "Glides the highlight from word to word. Turn off to jump between words.",
      "follow.title": "Follow playback",
      "follow.description": "Scrolls the chat to keep the spoken word in view.",
      "bar.label": "CopySpeak playback",
      "bar.pause": "Pause reading",
      "bar.resume": "Resume reading",
      "bar.back5": "Back 5 seconds",
      "bar.forward5": "Forward 5 seconds",
      "bar.stop": "Stop reading",
      "bar.showInChat": "Show in chat",
      "switch.on": "On",
      "switch.off": "Off",
      "action.read": "Read aloud with CopySpeak",
      "action.stop": "Stop reading",
      "selection.quote": "Quote text",
      "selection.read": "Read aloud",
      "error.empty": "Nothing to read aloud",
      "error.offline": "CopySpeak is not running",
      "error.failed": "CopySpeak request failed",
      "error.selection": "The selection is gone"
    };

    const PLUGIN_STYLE = [
      ".dsh-csva-glyph{display:block}",
      ".dsh-csva-root{position:relative;display:inline-flex;align-items:center;gap:8px}",
      /* The host action-row icon button (ui-chat MessageIconActions.module.css): a
         28 px square hit area, 17 px glyph, tertiary icon color that lifts on hover.
         The read action sits in the same row, so it copies these metrics. */
      ".dsh-csva-button{width:calc(28px + var(--dsh-content-font-delta,0px));height:calc(28px + var(--dsh-content-font-delta,0px));border-radius:var(--dsw-radius-sm,6px);border:none;background:0 0;padding:6px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:var(--dsw-alias-label-tertiary);}",
      ".dsh-csva-button svg{width:calc(17px + var(--dsh-content-font-delta,0px));height:calc(17px + var(--dsh-content-font-delta,0px))}",
      "@media (hover:hover){.dsh-csva-button:hover:not(:disabled){background:var(--dsw-alias-interactive-bg-hover);color:var(--dsw-alias-label-secondary)}}",
      /* While the message plays, the button shows the filled mark in the brand color. */
      ".dsh-csva-button[data-active]{color:var(--dsw-alias-brand-primary)}",
      ".dsh-csva-button:focus-visible{outline:var(--dsw-focus-ring-width,2px) solid var(--dsw-focus-ring-color,var(--dsw-alias-brand-primary));outline-offset:2px}",
      ".dsh-csva-button:disabled{opacity:.4;cursor:default}",
      /* The host Tooltip bubble (ui-primitives Tooltip): fixed above the page, solid
         tooltip fill, inherited font, a 150 ms fade. The plugin owns its own copy of
         the look because a plain-JS plugin cannot import the host primitives. */
      ".dsh-csva-tip{position:fixed;z-index:100;display:inline-flex;align-items:center;gap:8px;width:max-content;max-width:min(50vw,calc(100vw - 24px));box-sizing:border-box;padding:3px 7px;border-radius:var(--dsw-radius-sm,6px);background:var(--dsw-alias-tooltip-bg,#1f2329);color:var(--dsw-static-neutral-bluish-00,#fff);font-size:13px;line-height:20px;white-space:pre-line;overflow-wrap:break-word;pointer-events:none;transform:translateX(-50%);animation:dsh-csva-tip-in .15s var(--ds-ease-in-out,ease)}",
      ".dsh-csva-tip[data-side=top]{transform:translate(-50%,-100%)}",
      "@keyframes dsh-csva-tip-in{0%{opacity:0}to{opacity:1}}",
      "@media (prefers-reduced-motion:reduce){.dsh-csva-tip{animation:none}}",
      ".dsh-csva-note{font-size:12px;line-height:16px;color:var(--dsw-alias-state-error-primary);white-space:nowrap}",
      ".dsh-csva-panel{box-sizing:border-box;width:100%;height:100%;padding:calc(var(--dsh-frame-top-clearance,0px) + 28px) clamp(24px,4vw,48px) 48px;color:var(--dsw-alias-label-primary);background:var(--dsw-alias-bg-base);display:flex;flex-direction:column;align-items:center;gap:20px;overflow:hidden auto}",
      ".dsh-csva-panel>*{flex:none;width:100%;max-width:720px}",
      ".dsh-csva-heading{display:flex;flex-direction:column;gap:6px}",
      ".dsh-csva-title{margin:0;font-size:20px;line-height:28px;font-weight:600}",
      ".dsh-csva-intro{margin:0;font-size:13px;line-height:20px;color:var(--dsw-alias-label-secondary)}",
      ".dsh-csva-status{display:inline-flex;align-items:center;gap:8px;margin:0;color:var(--dsw-alias-label-secondary);font-size:13px;line-height:20px}",
      ".dsh-csva-dot{flex:none;width:8px;height:8px;border-radius:50%;background:var(--dsw-alias-state-idle-primary)}",
      ".dsh-csva-status[data-state=online] .dsh-csva-dot{background:var(--dsw-alias-state-success-primary)}",
      ".dsh-csva-status[data-state=offline] .dsh-csva-dot{background:var(--dsw-alias-state-error-primary)}",
      ".dsh-csva-rows{display:flex;flex-direction:column;gap:10px;margin:0;padding:0;list-style:none}",
      ".dsh-csva-row{display:flex;align-items:center;gap:16px;padding:12px 14px;border:.5px solid var(--dsw-alias-border-l1);border-radius:var(--dsw-radius-xl,10px);background:var(--dsw-alias-bg-layer-1)}",
      ".dsh-csva-rowText{display:flex;flex-direction:column;gap:2px;min-width:0;flex:1}",
      ".dsh-csva-rowTitle{font-size:13px;line-height:20px;color:var(--dsw-alias-label-primary)}",
      ".dsh-csva-rowDescription{font-size:12px;line-height:18px;color:var(--dsw-alias-label-secondary)}",
      ".dsh-csva-switch{position:relative;flex:none;width:36px;height:20px;padding:0;border:none;border-radius:999px;background:var(--dsw-alias-state-idle-primary);cursor:pointer;transition:background .15s}",
      ".dsh-csva-switch[data-on]{background:var(--dsw-alias-brand-primary)}",
      ".dsh-csva-switch:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:2px}",
      ".dsh-csva-switchThumb{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;background:var(--dsw-alias-bg-base);transition:transform .15s}",
      ".dsh-csva-switch[data-on] .dsh-csva-switchThumb{transform:translateX(16px)}",
      /* The frame-wide playback bar. The shell overlay is click-through, so the
         bar opts back into pointer events. It floats in the top right corner,
         below the session header's bottom border: the header is 76 px tall and
         starts under the frame top clearance. The bar is icon-only: the colored
         app logo, then pause or resume, rewind, skip, stop, and Show in chat.
         Every control names itself in its tooltip and aria-label. */
      ".dsh-csva-bar{position:fixed;right:20px;top:calc(var(--dsh-frame-top-clearance,0px) + 88px);z-index:20;display:inline-flex;align-items:center;gap:2px;box-sizing:border-box;max-width:calc(100vw - 40px);padding:6px 6px 6px 8px;border:.5px solid var(--dsw-alias-border-l1);border-radius:12px;background:var(--dsw-alias-bg-overlay);color:var(--dsw-alias-label-primary);box-shadow:0 6px 20px rgb(0 0 0 / 14%);pointer-events:auto}",
      ".dsh-csva-barGlyph{flex:none;display:inline-flex;align-items:center;justify-content:center;width:18px;height:18px;margin-right:6px}",
      ".dsh-csva-barLogo{display:block;width:18px;height:18px}",
      ".dsh-csva-barControl{flex:none;display:inline-flex;align-items:center;justify-content:center;width:26px;height:26px;padding:0;border:none;border-radius:var(--dsw-radius-sm,6px);background:0 0;color:var(--dsw-alias-label-secondary);cursor:pointer}",
      "@media (hover:hover){.dsh-csva-barControl:hover{background:var(--dsw-alias-interactive-bg-hover);color:var(--dsw-alias-label-primary)}}",
      ".dsh-csva-barControl:focus-visible{outline:var(--dsw-focus-ring-width,2px) solid var(--dsw-focus-ring-color,var(--dsw-alias-brand-primary));outline-offset:2px}",
      ".dsh-csva-barStop{color:var(--dsw-alias-label-primary)}",
      "::highlight(" + HIGHLIGHT_NAME + "){color:var(--dsw-alias-bg-base)}",
      /* During the animated glide the outgoing and incoming words carry their
         own names, so their text colors can be tweened frame by frame in
         lockstep with the gliding pill. The custom properties default to the
         settled knockout color, so an unset tween paints the same as the
         settled highlight. */
      /* The fallbacks are the plain text color: a custom property is absent
         only in an unmanaged gap (cancel, re-render), where plain text is
         safe. The old knockout fallback painted dark-on-dark in that gap. */
      "::highlight(" + HIGHLIGHT_NAME_OUT + "){color:var(--dsh-csva-out-color,currentColor)}",
      "::highlight(" + HIGHLIGHT_NAME_IN + "){color:var(--dsh-csva-in-color,currentColor)}",
      /* Code chips go transparent inside the reply: their own background
         paints over the z-index:-1 mark layer, so a spoken code word could
         never wear the rounded pill. With the chip see-through, code words
         run the same mark path as plain words. The declaration is marked
         !important because the host re-asserts an opaque background on
         shiki-highlighted code blocks (pre._shiki) with !important of its
         own; a plain declaration loses to it and the block would hide the
         pill under its background (dark-on-dark knockout text). */
      "[data-text-interaction-source] code, [data-text-interaction-source] pre" +
        "{background-color:transparent!important}",
      /* The rounded mark: a positioned layer inside the reply's own stacking
         context. Negative z-index keeps it behind the text, so the word wears
         the highlight's recolored text over a rounded brand pill. */
      "[data-dsh-csva-mark-host]{isolation:isolate}",
      ".dsh-csva-markLayer{position:absolute;top:0;left:0;width:0;height:0;z-index:-1;pointer-events:none}",
      ".dsh-csva-mark{position:absolute;border-radius:4px;background-color:var(--dsw-alias-brand-primary);pointer-events:none}",
      /* Animated glide: the mark layer carries data-animate while the
         animation switch is on, so the reading tweens the pill position and
         the two words' text colors in one rAF loop. The blend windows follow
         the pill's coverage of each word: the outgoing word keeps its
         knockout until the pill slides off it, and the incoming word keeps
         its plain color until the pill reaches it, so neither word darkens
         over the empty background mid-glide. Off keeps the instant jump. */
      /* The text-selection surface holds one pill group: Quote and Read sit
         inside one rounded floating container, separated from the error note.
         The group copies the host Quote pill fill (floating fill, elevation
         stroke, pill radius), because a plain-JS plugin cannot import the host
         CSS module. Each action is a flat segment; the hover fills one
         segment. */
      ".dsh-csva-selection{display:inline-flex;align-items:center;gap:8px;max-width:100%}",
      ".dsh-csva-selectionGroup{display:inline-flex;align-items:center;max-width:100%;border-radius:999px;padding:2px;background:var(--dsw-alias-button-floating-fill);--dsw-elevation-stroke-color:var(--dsw-alias-border-l3);box-shadow:var(--dsw-elevation-panel)}",
      ".dsh-csva-selectionAction{corner-shape:round;display:inline-flex;align-items:center;gap:5px;border:none;border-radius:999px;padding:3px 10px;background:0 0;color:var(--dsw-alias-label-primary);font:var(--dsw-font-xs-13);white-space:nowrap;cursor:pointer}",
      "@media (hover:hover){.dsh-csva-selectionAction:hover:not(:disabled){background:var(--dsw-alias-button-floating-hover)}}",
      ".dsh-csva-selectionAction:focus-visible{outline:var(--dsw-focus-ring-width,2px) solid var(--dsw-focus-ring-color,var(--dsw-alias-brand-primary));outline-offset:2px}",
      ".dsh-csva-selectionAction:disabled{opacity:.4;cursor:default}",
      ".dsh-csva-selectionNote{font-size:12px;line-height:16px;color:var(--dsw-alias-state-error-primary);white-space:nowrap}"
    ].join("\n");

    /* One reading at a time; every surface observes this record. `paused` is
       the app's pause state, mirrored for the bar, and an optimistic flip waits
       PAUSE_RECONCILE_MS before a poll may override it. */
    const playback = {
      messageId: null,
      requestId: null,
      /* Which surface owns the current reading: "message", "selection", or
         null when idle. */
      source: null,
      paused: false,
      pauseChangedAt: 0,
      listeners: new Set(),
      sequence: 1
    };

    /* The locale binder from apply(), for slots that pass no `t` prop. */
    let boundT = null;

    function emit(listeners) {
      for (const listener of Array.from(listeners)) listener();
    }

    function subscribe(listeners, listener) {
      listeners.add(listener);
      return function () {
        listeners.delete(listener);
      };
    }

    function remember(messageId, requestId, source) {
      playback.messageId = messageId;
      playback.requestId = requestId;
      playback.source = source;
      playback.paused = false;
      playback.pauseChangedAt = 0;
      emit(playback.listeners);
    }

    function forget() {
      playback.messageId = null;
      playback.requestId = null;
      playback.source = null;
      playback.paused = false;
      playback.pauseChangedAt = 0;
      stopStatusPolling();
      resetReading(null, null, "");
      emit(playback.listeners);
    }

    function setPlaybackPaused(paused) {
      if (playback.paused === paused) return;
      playback.paused = paused;
      emit(playback.listeners);
    }

    function nextRequestId() {
      const stamp = Date.now().toString(36);
      const sequence = playback.sequence.toString(36);
      playback.sequence += 1;
      return "dsh." + stamp + "." + sequence;
    }

    /* User preferences: page-local, so a switch keeps its state across
       reloads without a Host round trip. Captions start off; follow and the
       caption animation start on. */
    const preferences = {
      autoRead: readStoredFlag(STORAGE.autoRead, false),
      captions: readStoredFlag(STORAGE.captions, false),
      follow: readStoredFlag(STORAGE.follow, true),
      animation: readStoredFlag(STORAGE.animation, true),
      listeners: new Set()
    };

    function readStoredFlag(key, fallback) {
      try {
        const stored = window.localStorage.getItem(key);
        if (stored === null) return fallback;
        return stored === "1";
      } catch {
        return fallback;
      }
    }

    function setPreference(name, enabled) {
      preferences[name] = enabled;
      try {
        window.localStorage.setItem(STORAGE[name], enabled ? "1" : "0");
      } catch {
        /* Private mode: the switch still works for this page. */
      }
      /* The status poll runs for the whole reading, so the bar keeps its pause
         state and Show in chat keeps its target. Disabling captions only drops
         the inline highlight. */
      if (name === "captions" && !enabled) {
        clearHighlight();
        releaseMarkHost();
      }
      if (name === "animation") syncMarkAnimation();
      emit(preferences.listeners);
    }

    /* ---------- Rendered-message word highlight ---------- */

    /*
     * The word highlight paints inside the assistant reply itself: the Chat
     * view marks each settled Markdown body with `data-text-interaction-source`,
     * and each Chat row carries `data-chat-node-key` (the durable node key the
     * Chat snapshot also exposes) plus `data-chat-flow-kind`.
     *
     * Caption word offsets index the fragment text CopySpeak spoke, not the
     * rendered Markdown. Sanitizers and pagination change the two strings, so
     * offsets cannot transfer directly. The reader instead aligns the fragment's
     * normalized words onto the rendered text nodes once per fragment, then
     * paints the active word with the CSS Custom Highlight API.
     */
    const WORD_SPAN = /\S+/gu;
    const WORD_CHAR = /[\p{L}\p{N}]/u;
    const ROW_SELECTOR = '[data-chat-flow-kind="assistant-step"]';
    const SOURCE_SELECTOR = "[data-text-interaction-source]";
    /* A double-click expands the selection to the paragraph that holds the
       double-clicked word, so the host pill opens for the whole paragraph.
       The paragraph is the innermost block inside the reply body; a blockless
       body falls back to the body itself. */
    const PARAGRAPH_SELECTOR =
      "p, li, h1, h2, h3, h4, h5, h6, pre, blockquote, dd, dt, figcaption, td, th";

    /* Current reading and its resolved DOM target. */
    const reading = {
      messageId: null,
      nodeKey: null,
      /* The cloned browser range a selection reading highlights. A message
         reading leaves it null and resolves its target from the node key. */
      range: null,
      /* The read-selection action exists to show its words, so it paints them
         even when the Show captions switch is off. */
      forceCaptions: false,
      speechTokens: [],
      element: null,
      tokens: null,
      cursor: 0,
      fragmentText: null,
      fragmentWords: null,
      fragmentEntries: 0,
      fragmentMap: null,
      fragmentPosition: 0,
      /* The token start of every fragment already aligned in this reading,
         keyed by the app's fragment index (its text when an older app reports
         none). A cross-fragment rewind re-aligns from the recorded start. */
      fragmentStarts: new Map(),
      fragmentIndex: null,
      /* Where the current fragment starts and ends in the rendered tokens.
         Fresh synthesis streams one fragment as a growing prefix, so its
         reports align from the start again, not from the previous, shorter one. */
      fragmentStart: 0,
      fragmentEnd: 0,
      scroller: null,
      watchedScroller: null,
      activeToken: -1,
      /* The rounded mark: the element that hosts its stacking context, the
         layer, and one positioned mark per line of the spoken word. */
      markHost: null,
      markLayer: null,
      markRects: [],
      markContaining: null,
      /* The settled highlighted word: its token is where the highlight name
         points between glides, and where the next glide fades out from. */
      paintedToken: null,
      /* The running text-color tween, in lockstep with one pill glide. */
      tween: null,
      /* A hidden probe that resolves theme variables to concrete rgb colors. */
      colorProbe: null,
      lastScrollAt: 0,
      followPausedUntil: 0
    };

    /* Lowercased letters and digits only, so punctuation and wrapping do not
     * split a match between the spoken text and the rendered text. */
    function normalizeWord(raw) {
      return raw.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
    }

    /* The first and last letter or digit of a whitespace-free run. */
    function wordSpan(raw) {
      let start = 0;
      let end = raw.length;
      while (start < end && !WORD_CHAR.test(raw[start])) start += 1;
      while (end > start && !WORD_CHAR.test(raw[end - 1])) end -= 1;
      return { start: start, end: end };
    }

    function tokensFromText(text) {
      const tokens = [];
      for (const match of text.matchAll(WORD_SPAN)) {
        const norm = normalizeWord(match[0]);
        if (norm !== "") tokens.push({ norm: norm });
      }
      return tokens;
    }

    function tokensFromElement(element) {
      const tokens = [];
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
      let node = walker.nextNode();
      while (node !== null) {
        const text = node.nodeValue ?? "";
        /* A code chip paints its own background over the reply, and the mark
           layer sits behind the reply. Flag the word, so paintWord can use the
           code highlight instead of the mark. */
        const inCode =
          node.parentElement !== null && node.parentElement.closest("code, pre") !== null;
        for (const match of text.matchAll(WORD_SPAN)) {
          const norm = normalizeWord(match[0]);
          if (norm === "") continue;
          const span = wordSpan(match[0]);
          tokens.push({
            node: node,
            start: match.index + span.start,
            end: match.index + span.end,
            norm: norm,
            code: inCode
          });
        }
        node = walker.nextNode();
      }
      return tokens;
    }

    /* The words inside one cloned selection range, in document order. The owner
       admits a selection only when both range ends sit in one
       text-interaction source, so the range covers one rendered reply body. */
    function tokensFromRange(range) {
      const tokens = [];
      const root = range.commonAncestorContainer;
      if (!root.isConnected) return tokens;
      const area = root.nodeType === Node.TEXT_NODE ? root.parentElement : root;
      if (area === null) return tokens;
      const walker = document.createTreeWalker(area, NodeFilter.SHOW_TEXT);
      let node = walker.nextNode();
      while (node !== null) {
        if (!range.intersectsNode(node)) {
          node = walker.nextNode();
          continue;
        }
        const text = node.nodeValue ?? "";
        const first = node === range.startContainer ? range.startOffset : 0;
        const last = node === range.endContainer ? range.endOffset : text.length;
        if (last > first) {
          const inCode =
            node.parentElement !== null && node.parentElement.closest("code, pre") !== null;
          const slice = text.slice(first, last);
          for (const match of slice.matchAll(WORD_SPAN)) {
            const norm = normalizeWord(match[0]);
            if (norm === "") continue;
            const span = wordSpan(match[0]);
            const base = first + match.index;
            tokens.push({
              node: node,
              start: base + span.start,
              end: base + span.end,
              norm: norm,
              code: inCode
            });
          }
        }
        node = walker.nextNode();
      }
      return tokens;
    }

    function tokenCounts(tokens) {
      const counts = new Map();
      for (const token of tokens) counts.set(token.norm, (counts.get(token.norm) ?? 0) + 1);
      return counts;
    }

    /* How many spoken words appear in the rendered text, as a bag of words. */
    function overlapScore(tokens, spokenTokens) {
      const remaining = tokenCounts(tokens);
      let score = 0;
      for (const token of spokenTokens) {
        const count = remaining.get(token.norm) ?? 0;
        if (count === 0) continue;
        remaining.set(token.norm, count - 1);
        score += 1;
      }
      return score;
    }

    function mappedCount(map) {
      let count = 0;
      for (const index of map) if (index >= 0) count += 1;
      return count;
    }

    function nearestMapped(map, index) {
      for (let distance = 0; distance <= 3; distance += 1) {
        const before = index - distance;
        if (before >= 0 && map[before] >= 0) return map[before];
        const after = index + distance;
        if (after < map.length && map[after] >= 0) return map[after];
      }
      return -1;
    }

    /* Match the fragment words that surround one confirmed anchor token. */
    function matchAround(fragmentWords, tokens, anchor, anchorToken) {
      const map = fragmentWords.map(function () {
        return -1;
      });
      map[anchor] = anchorToken;
      let token = anchorToken - 1;
      for (let index = anchor - 1; index >= 0; index -= 1) {
        const norm = fragmentWords[index].norm;
        if (norm === "") continue;
        const lowest = Math.max(0, token - ALIGN_WINDOW);
        for (let scan = token; scan >= lowest; scan -= 1) {
          if (tokens[scan].norm !== norm) continue;
          map[index] = scan;
          token = scan - 1;
          break;
        }
      }
      token = anchorToken + 1;
      for (let index = anchor + 1; index < fragmentWords.length; index += 1) {
        const norm = fragmentWords[index].norm;
        if (norm === "") continue;
        const highest = Math.min(tokens.length, token + ALIGN_WINDOW);
        for (let scan = token; scan < highest; scan += 1) {
          if (tokens[scan].norm !== norm) continue;
          map[index] = scan;
          token = scan + 1;
          break;
        }
      }
      return { map: map, score: mappedCount(map) };
    }

    /* Map one caption fragment onto the rendered tokens. Playback runs forward,
     * so the fragment starts at the last matched position; a fragment that
     * cannot be found stays unhighlighted. */
    function alignFragment(fragmentWords, tokens, from) {
      let anchor = -1;
      let expected = 0;
      for (let index = 0; index < fragmentWords.length; index += 1) {
        if (fragmentWords[index].norm === "") continue;
        expected += 1;
        if (anchor < 0) anchor = index;
      }
      if (anchor < 0) return null;
      const norm = fragmentWords[anchor].norm;
      let best = null;
      let scan = from;
      for (let attempt = 0; attempt < ALIGN_ATTEMPTS; attempt += 1) {
        let found = -1;
        for (let index = scan; index < tokens.length; index += 1) {
          if (tokens[index].norm !== norm) continue;
          found = index;
          break;
        }
        if (found < 0) break;
        const trial = matchAround(fragmentWords, tokens, anchor, found);
        if (best === null || trial.score > best.score) best = trial;
        if (best.score >= expected) break;
        if (best.score >= Math.ceil(expected * 0.8)) break;
        scan = found + 1;
      }
      if (best === null) return null;
      const required = expected <= 1 ? 1 : Math.max(2, Math.ceil(expected * 0.5));
      return best.score >= required ? best.map : null;
    }

    /* Provider caption entries are not words: ElevenLabs sends one entry per
     * character, other engines send whole words. Group the entries under the
     * words the reply renders, taking the first start and the last end each
     * word touches, so one window works for either granularity. */
    function captionWords(snapshot) {
      const words = [];
      for (const match of snapshot.text.matchAll(WORD_SPAN)) {
        const span = wordSpan(match[0]);
        const norm = normalizeWord(match[0].slice(span.start, span.end));
        if (norm === "") continue;
        /* Group by the whole whitespace run: a word-level provider reports a
           trailing punctuation mark with its word, so both granularities give
           the same window. */
        const start = match.index;
        const end = start + match[0].length;
        let startMs = Infinity;
        let endMs = -Infinity;
        for (const entry of snapshot.words) {
          if (entry.text_end <= start || entry.text_start >= end) continue;
          if (entry.text_end <= entry.text_start || entry.end_ms <= entry.start_ms) continue;
          if (entry.start_ms < startMs) startMs = entry.start_ms;
          if (entry.end_ms > endMs) endMs = entry.end_ms;
        }
        if (startMs === Infinity || endMs <= startMs) continue;
        words.push({ norm: norm, text: match[0], start_ms: startMs, end_ms: endMs });
      }
      return words;
    }

    function activeWordIndex(words, position) {
      for (let cursor = 0; cursor < words.length; cursor += 1) {
        if (position >= words[cursor].start_ms && position < words[cursor].end_ms) return cursor;
      }
      return -1;
    }

    function findScroller(element) {
      let node = element.parentElement;
      while (node !== null) {
        const style = window.getComputedStyle(node);
        if (
          (style.overflowY === "auto" || style.overflowY === "scroll") &&
          node.scrollHeight > node.clientHeight
        ) {
          return node;
        }
        node = node.parentElement;
      }
      return null;
    }

    /* Syntax highlighters split one code word across several text nodes, so
       the per-node tokens never match the spoken word ("keep" + "_mark" vs
       "keepmark") and the highlight waits on the next plain word. Merge
       adjacent code tokens while their concatenated norm exists in the spoken
       stream: the merge can only create a spoken match, never break one. The
       merged token spans its nodes with endNode/end for painting. */
    function mergeCodeSplits(tokens, speechNorms) {
      if (speechNorms === undefined || speechNorms === null || speechNorms.size === 0) {
        return tokens;
      }
      const out = [];
      let index = 0;
      while (index < tokens.length) {
        const first = tokens[index];
        if (first.code !== true) {
          out.push(first);
          index += 1;
          continue;
        }
        let merged = first;
        let cursor = index + 1;
        while (cursor < tokens.length) {
          const next = tokens[cursor];
          if (next.code !== true) break;
          const joined = merged.norm + next.norm;
          if (!speechNorms.has(joined)) break;
          /* Adjacent means no whitespace between the two pieces: a bridge
             range reads the gap text between them. */
          const bridge = document.createRange();
          bridge.setStart(merged.endNode ?? merged.node, merged.end);
          bridge.setEnd(next.node, next.start);
          if (/\s/u.test(bridge.toString())) break;
          merged = {
            node: first.node,
            start: first.start,
            end: next.end,
            endNode: next.node,
            norm: joined,
            code: true
          };
          cursor += 1;
        }
        out.push(merged);
        index = cursor;
      }
      return out;
    }

    /* One reply can render several Markdown bodies (text before and after a
     * tool call). Read them as one ordered word list, so a fragment that spans
     * two bodies still aligns. */
    function targetFrom(sources) {
      if (sources.length === 0) return null;
      const tokens = [];
      for (const source of sources) {
        for (const token of mergeCodeSplits(tokensFromElement(source), reading.speechNorms)) {
          tokens.push(token);
        }
      }
      if (tokens.length === 0) return null;
      return {
        element: sources[0],
        tokens: tokens,
        score: overlapScore(tokens, reading.speechTokens)
      };
    }

    function rowSources(row) {
      const sources = [];
      for (const source of row.querySelectorAll(SOURCE_SELECTOR)) {
        /* Expanded reasoning carries its own source under the same row; the
           spoken reply must not align against reasoning words. */
        if (source.closest('[data-variant="think"]') !== null) continue;
        sources.push(source);
      }
      return sources;
    }

    /* Resolve the rendered reply. A node key is exact; without one, the DOM is
     * the only address and the closest assistant row wins. */
    function findMessageTarget() {
      const rows = document.querySelectorAll(ROW_SELECTOR);
      if (reading.nodeKey !== null) {
        /* The key is authoritative. A missing row means the reply is not on
           screen (another Session, or not rendered), so nothing is painted. */
        const sources = [];
        for (const row of rows) {
          if (row.getAttribute("data-chat-node-key") !== reading.nodeKey) continue;
          for (const source of rowSources(row)) sources.push(source);
        }
        const exact = targetFrom(sources);
        return exact !== null && exact.score > 0 ? exact : null;
      }
      /* Without a key the DOM is the only address, so score every assistant row. */
      let best = null;
      for (const row of rows) {
        const candidate = targetFrom(rowSources(row));
        if (candidate === null || candidate.score < 3) continue;
        if (best === null || candidate.score > best.score) best = candidate;
      }
      return best;
    }

    /* Resolve the rendered selection. The cloned range survives a cleared
       native selection; it fails only when the chat replaced the nodes it
       points at, and the highlight then stops while the audio continues. */
    function findSelectionTarget() {
      const range = reading.range;
      if (range === null) return null;
      const start = range.startContainer;
      const end = range.endContainer;
      if (start === null || end === null || !start.isConnected || !end.isConnected) return null;
      const tokens = mergeCodeSplits(tokensFromRange(range), reading.speechNorms);
      if (tokens.length === 0) return null;
      const element = sourceElement(start) ?? sourceElement(end);
      if (element === null) return null;
      return { element: element, tokens: tokens, score: tokens.length };
    }

    /* The text-interaction source element that holds one range boundary. */
    function sourceElement(node) {
      if (node === null) return null;
      if (node.nodeType === Node.TEXT_NODE) {
        const parent = node.parentElement;
        return parent === null ? null : parent.closest(SOURCE_SELECTOR);
      }
      if (node.nodeType !== Node.ELEMENT_NODE) return null;
      return node.closest(SOURCE_SELECTOR);
    }

    /* The logical token stream is the word order a reply renders; the physical
       span is the text node and the offsets a paint needs. A host re-render can
       replace the physical spans while the logical stream stays identical. */
    function sameTokenStream(left, right) {
      if (left.length !== right.length) return false;
      for (let index = 0; index < left.length; index += 1) {
        if (left[index].norm !== right[index].norm) return false;
        if ((left[index].code === true) !== (right[index].code === true)) return false;
      }
      return true;
    }

    /* Whether the cached spans lost the text node the next paint would use. The
       reply body was re-rendered in place, so its element is still connected but
       the cached tokens point at detached text nodes. */
    function spansStale() {
      if (reading.tokens === null || reading.tokens.length === 0) return false;
      const index =
        reading.activeToken >= 0 && reading.activeToken < reading.tokens.length
          ? reading.activeToken
          : 0;
      return !reading.tokens[index].node.isConnected;
    }

    /* Resolve the rendered reply. A node key is exact; without one, the DOM is
       the only address and the closest assistant row wins. Re-resolving keeps
       the alignment when the physical spans changed but the word order did not:
       the map and the cursor index the word order, not the nodes. */
    function ensureTarget(force = false) {
      if (!force && reading.element !== null && reading.element.isConnected && !spansStale()) {
        return reading.element;
      }
      const found = reading.range === null ? findMessageTarget() : findSelectionTarget();
      releaseMarkHost();
      const previous = reading.tokens;
      const previousActive = reading.activeToken;
      reading.element = found === null ? null : found.element;
      reading.tokens = found === null ? null : found.tokens;
      reading.scroller = null;
      if (
        previous !== null &&
        reading.tokens !== null &&
        sameTokenStream(previous, reading.tokens)
      ) {
        /* The reply text survived the DOM change. Its spans are new, but the
           word order is unchanged, so the fragment map, cursor, and painted
           token index stay valid. */
        if (previousActive >= reading.tokens.length) reading.activeToken = -1;
        return reading.element;
      }
      reading.cursor = 0;
      reading.fragmentText = null;
      reading.fragmentWords = null;
      reading.fragmentMap = null;
      reading.fragmentStart = 0;
      reading.fragmentEnd = 0;
      reading.fragmentStarts.clear();
      reading.fragmentIndex = null;
      reading.activeToken = -1;
      return reading.element;
    }

    /* A rewind moves the playhead inside the words, not the words themselves.
       Keep the aligned fragment unless the rewind crossed a fragment boundary:
       re-aligning from the message start can snap onto an earlier repeat of the
       same sentence. A crossing rewind starts at the target fragment's recorded
       token start instead. The caller repaints in the same pass, so the mark
       does not flash. */
    function invalidateAlignment(keepFragment = false, restartAt = 0) {
      if (!keepFragment) {
        reading.fragmentText = null;
        reading.fragmentWords = null;
        reading.fragmentMap = null;
        reading.fragmentPosition = 0;
        reading.fragmentStart = restartAt;
        reading.fragmentEnd = 0;
        reading.cursor = restartAt;
      }
      reading.activeToken = -1;
    }

    /* The key of one fragment in this reading: the app's fragment index while
       it plays, or the fragment text for an older app that reports none. Two
       fragments can carry identical text, so the index is the primary key. */
    function fragmentKey(snapshot) {
      const index = snapshot.fragmentIndex;
      return index === null || index === undefined ? "text:" + snapshot.text : index;
    }

    function ensureFragmentMap(snapshot) {
      if (reading.tokens === null) return null;
      /* The app's fragment index names the fragment when both sides report one;
         two fragments can carry identical text, so the text alone cannot tell
         them apart. */
      const indexed = reading.fragmentIndex !== null && snapshot.fragmentIndex !== null;
      const sameFragment = indexed ? reading.fragmentIndex === snapshot.fragmentIndex : true;
      /* Cache one alignment per fragment. An identical next fragment restarts
         the clock, so a position that jumps back re-anchors it. */
      const cached =
        sameFragment &&
        reading.fragmentText === snapshot.text &&
        reading.fragmentEntries === snapshot.words.length &&
        reading.fragmentWords !== null &&
        reading.fragmentMap !== null &&
        snapshot.position >= reading.fragmentPosition - 300;
      if (cached) return reading.fragmentMap;
      const fragmentWords = captionWords(snapshot);
      /* Fresh synthesis grows one fragment between polls: each report starts
         its text at the fragment's first word. Align a growing report from the
         fragment start again. Aligning forward from the previous, shorter
         report anchors on a later repeat of the first word and fails. A text
         that is not a longer prefix is a new fragment, so the next alignment
         starts after the previous fragment's mapped words. */
      const growing =
        sameFragment &&
        reading.fragmentText !== null &&
        snapshot.text.startsWith(reading.fragmentText);
      if (!growing) {
        reading.cursor = Math.max(reading.cursor, reading.fragmentEnd);
        reading.fragmentStart = reading.cursor;
        reading.fragmentEnd = 0;
      }
      const map = alignFragment(fragmentWords, reading.tokens, reading.fragmentStart);
      reading.fragmentText = snapshot.text;
      reading.fragmentWords = fragmentWords;
      reading.fragmentEntries = snapshot.words.length;
      reading.fragmentMap = map;
      reading.fragmentPosition = snapshot.position;
      if (map === null) return null;
      /* Remember where this fragment sits in the message, so a later rewind to
         it can align from the same region instead of the message start. */
      reading.fragmentStarts.set(fragmentKey(snapshot), reading.fragmentStart);
      reading.fragmentIndex = snapshot.fragmentIndex;
      let last = -1;
      for (const index of map) if (index > last) last = index;
      if (last >= 0) reading.fragmentEnd = Math.max(reading.fragmentEnd, last + 1);
      return map;
    }

    function clearHighlight() {
      cancelHighlightTween();
      if (window.CSS !== undefined && window.CSS.highlights !== undefined) {
        window.CSS.highlights.delete(HIGHLIGHT_NAME);
        window.CSS.highlights.delete(HIGHLIGHT_NAME_OUT);
        window.CSS.highlights.delete(HIGHLIGHT_NAME_IN);
      }
      reading.paintedToken = null;
      clearMarks();
      /* The mapped token stays, so Show in chat still finds the spoken word
         during a gap between words. A new reading resets it in resetReading. */
    }

    /* Drop every painted mark but keep the layer and its host. */
    function clearMarks() {
      for (const mark of reading.markRects) mark.remove();
      reading.markRects.length = 0;
    }

    /* The animation switch decides whether the mark glides between words or
       jumps. Keep the live layer in step when the switch flips mid-reading. */
    function syncMarkAnimation() {
      if (reading.markLayer === null) return;
      if (preferences.animation) reading.markLayer.setAttribute("data-animate", "");
      else reading.markLayer.removeAttribute("data-animate");
    }

    /* Drop the layer and its host attribute: the stack context must not
       outlive the reading or outlive the target element. */
    function releaseMarkHost() {
      cancelHighlightTween();
      clearMarks();
      if (reading.markLayer !== null) reading.markLayer.remove();
      if (reading.markHost !== null) reading.markHost.removeAttribute("data-dsh-csva-mark-host");
      reading.markHost = null;
      reading.markLayer = null;
      reading.markContaining = null;
    }

    /* The rounded pill paints inside the mark host's own stacking context. A
       React re-render can drop the layer, so this re-creates it on demand. */
    function ensureMarkLayer() {
      const host = reading.element;
      if (host === null) return null;
      if (
        reading.markHost === host &&
        reading.markLayer !== null &&
        reading.markLayer.isConnected
      ) {
        return reading.markLayer;
      }
      if (reading.markHost !== null) releaseMarkHost();
      host.setAttribute("data-dsh-csva-mark-host", "");
      const layer = document.createElement("div");
      layer.className = "dsh-csva-markLayer";
      layer.setAttribute("aria-hidden", "true");
      host.appendChild(layer);
      reading.markHost = host;
      reading.markLayer = layer;
      syncMarkAnimation();
      return layer;
    }

    /* The positioned ancestor the marks sit in. It travels with the reply when
       the chat scrolls, so the mark needs no scroll listener. */
    function markContainingBlock(host) {
      const cached = reading.markContaining;
      if (cached !== null && cached.isConnected && cached.contains(host)) return cached;
      let node = host.parentElement;
      while (node !== null) {
        const position = window.getComputedStyle(node).position;
        if (position !== undefined && position !== "static") {
          reading.markContaining = node;
          return node;
        }
        node = node.parentElement;
      }
      reading.markContaining = null;
      return null;
    }

    /* Range.getClientRects is missing in jsdom and can throw on a stale node,
       so a thrown call reads as "no layout rect". */
    function wordRects(range) {
      let rects = [];
      try {
        rects = Array.from(range.getClientRects());
      } catch {
        rects = [];
      }
      return rects.filter(function (rect) {
        return rect.width > 0 && rect.height > 0;
      });
    }

    /* Paint one rounded mark per line of the spoken word. */
    function placeMarks(rects) {
      const layer = ensureMarkLayer();
      if (layer === null) return;
      const block = markContainingBlock(reading.markHost);
      const blockRect = block === null ? null : block.getBoundingClientRect();
      /* Absolute offsets count from the containing block's padding box, or
         from the document when no ancestor is positioned. */
      const baseX = blockRect === null ? window.scrollX : -blockRect.left - block.clientLeft;
      const baseY = blockRect === null ? window.scrollY : -blockRect.top - block.clientTop;
      while (reading.markRects.length < rects.length) {
        const mark = document.createElement("div");
        mark.className = "dsh-csva-mark";
        mark.setAttribute("aria-hidden", "true");
        layer.appendChild(mark);
        reading.markRects.push(mark);
      }
      while (reading.markRects.length > rects.length) {
        const mark = reading.markRects.pop();
        mark.remove();
      }
      for (let index = 0; index < rects.length; index += 1) {
        const rect = rects[index];
        const mark = reading.markRects[index];
        mark.style.transform = "";
        mark.style.left = Math.round(rect.left + baseX - MARK_PADDING_X) + "px";
        mark.style.top = Math.round(rect.top + baseY - MARK_PADDING_Y) + "px";
        mark.style.width = Math.round(rect.width + MARK_PADDING_X * 2) + "px";
        mark.style.height = Math.round(rect.height + MARK_PADDING_Y * 2) + "px";
      }
    }

    /* Recolor the word, place its rounded mark, then follow it. A word inside a
       code chip or code block takes the code highlight: that highlight paints
       its own background, and no mark paints, because the chip background
       covers the mark layer. A browser without the CSS Custom Highlight API
       keeps the old behavior: no mark, because the mark only reads as a
       highlight under the recolored text. */
    function paintWord(token) {
      const range = tokenRange(token);
      if (window.CSS !== undefined && window.CSS.highlights !== undefined) {
        const previous = reading.paintedToken;
        const glider = reading.markRects[0] ?? null;
        const fromLeft = glider === null ? NaN : parseFloat(glider.style.left);
        const fromTop = glider === null ? NaN : parseFloat(glider.style.top);
        /* Read the width before placeMarks overwrites the mark geometry with
           the NEW word's rects: the tween needs the OLD width as its start. */
        const fromWidth = glider === null || Number.isNaN(parseFloat(glider.style.width))
          ? 0
          : parseFloat(glider.style.width);
        const settlesInstantly =
          !markGlides() || previous === null || reading.markRects.length === 0;
        placeMarks(wordRects(range));
        if (settlesInstantly) {
          cancelHighlightTween();
          applyHighlight(HIGHLIGHT_NAME, range);
          reading.paintedToken = token;
        } else {
          /* One rAF clock glides the pill and blends the two words' text
             colors, with each blend windowed to when the pill actually
             covers that word, so no word darkens over bare background
             mid-glide. */
          startHighlightTween(previous, token, range, {
            left: fromLeft,
            top: fromTop,
            width: fromWidth
          });
        }
      }
      followRange(token, range);
    }

    /* Set exactly one highlight name, replacing whatever word was painted. */
    function applyHighlight(name, range) {
      window.CSS.highlights.delete(HIGHLIGHT_NAME);
      window.CSS.highlights.delete(HIGHLIGHT_NAME_OUT);
      window.CSS.highlights.delete(HIGHLIGHT_NAME_IN);
      window.CSS.highlights.set(name, new window.Highlight(range));
    }

    /* Whether the pill animates toward this word: the switch is on and the
     * layer is live; reduced motion disables the glide entirely. */
    function markGlides() {
      return (
        reading.markLayer !== null &&
        reading.markLayer.hasAttribute("data-animate") &&
        !window.matchMedia("(prefers-reduced-motion: reduce)").matches
      );
    }

    /* Resolve one CSS color value to concrete rgb components through a hidden
     * probe inside the reply, so theme variables and hex/oklch aliases all
     * arrive as numbers a tween can blend. */
    function resolveColor(value) {
      if (reading.colorProbe === null) {
        const probe = document.createElement("span");
        probe.setAttribute("aria-hidden", "true");
        probe.style.position = "absolute";
        probe.style.visibility = "hidden";
        reading.colorProbe = probe;
      }
      const probe = reading.colorProbe;
      const host = reading.element;
      if (host === null || !host.isConnected) return null;
      probe.remove();
      host.appendChild(probe);
      probe.style.color = value;
      const computed = window.getComputedStyle(probe).color;
      const match = computed.match(/rgba?\(([^)]+)\)/);
      if (match === null) return null;
      const parts = match[1].split(/[,/\s]+/).map(parseFloat).filter(Number.isFinite);
      if (parts.length < 3) return null;
      return { r: parts[0], g: parts[1], b: parts[2] };
    }

    function blendColor(from, to, t) {
      const channel = (a, b) => Math.round(a + (b - a) * t);
      return (
        "rgb(" +
        channel(from.r, to.r) +
        "," +
        channel(from.g, to.g) +
        "," +
        channel(from.b, to.b) +
        ")"
      );
    }

    /* The single easing curve for the glide: one rAF loop uses it for the
     * pill position and both color blends, so they cannot drift apart. */
    function easeGlide(t) {
      return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
    }

    /* Glide the pill and blend the two words' text colors across one rAF
       clock, so the colors land exactly when the pill covers each word. Each
       blend runs inside a coverage window computed from the two word boxes:
       the outgoing word stays knocked out until the pill slides off it, and
       the incoming word stays plain until the pill reaches it, so neither
       word darkens over bare background mid-glide. The custom properties and
       the mark transform live on the reply element, so a frame costs three
       style writes and a recalc scoped to one subtree. */
    function startHighlightTween(previous, token, range, from) {
      cancelHighlightTween();
      const normalOut = resolveColor("currentColor");
      const knockout = resolveColor("var(--dsw-alias-bg-base)");
      const normalIn = resolveColor("currentColor");
      const glider = reading.markRects[0] ?? null;
      const toLeft = glider === null ? NaN : parseFloat(glider.style.left);
      const toTop = glider === null ? NaN : parseFloat(glider.style.top);
      const toWidth = glider === null ? NaN : parseFloat(glider.style.width);
      if (
        knockout === null ||
        normalIn === null ||
        reading.element === null ||
        from === null ||
        glider === null ||
        Number.isNaN(toLeft) ||
        Number.isNaN(toTop) ||
        Number.isNaN(toWidth) ||
        Number.isNaN(from.left) ||
        Number.isNaN(from.top)
      ) {
        /* Unresolvable colors or geometry: keep the instant, correct swap. */
        applyHighlight(HIGHLIGHT_NAME, range);
        reading.paintedToken = token;
        return;
      }
      const scope = reading.element;
      const previousRange = previous === null ? null : tokenRange(previous);
      const plain = normalOut ?? normalIn;
      /* Blend each word by how much of it the pill actually covers, measured
         per frame in the mark layer's own coordinate space. Coverage handles
         any direction, any width change, and line changes uniformly: a word
         darkens only in step with the pill arriving over it and recovers as
         the pill leaves, so no word ever shows the knockout color over bare
         background. The old closed-form windows assumed forward motion and a
         fixed pill width, and dimmed words on backward and line-change
         glides. */
      const block = markContainingBlock(reading.markHost);
      const blockRect = block === null ? null : block.getBoundingClientRect();
      const baseX = blockRect === null ? window.scrollX : -blockRect.left - block.clientLeft;
      const baseY = blockRect === null ? window.scrollY : -blockRect.top - block.clientTop;
      const layerRects = function (range) {
        return wordRects(range).map(function (rect) {
          return {
            left: rect.left + baseX,
            top: rect.top + baseY,
            width: rect.width,
            height: rect.height
          };
        });
      };
      const tween = {
        raf: 0,
        start: performance.now(),
        duration: MARK_TRANSITION_MS,
        knockout: knockout,
        plainOut: plain,
        plainIn: normalIn,
        range: range,
        token: token,
        scope: scope,
        hadOut: previousRange !== null,
        mark: glider,
        dx: from.left - toLeft,
        dy: from.top - toTop,
        /* The pill keeps the OLD word's width at the start of the glide and
           eases to the new width on the same clock. Snapping the width first
           leaves the outgoing word's uncovered tail painted in the knockout
           color over bare background: the text vanishes for the first frames.
           One rAF drives position, width, and both blends, so nothing drifts. */
        w0: from.width,
        w1: toWidth,
        outRects: previousRange !== null ? layerRects(previousRange) : null,
        inRects: layerRects(range)
      };
      /* Fraction of the given word rects the pill currently covers. */
      const coverage = function (rects, pillLeft, pillTop, pillWidth, pillHeight) {
        if (rects === null) return 0;
        const pillRight = pillLeft + pillWidth;
        const pillBottom = pillTop + pillHeight;
        let covered = 0;
        let total = 0;
        rects.forEach(function (rect) {
          const across = Math.min(pillRight, rect.left + rect.width) -
            Math.max(pillLeft, rect.left);
          const down = Math.min(pillBottom, rect.top + rect.height) -
            Math.max(pillTop, rect.top);
          total += rect.width * rect.height;
          if (across > 0 && down > 0) covered += across * down;
        });
        return total === 0 ? 0 : Math.min(Math.max(covered / total, 0), 1);
      };
      /* Paint the first frame's state before the first rAF, so the pill and
         the colors never show their settled neighbors for a frame. */
      tween.mark.style.transform =
        "translate(" + tween.dx + "px," + tween.dy + "px)";
      tween.mark.style.width = tween.w0 + "px";
      if (tween.hadOut) {
        scope.style.setProperty("--dsh-csva-out-color", blendColor(knockout, plain, 0));
      }
      scope.style.setProperty("--dsh-csva-in-color", blendColor(normalIn, knockout, 0));
      if (previousRange !== null && previousRange.startContainer.isConnected) {
        window.CSS.highlights.delete(HIGHLIGHT_NAME);
        window.CSS.highlights.set(HIGHLIGHT_NAME_OUT, new window.Highlight(previousRange));
        window.CSS.highlights.set(HIGHLIGHT_NAME_IN, new window.Highlight(range));
      } else {
        applyHighlight(HIGHLIGHT_NAME_IN, range);
      }
      const frame = function () {
        if (reading.tween !== tween) return;
        if (!token.node.isConnected) {
          /* The chat re-rendered mid-glide: drop the tween and both names.
             The next aligned paint restores the highlight. */
          cancelHighlightTween();
          return;
        }
        const t = Math.min((performance.now() - tween.start) / tween.duration, 1);
        const p = easeGlide(t);
        /* One eased progress drives the pill position, width, and both
           blends, so they cannot drift apart. */
        const pillLeft = toLeft + tween.dx * (1 - p);
        const pillTop = toTop + tween.dy * (1 - p);
        const pillWidth = tween.w0 + (tween.w1 - tween.w0) * p;
        tween.mark.style.transform =
          "translate(" + tween.dx * (1 - p) + "px," + tween.dy * (1 - p) + "px)";
        tween.mark.style.width = pillWidth + "px";
        const pillHeight = parseFloat(tween.mark.style.height) || 0;
        if (tween.hadOut) {
          /* The outgoing word recovers its plain color as coverage drops. */
          const outBlend = 1 - coverage(tween.outRects, pillLeft, pillTop, pillWidth, pillHeight);
          tween.scope.style.setProperty(
            "--dsh-csva-out-color",
            blendColor(tween.knockout, tween.plainOut, outBlend)
          );
        }
        /* The incoming word darkens only as the pill slides over it. */
        const inBlend = coverage(tween.inRects, pillLeft, pillTop, pillWidth, pillHeight);
        tween.scope.style.setProperty(
          "--dsh-csva-in-color",
          blendColor(tween.plainIn, tween.knockout, inBlend)
        );
        if (t < 1) {
          tween.raf = window.requestAnimationFrame(frame);
          return;
        }
        /* Settled: swap to the single settled highlight name. */
        cancelHighlightTween();
        applyHighlight(HIGHLIGHT_NAME, tween.range);
        reading.paintedToken = tween.token;
      };
      reading.tween = tween;
      tween.raf = window.requestAnimationFrame(frame);
    }

    function cancelHighlightTween() {
      const tween = reading.tween;
      if (tween === null) return;
      reading.tween = null;
      window.cancelAnimationFrame(tween.raf);
      if (tween.mark !== null && tween.mark.isConnected) {
        tween.mark.style.transform = "";
      }
      tween.scope.style.removeProperty("--dsh-csva-out-color");
      tween.scope.style.removeProperty("--dsh-csva-in-color");
      if (window.CSS !== undefined && window.CSS.highlights !== undefined) {
        window.CSS.highlights.delete(HIGHLIGHT_NAME_OUT);
        window.CSS.highlights.delete(HIGHLIGHT_NAME_IN);
      }
    }

    /* A React re-render can drop the mark layer while the same word plays.
       Re-assert the painted word when that happened. */
    function keepWordPainted() {
      const paint = preferences.captions || reading.forceCaptions;
      if (!paint || reading.activeToken < 0 || reading.tokens === null) return;
      if (window.CSS === undefined || window.CSS.highlights === undefined) return;
      const token = reading.tokens[reading.activeToken];
      if (!token.node.isConnected) return;
      const placed =
        reading.markHost === reading.element &&
        reading.markLayer !== null &&
        reading.markLayer.isConnected &&
        reading.markRects.length > 0 &&
        reading.markRects.every(function (mark) {
          return mark.isConnected;
        });
      if (!placed) paintWord(token);
    }

    /* Resolve the scrollport of the painted word, and keep it while the word
     * stays inside it. */
    function ensureScroller(token) {
      const current = reading.scroller;
      if (current !== null && current.isConnected && current.contains(token.node)) return current;
      const element = token.node.parentElement;
      reading.scroller = element === null ? null : findScroller(element);
      if (reading.scroller !== null) watchScroller(reading.scroller);
      return reading.scroller;
    }

    /* A manual scroll pauses following, so playback does not fight the view. */
    function pauseFollow() {
      reading.followPausedUntil = Date.now() + FOLLOW_PAUSE_MS;
    }

    /* Scroll keys only, so typing elsewhere in the frame never pauses follow. */
    const FOLLOW_KEYS = new Set(["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"]);

    function pauseFollowKey(event) {
      if (FOLLOW_KEYS.has(event.key)) pauseFollow();
    }

    function watchScroller(scroller) {
      if (reading.watchedScroller === scroller) return;
      unwatchScroller();
      reading.watchedScroller = scroller;
      scroller.addEventListener("wheel", pauseFollow, { passive: true });
      scroller.addEventListener("touchstart", pauseFollow, { passive: true });
      scroller.addEventListener("keydown", pauseFollowKey, true);
    }

    function unwatchScroller() {
      const scroller = reading.watchedScroller;
      if (scroller === null) return;
      scroller.removeEventListener("wheel", pauseFollow);
      scroller.removeEventListener("touchstart", pauseFollow);
      scroller.removeEventListener("keydown", pauseFollowKey, true);
      reading.watchedScroller = null;
    }

    function motionBehavior() {
      return window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth";
    }

    function tokenRange(token) {
      const range = document.createRange();
      range.setStart(token.node, token.start);
      range.setEnd(token.endNode ?? token.node, token.end);
      return range;
    }

    /* Move the spoken line to an anchor inside the chat scrollport. */
    function scrollRangeToLine(range, token, ratio) {
      const scroller = ensureScroller(token);
      if (scroller === null) return;
      const rect = range.getBoundingClientRect();
      if (rect.width === 0 && rect.height === 0) return;
      const box = scroller.getBoundingClientRect();
      const offset = rect.top - box.top - box.height * ratio;
      scroller.scrollTo({ top: scroller.scrollTop + offset, behavior: motionBehavior() });
    }

    /* Follow the spoken word: scroll only when its line leaves the wide band,
     * so most lines play without moving the view. */
    function followRange(token, range) {
      if (!preferences.follow) return;
      const now = Date.now();
      if (now < reading.followPausedUntil) return;
      if (now - reading.lastScrollAt < SCROLL_INTERVAL_MS) return;
      const scroller = ensureScroller(token);
      if (scroller === null) return;
      const rect = range.getBoundingClientRect();
      if (rect.width === 0 && rect.height === 0) return;
      const box = scroller.getBoundingClientRect();
      const top = rect.top - box.top;
      const bottom = rect.bottom - box.top;
      if (top >= box.height * FOLLOW_BAND_TOP && bottom <= box.height * FOLLOW_BAND_BOTTOM) {
        return;
      }
      reading.lastScrollAt = now;
      const offset = top - box.height * FOLLOW_ANCHOR;
      scroller.scrollTo({ top: scroller.scrollTop + offset, behavior: motionBehavior() });
    }

    /* The bar's scroll control: go to the spoken word now, and resume follow. */
    function showPlaybackInChat() {
      reading.followPausedUntil = 0;
      reading.lastScrollAt = 0;
      const element = ensureTarget();
      if (element === null) return;
      if (reading.activeToken >= 0 && reading.tokens !== null) {
        const token = reading.tokens[reading.activeToken];
        if (token.node.isConnected) {
          scrollRangeToLine(tokenRange(token), token, FOLLOW_ANCHOR);
          return;
        }
      }
      /* No mapped word yet: fall back to the top of the reply. */
      const scroller = findScroller(element);
      if (scroller === null) {
        element.scrollIntoView({ block: "center", behavior: motionBehavior() });
        return;
      }
      const rect = element.getBoundingClientRect();
      const box = scroller.getBoundingClientRect();
      scroller.scrollTo({
        top: scroller.scrollTop + rect.top - box.top - box.height * 0.2,
        behavior: motionBehavior()
      });
    }

    /* One status snapshot drives the inline highlight, the follow scroll, and
     * the Show in chat target. Alignment runs with captions off too, so Show in
     * chat still finds the spoken word.
     *
     * Only an inactive snapshot clears the paint. Provider word windows have
     * gaps between words (ElevenLabs reports one timing per character), so a
     * position inside a gap holds the last painted word until the next one
     * paints. The mark then follows the audio instead of flashing off. */
    function updateCaption(snapshot) {
      if (snapshot === null) {
        clearHighlight();
        return;
      }
      const element = ensureTarget();
      if (element === null || reading.tokens === null) {
        clearHighlight();
        return;
      }
      const map = ensureFragmentMap(snapshot);
      if (map === null) {
        /* A fragment that cannot align yet keeps the last painted word. */
        return;
      }
      const words = reading.fragmentWords === null ? [] : reading.fragmentWords;
      const wordIndex = activeWordIndex(words, snapshot.position);
      if (wordIndex < 0) return;
      const tokenIndex = nearestMapped(map, wordIndex);
      if (tokenIndex < 0) return;
      const token = reading.tokens[tokenIndex];
      if (!token.node.isConnected) {
        /* The reply body re-rendered in place: its word order is unchanged, so
           refresh the spans and retry the paint once instead of dropping it. */
        reading.activeToken = -1;
        clearHighlight();
        ensureTarget(true);
        if (
          reading.tokens === null ||
          reading.fragmentMap === null ||
          reading.fragmentWords === null
        ) {
          return;
        }
        const retryWordIndex = activeWordIndex(reading.fragmentWords, snapshot.position);
        if (retryWordIndex < 0) return;
        const retryTokenIndex = nearestMapped(reading.fragmentMap, retryWordIndex);
        if (retryTokenIndex < 0) return;
        const retryToken = reading.tokens[retryTokenIndex];
        if (!retryToken.node.isConnected) return;
        reading.activeToken = retryTokenIndex;
        if (preferences.captions || reading.forceCaptions) {
          paintWord(retryToken);
          return;
        }
        followRange(retryToken, tokenRange(retryToken));
        return;
      }
      if (tokenIndex === reading.activeToken) {
        keepWordPainted();
        return;
      }
      reading.activeToken = tokenIndex;
      if (preferences.captions || reading.forceCaptions) {
        paintWord(token);
        return;
      }
      followRange(token, tokenRange(token));
    }

    function resetReading(messageId, nodeKey, speech, range = null) {
      reading.messageId = messageId;
      reading.nodeKey = nodeKey;
      reading.range = range;
      reading.forceCaptions = range !== null;
      reading.speechTokens = speech === "" ? [] : tokensFromText(speech);
      reading.speechNorms = new Set(
        reading.speechTokens.map(function (token) { return token.norm; })
      );
      reading.element = null;
      reading.tokens = null;
      reading.cursor = 0;
      reading.fragmentText = null;
      reading.fragmentMap = null;
      reading.fragmentPosition = 0;
      reading.fragmentWords = null;
      reading.fragmentEntries = 0;
      reading.fragmentStart = 0;
      reading.fragmentEnd = 0;
      reading.fragmentStarts.clear();
      reading.fragmentIndex = null;
      reading.scroller = null;
      reading.activeToken = -1;
      reading.lastScrollAt = 0;
      reading.followPausedUntil = 0;
      unwatchScroller();
      clearHighlight();
      releaseMarkHost();
    }

    /* The status poll. It runs for the whole reading, with captions off too: the
     * same snapshot drives the highlight, follow, Show in chat, and the bar's
     * pause state. `position` follows the app's per-fragment clock so a rewind
     * shows itself as a backward jump. */
    const statusPoll = { timer: null, attempts: 0, sequence: 0, position: null };

    function stopStatusPolling() {
      if (statusPoll.timer !== null) clearInterval(statusPoll.timer);
      statusPoll.timer = null;
      statusPoll.attempts = 0;
      statusPoll.sequence = 0;
      statusPoll.position = null;
    }

    function startStatusPolling(requestId) {
      stopStatusPolling();
      function poll() {
        if (playback.requestId !== requestId) {
          stopStatusPolling();
          return;
        }
        statusPoll.sequence += 1;
        const sequence = statusPoll.sequence;
        void readPlaybackStatus().then(function (result) {
          /* Ignore a slower answer from an older poll. */
          if (playback.requestId !== requestId || sequence !== statusPoll.sequence) return;
          if (!result.reached) {
            clearHighlight();
            statusPoll.attempts += 1;
            if (statusPoll.attempts >= STATUS_ATTEMPTS) {
              statusPoll.attempts = 0;
              stopStatusPolling();
              /* An older app has no status route and no pause routes: assume
                 the reading still plays, so the glyph does not stay on pause. */
              setPlaybackPaused(false);
            }
            return;
          }
          statusPoll.attempts = 0;
          /* The app owns the pause state: it can toggle from its own window,
             and an app without the pause routes answers the poll anyway. Skip
             the reconcile right after a local flip, so the app can apply it. */
          if (Date.now() - playback.pauseChangedAt >= PAUSE_RECONCILE_MS) {
            setPlaybackPaused(result.paused);
          }
          const snapshot = result.caption;
          const rewound = trackRewind(snapshot);
          updateCaption(snapshot);
          /* A rewind that crossed a fragment boundary only shows itself when
             the forward alignment finds nothing: retry from the target
             fragment's recorded token start, or from the message start when
             this reading never aligned it. */
          if (rewound && snapshot !== null && reading.fragmentMap === null) {
            const start = reading.fragmentStarts.get(fragmentKey(snapshot));
            invalidateAlignment(false, start === undefined ? 0 : start);
            updateCaption(snapshot);
          }
        });
      }
      poll();
      statusPoll.timer = setInterval(poll, STATUS_POLL_MS);
    }

    /* The app reports the position inside one fragment, so a normal fragment
       change also drops it. A drop on the same fragment is certainly a rewind:
       keep the fragment map and repaint, so the alignment cannot snap onto an
       earlier repeat of the same words. A crossing rewind re-aligns from the
       target fragment's recorded token start; a forward fragment change is left
       to the normal forward alignment. The app's fragment index decides the
       direction when both sides report one, otherwise the fragment text does. */
    function trackRewind(snapshot) {
      if (snapshot === null) return false;
      const previous = statusPoll.position;
      statusPoll.position = snapshot.position;
      if (previous === null || snapshot.position >= previous - REWIND_SLOP_MS) return false;
      const indexed = reading.fragmentIndex !== null && snapshot.fragmentIndex !== null;
      const sameFragment = indexed
        ? reading.fragmentIndex === snapshot.fragmentIndex
        : reading.fragmentText === snapshot.text;
      if (sameFragment) {
        /* A same-fragment rewind is certain: keep the aligned fragment, drop
           the paint now, then repaint the rewound word in this same poll pass.
           A cross-fragment drop keeps the last painted word until an alignment
           lands. */
        invalidateAlignment(true);
        clearHighlight();
        return true;
      }
      if (indexed && snapshot.fragmentIndex < reading.fragmentIndex) {
        /* A crossing rewind to a fragment this reading already aligned: start
           the alignment at its recorded region, not at the message start. */
        const start = reading.fragmentStarts.get(fragmentKey(snapshot));
        if (start !== undefined) invalidateAlignment(false, start);
      }
      return true;
    }

    /* CopySpeak reports the caption it renders in its own webview, plus the
       pause state. A missing route stops the polling; an inactive snapshot
       clears the highlight. */
    function readPlaybackStatus() {
      return fetch(CONTROL_URL + "/status", { mode: "cors", cache: "no-store" })
        .then(function (response) {
          if (!response.ok) return { reached: false, paused: false, caption: null };
          return response.json().then(function (body) {
            return {
              reached: true,
              paused: body !== null && body !== undefined && body.paused === true,
              caption: captionFromStatus(body)
            };
          });
        })
        .catch(function () {
          return { reached: false, paused: false, caption: null };
        });
    }

    function captionFromStatus(body) {
      if (body === null || body === undefined || body.active !== true) return null;
      const source = body.captions;
      if (source === null || source === undefined) return null;
      if (!Array.isArray(source.words) || source.words.length === 0) return null;
      if (source.text === null || source.text === undefined || source.text === "") return null;
      if (body.position_ms === null || body.position_ms === undefined) return null;
      /* The app names the fragment it plays. An older app sends no index, so the
         fragment text stays the fallback key. */
      const fragmentIndex =
        Number.isInteger(body.fragment_index) && body.fragment_index >= 0
          ? body.fragment_index
          : null;
      return {
        text: source.text,
        words: source.words,
        position: body.position_ms,
        fragmentIndex: fragmentIndex
      };
    }

    function post(path, body) {
      return fetch(CONTROL_URL + path, {
        method: "POST",
        mode: "no-cors",
        cache: "no-store",
        headers: { "content-type": "text/plain" },
        body: JSON.stringify(body)
      }).then(
        function () {
          return true;
        },
        function () {
          return false;
        }
      );
    }

    /* CopySpeak reachability: a no-cors GET rejects only when nothing listens. */
    function probeControlServer() {
      return fetch(CONTROL_URL + "/health", { mode: "no-cors", cache: "no-store" }).then(
        function () {
          return true;
        },
        function () {
          return false;
        }
      );
    }

    function stopReading(requestId) {
      return post("/stop", { request_id: requestId });
    }

    /* Pause and resume address one reading by request id, like stop. The glyph
       flips at once; the next status poll confirms it, and an older app that
       has no pause route ignores the request. Without a live status poll there
       is nothing to reconcile against, so the glyph stays as it is. */
    function pauseReading() {
      const requestId = playback.requestId;
      if (requestId === null) return Promise.resolve();
      if (statusPoll.timer === null) return post("/pause", { request_id: requestId });
      setPlaybackPaused(true);
      playback.pauseChangedAt = Date.now();
      return post("/pause", { request_id: requestId });
    }

    function resumeReading() {
      const requestId = playback.requestId;
      if (requestId === null) return Promise.resolve();
      if (statusPoll.timer === null) return post("/resume", { request_id: requestId });
      setPlaybackPaused(false);
      playback.pauseChangedAt = Date.now();
      return post("/resume", { request_id: requestId });
    }

    /* Rewind or skip one reading. Seek stays available while paused: the app
       moves the position, and a later resume plays from it. */
    function seekReading(deltaMs) {
      const requestId = playback.requestId;
      if (requestId === null) return Promise.resolve();
      return post("/seek", { request_id: requestId, delta_ms: deltaMs });
    }

    /* Start one reading and resolve with its failure code, or null. A selection
       reading passes its cloned range instead of a message id and node key. */
    function beginReading(messageId, nodeKey, speech, range = null) {
      const previous = playback.requestId;
      const stopping = previous === null ? Promise.resolve(true) : stopReading(previous);
      return stopping.then(function (stopped) {
        if (!stopped) {
          forget();
          return "error.offline";
        }
        const requestId = nextRequestId();
        resetReading(messageId, nodeKey, speech, range);
        remember(messageId, requestId, range === null ? "message" : "selection");
        startStatusPolling(requestId);
        return post("/speak", { text: speech, request_id: requestId, wait: true }).then(
          function (finished) {
            if (playback.requestId !== requestId) return null;
            forget();
            return finished ? null : "error.offline";
          }
        );
      });
    }

    function stopReadingNow() {
      const requestId = playback.requestId;
      if (requestId === null) {
        forget();
        return Promise.resolve();
      }
      return stopReading(requestId).then(function () {
        forget();
      });
    }

    /* Turn rendered Markdown into something a speech engine can read. */
    function toSpeech(markdown) {
      let text = markdown == null ? "" : String(markdown);
      text = text.replace(/```[\s\S]*?```/g, " ");
      text = text.replace(/`([^`]*)`/g, "$1");
      text = text.replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1");
      text = text.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
      text = text.replace(/^ {0,3}#{1,6}\s*/gm, "");
      text = text.replace(/^ {0,3}>\s?/gm, "");
      text = text.replace(/^ {0,3}(?:[-*+]|\d+[.)])\s+/gm, "");
      text = text.replace(/^ {0,3}(?:-{3,}|\*{3,}|_{3,})\s*$/gm, "");
      text = text.replace(/(\*\*|__)(.*?)\1/g, "$2");
      text = text.replace(/\*([^*\n]+)\*/g, "$1");
      text = text.replace(/~~(.*?)~~/g, "$1");
      text = text.replace(/<\/?[a-zA-Z][^>]*>/g, " ");
      text = text.replace(/\|/g, " ");
      text = text.replace(/[ \t]+\n/g, "\n");
      text = text.replace(/\n{3,}/g, "\n\n");
      return text.trim();
    }

    function assistantText(blocks) {
      if (!Array.isArray(blocks)) return "";
      const parts = [];
      for (const block of blocks) {
        if (block != null && block.kind === "text" && block.text != null) {
          parts.push(block.text);
        }
      }
      return parts.join("\n\n");
    }

    /* The Chat snapshot is the client's only source of settled message text:
       each settled assistant step carries its durable message id and blocks. */
    function assistantNode(snapshot, messageId) {
      if (snapshot == null || snapshot.nodes == null || snapshot.order == null) return null;
      const order = snapshot.order;
      for (let index = order.length - 1; index >= 0; index -= 1) {
        const node = snapshot.nodes.get(order[index]);
        if (node == null || node.kind !== "assistant-step" || node.data == null) continue;
        const finalNode = node.data.finalNode;
        if (finalNode == null || finalNode.messageId === undefined) continue;
        if (messageId !== undefined && finalNode.messageId !== messageId) continue;
        return node;
      }
      return null;
    }

    /* The host closes every turn with a `turn-tail` location. Its `closing` step
       is the turn's final text-bearing assistant step, so auto-read fires once
       per completed turn, never per intermediate step of a tool-calling turn. */
    function closingReply(snapshot) {
      if (snapshot == null || snapshot.nodes == null || snapshot.order == null) return null;
      const timeline = snapshot.timeline;
      if (timeline == null || timeline.turns == null) return null;
      if (!Array.isArray(timeline.turnOrder) || timeline.turnOrder.length === 0) return null;
      const number = timeline.turnOrder[timeline.turnOrder.length - 1];
      const turn = timeline.turns.get(number);
      if (turn == null || turn.end === undefined) return null;
      const tail = turn.data == null ? null : turn.data.get("turn-tail");
      const closing = tail == null ? null : tail.closing;
      if (closing == null || closing.finalNode == null) return null;
      const finalNode = closing.finalNode;
      /* A stopped attempt has no durable message id, and no read action either. */
      if (finalNode.messageId === undefined) return null;
      if (!Array.isArray(closing.blocks)) return null;
      /* The assistant-step node carries the key the caption reader addresses. */
      let nodeKey = null;
      for (const key of snapshot.order) {
        const node = snapshot.nodes.get(key);
        if (node == null || node.kind !== "assistant-step" || node.data == null) continue;
        const settled = node.data.finalNode;
        if (settled != null && settled.seq === finalNode.seq) {
          nodeKey = key;
          break;
        }
      }
      return { messageId: finalNode.messageId, key: nodeKey, blocks: closing.blocks };
    }

    /* The filled app-logo speaker silhouette. The playing action and the
       floating playback bar use it in currentColor, so it follows the theme. */
    function CsGlyph(props) {
      const size = props.size ?? 18;
      return h(
        "svg",
        {
          viewBox: GLYPH_VIEW_BOX,
          width: size,
          height: size,
          className: "dsh-csva-glyph",
          "aria-hidden": true,
          focusable: "false"
        },
        GLYPH_PATHS.map(function (d, index) {
          return h("path", { key: index, d: d, fill: "currentColor" });
        })
      );
    }

    /* The outlined read mark: the idle message action and the sidebar panel
       icon share it. The speaker contour is stroked and the waves become
       centerline arcs, which stay crisp at 16-17 px. */
    function ReadGlyph(props) {
      const size = props.size ?? 17;
      /* Keep the rendered stroke the same weight at every size. */
      const stroke = GLYPH_STROKE * (17 / size);
      return h(
        "svg",
        {
          viewBox: GLYPH_VIEW_BOX,
          width: size,
          height: size,
          fill: "none",
          stroke: "currentColor",
          strokeLinecap: "round",
          strokeLinejoin: "round",
          className: "dsh-csva-glyph",
          "aria-hidden": true,
          focusable: "false"
        },
        h("path", { d: GLYPH_PATHS[0], strokeWidth: stroke }),
        h("path", { d: GLYPH_ARC_OUTER, strokeWidth: stroke }),
        h("path", { d: GLYPH_ARC_INNER, strokeWidth: stroke })
      );
    }

    /* Sidebar panel icon: the idle read mark, shared with the message action. */
    function CopySpeakPanelIcon(props) {
      return h(ReadGlyph, { size: props.size ?? 16 });
    }

    /*
     * Bar control glyphs. They draw in a 16 px box in `currentColor`, so the
     * icon-only controls follow the host theme. Fill marks (pause, play, stop)
     * pair with stroked direction marks (rewind, skip, chat).
     */
    function ControlIcon(props) {
      return h(
        "svg",
        {
          viewBox: "0 0 16 16",
          width: 16,
          height: 16,
          className: "dsh-csva-glyph",
          "aria-hidden": true,
          focusable: "false"
        },
        props.children
      );
    }

    function PauseGlyph() {
      return h(
        ControlIcon,
        null,
        h("rect", { x: 4.6, y: 3, width: 2.5, height: 10, rx: 0.9, fill: "currentColor" }),
        h("rect", { x: 8.9, y: 3, width: 2.5, height: 10, rx: 0.9, fill: "currentColor" })
      );
    }

    function PlayGlyph() {
      return h(
        ControlIcon,
        null,
        h("path", {
          d: "M5.8 3.5 12.2 8 5.8 12.5Z",
          fill: "currentColor",
          stroke: "currentColor",
          strokeWidth: 1,
          strokeLinejoin: "round"
        })
      );
    }

    function StopGlyph() {
      return h(
        ControlIcon,
        null,
        h("rect", { x: 3.8, y: 3.8, width: 8.4, height: 8.4, rx: 1.3, fill: "currentColor" })
      );
    }

    /* The rewind and skip marks: two chevrons plus the numeral 5. The numeral
       is SVG text, so it inherits currentColor and stays legible at 16 px. */
    function SkipGlyph(props) {
      const back = props.back === true;
      const chevrons = back
        ? ["M4.4 3.9 1.7 8l2.7 4.1", "M7.6 3.9 4.9 8l2.7 4.1"]
        : ["M8.4 3.9 11.1 8l-2.7 4.1", "M11.6 3.9 14.3 8l-2.7 4.1"];
      return h(
        ControlIcon,
        null,
        chevrons.map(function (d, index) {
          return h("path", {
            key: index,
            d: d,
            fill: "none",
            stroke: "currentColor",
            strokeWidth: 1.5,
            strokeLinecap: "round",
            strokeLinejoin: "round"
          });
        }),
        h(
          "text",
          {
            x: back ? 11.2 : 4.8,
            y: 8.1,
            textAnchor: "middle",
            dominantBaseline: "central",
            fontSize: 8.2,
            fontWeight: 700,
            fill: "currentColor"
          },
          "5"
        )
      );
    }

    /* Show in chat: a speech bubble, the same mark chat surfaces use. */
    function ChatGlyph() {
      return h(
        ControlIcon,
        null,
        h("path", {
          d: "M3.6 3.1H12.4A1.9 1.9 0 0 1 14.3 5V9A1.9 1.9 0 0 1 12.4 10.9H7.6L4.9 13.3V10.9H3.6A1.9 1.9 0 0 1 1.7 9V5A1.9 1.9 0 0 1 3.6 3.1Z",
          fill: "none",
          stroke: "currentColor",
          strokeWidth: 1.4,
          strokeLinejoin: "round"
        })
      );
    }

    /*
     * The host Tooltip primitive is a package a plain-JS plugin cannot require,
     * so the plugin owns a copy of that bubble: a fixed bubble under or above
     * the anchor button, shown on hover and on keyboard focus. The message
     * action and every bar control share this hook.
     */
    function useButtonTooltip(preferredSide) {
      const side = preferredSide ?? "bottom";
      const buttonRef = React.useRef(null);
      const tipRef = React.useRef(null);
      const tipId = React.useId();
      const [tip, setTip] = React.useState(null);

      /* Escape, any scroll, and a viewport resize close the bubble, so a moved
         anchor cannot leave a stale one behind. Listeners attach only while
         open. */
      React.useEffect(
        function () {
          if (tip === null) return;
          function close() {
            setTip(null);
          }
          function onKeyDown(event) {
            if (event.key === "Escape") close();
          }
          document.addEventListener("keydown", onKeyDown, true);
          window.addEventListener("scroll", close, true);
          window.addEventListener("resize", close);
          return function () {
            document.removeEventListener("keydown", onKeyDown, true);
            window.removeEventListener("scroll", close, true);
            window.removeEventListener("resize", close);
          };
        },
        [tip]
      );

      /* Fit the bubble after it renders: clamp its center inside the viewport,
         and flip it when the preferred side runs out of room. */
      React.useLayoutEffect(
        function () {
          if (tip === null) return;
          const bubble = tipRef.current;
          if (bubble === null) return;
          const half = bubble.offsetWidth / 2;
          const min = TOOLTIP_MARGIN + half;
          const max = window.innerWidth - TOOLTIP_MARGIN - half;
          const left = Math.min(Math.max(tip.left, min), max);
          let nextSide = tip.side;
          if (
            nextSide === "bottom" &&
            tip.bottom + bubble.offsetHeight > window.innerHeight - TOOLTIP_MARGIN
          ) {
            nextSide = "top";
          } else if (nextSide === "top" && tip.top - bubble.offsetHeight < TOOLTIP_MARGIN) {
            nextSide = "bottom";
          }
          if (Math.abs(left - tip.left) > 0.5 || nextSide !== tip.side) {
            setTip({ left: left, top: tip.top, bottom: tip.bottom, side: nextSide });
          }
        },
        [tip]
      );

      function open() {
        const button = buttonRef.current;
        if (button === null) return;
        const rect = button.getBoundingClientRect();
        setTip({
          left: rect.left + rect.width / 2,
          top: rect.top - TOOLTIP_GAP,
          bottom: rect.bottom + TOOLTIP_GAP,
          side: side
        });
      }

      function close() {
        setTip(null);
      }

      function onPointerEnter(event) {
        /* Touch taps fire the click; a bubble on a touch screen is noise. */
        if (event.pointerType === "touch") return;
        open();
      }

      function onFocus(event) {
        /* A mouse click focuses the button before it clicks. The host shows a
           focus tooltip for keyboard focus only, so a click never flashes one. */
        if (!event.currentTarget.matches(":focus-visible")) return;
        open();
      }

      function render(label) {
        if (tip === null) return null;
        return h(
          "span",
          {
            ref: tipRef,
            id: tipId,
            role: "tooltip",
            className: "dsh-csva-tip",
            "data-side": tip.side,
            style: {
              left: tip.left,
              top: tip.side === "bottom" ? tip.bottom : tip.top
            }
          },
          label
        );
      }

      return {
        buttonRef: buttonRef,
        describedBy: tip === null ? undefined : tipId,
        close: close,
        onPointerEnter: onPointerEnter,
        onFocus: onFocus,
        render: render
      };
    }

    function CopySpeakAction(props) {
      const messageId = props.messageId;
      const useChat = props.useChat;
      const t = props.t;
      /* The node key addresses the reply in the Chat flow, which the caption
         reader highlights; the node itself is kept out of the selector result
         so a streamed token does not allocate one. */
      const nodeRef = React.useRef(null);

      /* The host action-row buttons carry a Tooltip primitive; the plugin owns
         the same bubble through the shared hook. */
      const tip = useButtonTooltip("bottom");

      const text = useChat
        ? useChat(function (snapshot) {
            const node = assistantNode(snapshot, messageId);
            nodeRef.current = node;
            return node === null ? "" : assistantText(node.data.blocks);
          })
        : "";

      const [activeMessageId, setActiveMessageId] = React.useState(playback.messageId);
      const [busy, setBusy] = React.useState(false);
      const [failure, setFailure] = React.useState(null);
      const failureTimer = React.useRef(null);

      React.useEffect(function () {
        return subscribe(playback.listeners, function () {
          setActiveMessageId(playback.messageId);
        });
      }, []);

      React.useEffect(function () {
        return function () {
          if (failureTimer.current !== null) clearTimeout(failureTimer.current);
        };
      }, []);

      function fail(key) {
        setFailure(key);
        if (failureTimer.current !== null) clearTimeout(failureTimer.current);
        failureTimer.current = setTimeout(function () {
          setFailure(null);
        }, 4000);
      }

      const active = activeMessageId === messageId;
      const hasText = text.trim() !== "";
      const label = active ? t("action.stop") : t("action.read");
      /* A running reading must stay clickable even while its start is still
         waiting: the stop control is the same button. */
      const disabled = !active && (busy || !hasText);

      function onClick() {
        tip.close();
        if (active) {
          setBusy(true);
          void stopReadingNow().then(function () {
            setBusy(false);
          });
          return;
        }
        if (busy) return;
        const speech = toSpeech(text);
        if (speech === "") {
          fail("error.empty");
          return;
        }
        const node = nodeRef.current;
        setBusy(true);
        void beginReading(messageId, node === null ? null : node.key, speech).then(
          function (error) {
            setBusy(false);
            if (error !== null) fail(error);
          }
        );
      }

      return h(
        "span",
        { className: "dsh-csva-root" },
        h(
          "button",
          {
            ref: tip.buttonRef,
            type: "button",
            className: "dsh-csva-button",
            "aria-label": label,
            "aria-pressed": active,
            "aria-describedby": tip.describedBy,
            "data-active": active || undefined,
            disabled: disabled,
            onPointerEnter: tip.onPointerEnter,
            onPointerLeave: tip.close,
            onFocus: tip.onFocus,
            onBlur: tip.close,
            onClick: onClick
          },
          active ? h(CsGlyph, { size: 17 }) : h(ReadGlyph)
        ),
        tip.render(label),
        failure === null
          ? null
          : h("span", { className: "dsh-csva-note", role: "status" }, t(failure))
      );
    }

    /* Quote needs a writable conversation; reading a selection does not. */
    function sessionWritable(session) {
      if (session.removed) return false;
      if (session.subagent === null) return true;
      return (
        session.subagent.address.mode === "continuable" && session.subagent.parentAvailable === true
      );
    }

    /* The selection pills: the host Quote action beside the CopySpeak read
       action. The read pill only starts a reading; stop, pause, and seek stay
       on the floating playback bar. A writable session shows both pills. A
       read-only session opens this surface through the CopySpeak interaction
       alone, so Quote is absent: the host Quote admission refuses a
       non-writable conversation. */
    function SelectionAction(props) {
      const t = props.t ?? boundT;
      const dismiss = props.dismiss;
      const readText = props.readText;
      const clearSelection = props.clearSelection;
      const inputActions = props.inputActions;
      const useSession = props.useSession;
      const enabled = useSession === undefined ? true : useSession(sessionWritable);
      const [failure, setFailure] = React.useState(null);
      const failureTimer = React.useRef(null);

      React.useEffect(function () {
        return function () {
          if (failureTimer.current !== null) clearTimeout(failureTimer.current);
        };
      }, []);

      function fail(key) {
        setFailure(key);
        if (failureTimer.current !== null) clearTimeout(failureTimer.current);
        failureTimer.current = setTimeout(function () {
          setFailure(null);
        }, 4000);
      }

      function onQuote() {
        const text = readText();
        if (text === null) {
          dismiss();
          return;
        }
        if (!inputActions.insertQuote(text)) return;
        clearSelection();
        dismiss();
      }

      function onRead() {
        const text = readText();
        if (text === null) {
          dismiss();
          return;
        }
        const selection = window.getSelection();
        if (selection === null || selection.rangeCount !== 1 || selection.isCollapsed) {
          fail("error.selection");
          return;
        }
        /* The owner keeps the selection mounted while this surface is open, so
           this clone is the range to highlight. */
        const range = selection.getRangeAt(0).cloneRange();
        void beginReading(null, null, text, range).then(function (error) {
          if (error !== null) fail(error);
        });
        /* The reading owns the words now, so clear the native selection and
           leave only the word highlight. The cloned range survives a cleared
           selection, and resetReading sets the reading's range and
           forceCaptions, so the caption paint continues inside the former
           selection. The host also dismisses on selectionchange; dismissing
           here does not wait for that event. */
        clearSelection();
        dismiss();
      }

      /* A pointer-down on either pill must not collapse the selection before
         the read action can clone its range; the host Quote pill does this. */
      function keepSelection(event) {
        event.preventDefault();
      }

      return h(
        "span",
        { className: "dsh-csva-selection" },
        h(
          "span",
          { className: "dsh-csva-selectionGroup" },
          enabled
            ? h(
                "button",
                {
                  type: "button",
                  className: "dsh-csva-selectionAction",
                  "aria-label": t("selection.quote"),
                  onPointerDown: keepSelection,
                  onClick: onQuote
                },
                t("selection.quote")
              )
            : null,
          h(
            "button",
            {
              type: "button",
              className: "dsh-csva-selectionAction",
              "aria-label": t("selection.read"),
              onPointerDown: keepSelection,
              onClick: onRead
            },
            t("selection.read")
          )
        ),
        failure === null
          ? null
          : h("span", { className: "dsh-csva-selectionNote", role: "status" }, t(failure))
      );
    }

    /**
     * Per-session watcher under the composer: when a turn completes and
     * auto-read is on, speak its final reply. It renders nothing.
     */
    function AutoReader(props) {
      const useChat = props.useChat;
      const latest = React.useRef(null);
      const latestId = useChat
        ? useChat(function (snapshot) {
            latest.current = closingReply(snapshot);
            return latest.current === null ? null : latest.current.messageId;
          })
        : null;

      const ready = React.useRef(false);
      const seen = React.useRef(null);

      React.useEffect(
        function () {
          if (!ready.current) {
            ready.current = true;
            seen.current = latestId;
            return;
          }
          if (latestId === null || latestId === seen.current) return;
          seen.current = latestId;
          if (!preferences.autoRead) return;
          const found = latest.current;
          if (found === null) return;
          /* The watcher reads the same rendered Markdown as the action, so it
             must strip the markup exactly the same way. */
          const speech = toSpeech(assistantText(found.blocks));
          if (speech === "") return;
          void beginReading(found.messageId, found.key, speech).then(function (error) {
            if (error !== null) console.error("[copyspeak-voice] auto-read failed:", error);
          });
        },
        [latestId]
      );

      return null;
    }

    /**
     * One icon control in the playback bar. The label serves the tooltip and
     * the accessible name; the glyph is the button's only visible content.
     */
    function BarControl(props) {
      const tip = useButtonTooltip("top");
      return h(
        React.Fragment,
        null,
        h(
          "button",
          {
            ref: tip.buttonRef,
            type: "button",
            className:
              props.className === undefined
                ? "dsh-csva-barControl"
                : "dsh-csva-barControl " + props.className,
            "aria-label": props.label,
            "aria-describedby": tip.describedBy,
            onPointerEnter: tip.onPointerEnter,
            onPointerLeave: tip.close,
            onFocus: tip.onFocus,
            onBlur: tip.close,
            onClick: function (event) {
              tip.close();
              if (props.onClick !== undefined) props.onClick(event);
            }
          },
          props.children
        ),
        tip.render(props.label)
      );
    }

    /**
     * Frame-wide playback bar: it renders in the shell overlay while this
     * plugin reads, so the controls stay reachable when the message action
     * scrolled away. The bar is icon-only: pause or resume, rewind 5 seconds,
     * skip 5 seconds, stop, and Show in chat. The theme carries the colors.
     */
    function PlaybackBar(props) {
      const t = props.t ?? boundT;
      const [state, setState] = React.useState({
        active: playback.requestId !== null,
        paused: playback.paused
      });

      React.useEffect(function () {
        return subscribe(playback.listeners, function () {
          setState({ active: playback.requestId !== null, paused: playback.paused });
        });
      }, []);

      if (!state.active) return null;

      return h(
        "div",
        {
          className: "dsh-csva-bar",
          role: "region",
          "aria-label": t("bar.label")
        },
        h(
          "span",
          { className: "dsh-csva-barGlyph" },
          h("img", {
            className: "dsh-csva-barLogo",
            src: APP_LOGO_URL,
            width: 18,
            height: 18,
            alt: "",
            "aria-hidden": "true",
            draggable: "false"
          })
        ),
        h(
          BarControl,
          {
            label: t(state.paused ? "bar.resume" : "bar.pause"),
            onClick: function () {
              void (playback.paused ? resumeReading() : pauseReading());
            }
          },
          state.paused ? h(PlayGlyph) : h(PauseGlyph)
        ),
        h(
          BarControl,
          {
            label: t("bar.back5"),
            onClick: function () {
              void seekReading(-SEEK_STEP_MS);
            }
          },
          h(SkipGlyph, { back: true })
        ),
        h(
          BarControl,
          {
            label: t("bar.forward5"),
            onClick: function () {
              void seekReading(SEEK_STEP_MS);
            }
          },
          h(SkipGlyph, { back: false })
        ),
        h(
          BarControl,
          {
            className: "dsh-csva-barStop",
            label: t("bar.stop"),
            onClick: function () {
              void stopReadingNow();
            }
          },
          h(StopGlyph)
        ),
        h(BarControl, { label: t("bar.showInChat"), onClick: showPlaybackInChat }, h(ChatGlyph))
      );
    }

    function Switch(props) {
      const label = props.label;
      return h(
        "button",
        {
          type: "button",
          role: "switch",
          className: "dsh-csva-switch",
          "aria-checked": props.checked,
          "aria-label": label,
          title: label,
          "data-on": props.checked || undefined,
          onClick: props.onChange
        },
        h("span", { className: "dsh-csva-switchThumb" })
      );
    }

    /** CopySpeak options and app status on the CopySpeak sidebar panel. */
    function CopySpeakScreen(props) {
      const t = props.t;
      const [autoRead, setAutoReadState] = React.useState(preferences.autoRead);
      const [captions, setCaptionsState] = React.useState(preferences.captions);
      const [animation, setAnimationState] = React.useState(preferences.animation);
      const [follow, setFollowState] = React.useState(preferences.follow);
      const [online, setOnline] = React.useState(null);

      React.useEffect(function () {
        return subscribe(preferences.listeners, function () {
          setAutoReadState(preferences.autoRead);
          setCaptionsState(preferences.captions);
          setAnimationState(preferences.animation);
          setFollowState(preferences.follow);
        });
      }, []);

      React.useEffect(function () {
        let cancelled = false;
        function probe() {
          void probeControlServer().then(function (reachable) {
            if (!cancelled) setOnline(reachable);
          });
        }
        probe();
        const timer = setInterval(probe, 10000);
        window.addEventListener("focus", probe);
        return function () {
          cancelled = true;
          clearInterval(timer);
          window.removeEventListener("focus", probe);
        };
      }, []);

      const statusKey =
        online === null ? "status.checking" : online ? "status.online" : "status.offline";
      const statusState = online === null ? "checking" : online ? "online" : "offline";

      return h(
        "div",
        { className: "dsh-csva-panel" },
        h(
          "header",
          { className: "dsh-csva-heading" },
          h("h1", { className: "dsh-csva-title" }, t("panel.title")),
          h("p", { className: "dsh-csva-intro" }, t("panel.intro"))
        ),
        h(
          "p",
          { className: "dsh-csva-status", "data-state": statusState, role: "status" },
          h("span", { className: "dsh-csva-dot" }),
          h("span", null, t(statusKey))
        ),
        h("ul", { className: "dsh-csva-rows" }, [
          h(
            "li",
            { key: "auto-read", className: "dsh-csva-row" },
            h(
              "span",
              { className: "dsh-csva-rowText" },
              h("span", { className: "dsh-csva-rowTitle" }, t("autoRead.title")),
              h("span", { className: "dsh-csva-rowDescription" }, t("autoRead.description"))
            ),
            h(Switch, {
              checked: autoRead,
              label: t("autoRead.title") + ": " + t(autoRead ? "switch.on" : "switch.off"),
              onChange: function () {
                setPreference("autoRead", !preferences.autoRead);
              }
            })
          ),
          h(
            "li",
            { key: "captions", className: "dsh-csva-row" },
            h(
              "span",
              { className: "dsh-csva-rowText" },
              h("span", { className: "dsh-csva-rowTitle" }, t("captions.title")),
              h("span", { className: "dsh-csva-rowDescription" }, t("captions.description"))
            ),
            h(Switch, {
              checked: captions,
              label: t("captions.title") + ": " + t(captions ? "switch.on" : "switch.off"),
              onChange: function () {
                setPreference("captions", !preferences.captions);
              }
            })
          ),
          h(
            "li",
            { key: "animation", className: "dsh-csva-row" },
            h(
              "span",
              { className: "dsh-csva-rowText" },
              h("span", { className: "dsh-csva-rowTitle" }, t("animation.title")),
              h("span", { className: "dsh-csva-rowDescription" }, t("animation.description"))
            ),
            h(Switch, {
              checked: animation,
              label: t("animation.title") + ": " + t(animation ? "switch.on" : "switch.off"),
              onChange: function () {
                setPreference("animation", !preferences.animation);
              }
            })
          ),
          h(
            "li",
            { key: "follow", className: "dsh-csva-row" },
            h(
              "span",
              { className: "dsh-csva-rowText" },
              h("span", { className: "dsh-csva-rowTitle" }, t("follow.title")),
              h("span", { className: "dsh-csva-rowDescription" }, t("follow.description"))
            ),
            h(Switch, {
              checked: follow,
              label: t("follow.title") + ": " + t(follow ? "switch.on" : "switch.off"),
              onChange: function () {
                setPreference("follow", !preferences.follow);
              }
            })
          )
        ])
      );
    }

    return {
      inject: ["slots", "locale", "sessions", "uiConversation"],
      apply(ctx) {
        ctx.effect(function () {
          const style = document.createElement("style");
          style.dataset.plugin = PACKAGE;
          style.dataset.pluginCss = STYLE_KEY;
          style.textContent = PLUGIN_STYLE;
          document.head.appendChild(style);
          return function () {
            style.remove();
          };
        }, PACKAGE + ": styles");

        ctx.effect(function () {
          return ctx.locale.register(NS, { en: en });
        }, PACKAGE + ": dictionaries");

        ctx.effect(function () {
          return function () {
            /* This plugin owns its reading; unloading it must stop the server
               playback, not just the timer. */
            const requestId = playback.requestId;
            stopStatusPolling();
            if (requestId !== null) void stopReading(requestId);
            forget();
          };
        }, PACKAGE + ": playback");

        const t = ctx.locale.bind(NS);
        boundT = t;

        /* The sidebar entry owns the panel icon and resolves its label from the
           list metadata; the same id selects the `main` key. */
        ctx.slots.inject("sidebar.panellist", function () {
          return ctx.slots.register(
            {
              name: "sidebar.panellist",
              id: PANEL_ID,
              order: 20,
              label: function () {
                return t("panel.label");
              },
              locale: NS
            },
            CopySpeakPanelIcon
          );
        });

        ctx.slots.inject("main", function () {
          return ctx.slots.register(
            {
              name: "main",
              key: PANEL_ID,
              locale: NS
            },
            CopySpeakScreen
          );
        });

        ctx.slots.inject("conversation.chat.assistant-actions", function () {
          return ctx.slots.register(
            {
              name: "conversation.chat.assistant-actions",
              id: "copyspeak-voice",
              order: 20,
              locale: NS
            },
            CopySpeakAction
          );
        });

        /* The watcher lives in the session-scoped composer dock so it reads
           the session on screen; it renders nothing. */
        ctx.slots.inject("conversation.composer.dock", function () {
          return ctx.slots.register(
            {
              name: "conversation.composer.dock",
              id: "copyspeak-auto-reader",
              order: 90,
              locale: NS
            },
            AutoReader
          );
        });

        /* The playback bar lives in the frame-wide overlay, so it stays
           reachable while a reply plays and the message action scrolled away. */
        ctx.slots.inject("shell.overlay", function () {
          return ctx.slots.register(
            {
              name: "shell.overlay",
              id: "copyspeak-playback",
              order: 40,
              locale: NS
            },
            PlaybackBar
          );
        });

        /* A writable selection opens the host Quote interaction, which renders
           this plugin's shadowing "quote" cell: the Quote pill stays and the
           read pill joins it. A non-writable selection gets this plugin's own
           interaction and cell instead, because the Quote admission closes
           before it opens a surface. */
        ctx.effect(function () {
          return ctx.uiConversation.interactions.register({
            id: SELECTION_ID,
            onInteraction(request) {
              if (request.trigger !== "selection") return;
              if (request.readText() === null) return;
              const binding = ctx.sessions.binding(request.sessionId);
              if (binding === undefined) return;
              if (sessionWritable(binding.session.getSnapshot())) return;
              request.open();
            }
          });
        }, PACKAGE + ": selection interaction");

        /* A double-click selects the paragraph under the double-clicked word,
           so the host opens the same pill surface a manual selection gets:
           Quote text and Read aloud act on the whole paragraph. The host chat
           view re-dispatches its selection trigger on every selection change
           outside a pointer gesture, so replacing the native word selection
           here re-opens its pill with the paragraph text. */
        ctx.effect(function () {
          function onDoubleClick(event) {
            if (event.button !== 0) return;
            const target = event.target;
            if (!(target instanceof Element)) return;
            const source = target.closest(SOURCE_SELECTOR);
            if (source === null) return;
            /* Native chrome (buttons, links, excluded regions) keeps its own
               double-click behavior; the pill surface never hijacks it. */
            if (target.closest("button, a, [data-text-interaction-exclude]") !== null) return;
            const block = target.closest(PARAGRAPH_SELECTOR);
            const paragraph = block === null || !source.contains(block) ? source : block;
            const selection = window.getSelection();
            if (selection === null) return;
            const range = document.createRange();
            range.selectNodeContents(paragraph);
            selection.removeAllRanges();
            selection.addRange(range);
          }
          document.addEventListener("dblclick", onDoubleClick);
          return function () {
            document.removeEventListener("dblclick", onDoubleClick);
          };
        }, PACKAGE + ": paragraph selection");

        ctx.slots.inject("conversation.chat.text-interaction", function () {
          const quote = ctx.slots.register(
            {
              name: "conversation.chat.text-interaction",
              key: "quote",
              /* The host Quote cell registers at the default priority 0. A
                 lower priority shadows it; the lowest live priority renders. */
              priority: -10,
              locale: NS
            },
            SelectionAction
          );
          const selection = ctx.slots.register(
            {
              name: "conversation.chat.text-interaction",
              key: SELECTION_ID,
              locale: NS
            },
            SelectionAction
          );
          return function () {
            selection();
            quote();
          };
        });
      }
    };
  }
});
