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
    const STORAGE = {
      autoRead: "dsh.copyspeak-voice.auto-read",
      captions: "dsh.copyspeak-voice.captions",
      follow: "dsh.copyspeak-voice.follow",
      animation: "dsh.copyspeak-voice.animation",
      excerpt: "dsh.copyspeak-voice.excerpt",
      excerptMode: "dsh.copyspeak-voice.excerpt-mode",
      excerptTop: "dsh.copyspeak-voice.excerpt-top",
      excerptBottom: "dsh.copyspeak-voice.excerpt-bottom"
    };
    /* Partial auto-read: keep the first and last paragraphs of a reply. */
    const EXCERPT_MODES = ["top", "bottom", "both"];
    const EXCERPT_MIN = 1;
    const EXCERPT_MAX = 20;
    const EXCERPT_DEFAULT = 2;
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
    /* The difference-blend pill: absolute div with white fill, mix-blend-mode
     * difference, pointer-events none. The white fill inverts the pill area so
     * the word reads as black on light pages, white on dark pages. */
    const MARK_PADDING_X = 4;
    const MARK_PADDING_Y = 2;
    const MARK_RADIUS_PX = 4;
    /* The mark's glide duration. The pill glides position and width together. */
    const MARK_TRANSITION_MS = 120;
    /* Per-frame cap so a heavy frame does not skip ahead. */
    const MARK_FRAME_CAP_MS = 40;
    /* Host Tooltip: the bubble sits this far from its anchor. */
    const TOOLTIP_GAP = 8;
    /* Keep the centered bubble this far from the viewport edge. */
    const TOOLTIP_MARGIN = 12;
    /* Token look-ahead while aligning a caption fragment to rendered words. */
    const ALIGN_WINDOW = 24;
    const ALIGN_ATTEMPTS = 8;
    /* A caption fragment can skip rendered text: partial auto-read speaks the
       first and last paragraphs, so the spoken fragment jumps over the omitted
       middle. A skipped word may only resume on a run the reply really contains,
       which keeps a stray common word from snapping the highlight forward. */
    const RESYNC_LOOKAHEAD = 3;

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
      "excerpt.title": "Read only part of the reply",
      "excerpt.description": "Reads the first or last paragraphs of each automatic reply.",
      "excerpt.badge": "Experimental",
      "excerpt.mode": "Part to read",
      "excerpt.mode.top": "Top",
      "excerpt.mode.bottom": "Bottom",
      "excerpt.mode.both": "Top and bottom",
      "excerpt.top": "Paragraphs from the top",
      "excerpt.bottom": "Paragraphs from the bottom",
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
      ".dsh-csva-rowTitle{display:inline-flex;align-items:center;gap:8px;font-size:13px;line-height:20px;color:var(--dsw-alias-label-primary)}",
      ".dsh-csva-rowDescription{font-size:12px;line-height:18px;color:var(--dsw-alias-label-secondary)}",
      ".dsh-csva-rowOptions{display:block;padding:12px 14px}",
      ".dsh-csva-switch{position:relative;flex:none;width:36px;height:20px;padding:0;border:none;border-radius:999px;background:var(--dsw-alias-state-idle-primary);cursor:pointer;transition:background .15s}",
      ".dsh-csva-switch[data-on]{background:var(--dsw-alias-brand-primary)}",
      ".dsh-csva-switch:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:2px}",
      ".dsh-csva-switchThumb{position:absolute;top:2px;left:2px;width:16px;height:16px;border-radius:50%;background:var(--dsw-alias-bg-base);transition:transform .15s}",
      ".dsh-csva-switch[data-on] .dsh-csva-switchThumb{transform:translateX(16px)}",
      ".dsh-csva-badge{flex:none;padding:0 6px;border:.5px solid var(--dsw-alias-border-l1);border-radius:999px;background:var(--dsw-alias-bg-layer-1);color:var(--dsw-alias-label-secondary);font-size:11px;line-height:16px}",
      ".dsh-csva-options{display:flex;flex-wrap:wrap;gap:12px;width:100%}",
      ".dsh-csva-field{display:flex;flex-direction:column;gap:6px;min-width:140px}",
      ".dsh-csva-fieldLabel{font-size:12px;line-height:18px;color:var(--dsw-alias-label-secondary)}",
      ".dsh-csva-select,.dsh-csva-number{box-sizing:border-box;height:32px;padding:0 8px;border:.5px solid var(--dsw-alias-border-l1);border-radius:var(--dsw-radius-sm,6px);background:var(--dsw-alias-bg-base);color:var(--dsw-alias-label-primary);font:inherit;font-size:13px}",
      ".dsh-csva-select:focus-visible,.dsh-csva-number:focus-visible{outline:var(--dsw-focus-ring-width,2px) solid var(--dsw-focus-ring-color,var(--dsw-alias-brand-primary));outline-offset:2px}",
      /* The frame-wide playback bar. The shell overlay is click-through, so the
         bar opts back into pointer events. It floats in the top right corner,
         below the session header's bottom border: the header is 76 px tall and
         starts under the frame top clearance. The bar is icon-only: pause or
         resume, rewind, skip, stop, and Show in chat. Every control names
         itself in its tooltip and aria-label. */
      ".dsh-csva-bar{position:fixed;right:20px;top:calc(var(--dsh-frame-top-clearance,0px) + 88px);z-index:20;display:inline-flex;align-items:center;gap:2px;box-sizing:border-box;max-width:calc(100vw - 40px);padding:6px;border:.5px solid var(--dsw-alias-border-l1);border-radius:12px;background:var(--dsw-alias-bg-overlay);color:var(--dsw-alias-label-primary);box-shadow:0 6px 20px rgb(0 0 0 / 14%);pointer-events:auto}",
      ".dsh-csva-barControl{flex:none;display:inline-flex;align-items:center;justify-content:center;width:26px;height:26px;padding:0;border:none;border-radius:var(--dsw-radius-sm,6px);background:0 0;color:var(--dsw-alias-label-secondary);cursor:pointer}",
      "@media (hover:hover){.dsh-csva-barControl:hover{background:var(--dsw-alias-interactive-bg-hover);color:var(--dsw-alias-label-primary)}}",
      ".dsh-csva-barControl:focus-visible{outline:var(--dsw-focus-ring-width,2px) solid var(--dsw-focus-ring-color,var(--dsw-alias-brand-primary));outline-offset:2px}",
      ".dsh-csva-barStop{color:var(--dsw-alias-label-primary)}",
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
      sequence: 1,
      /* Chain that serializes beginReading calls. Each call appends to it,
         so overlapping starts queue instead of racing. */
      beginChain: Promise.resolve(),
      /* Bumped by a stop, so a reading queued behind the stop does not start. */
      stopEpoch: 0,
      /* Abort controller for the in-flight /speak request. */
      speakAbort: null
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
      playback.speakAbort = null;
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
      excerpt: readStoredFlag(STORAGE.excerpt, false),
      excerptMode: readStoredChoice(STORAGE.excerptMode, EXCERPT_MODES, "both"),
      excerptTop: readStoredCount(STORAGE.excerptTop, EXCERPT_DEFAULT),
      excerptBottom: readStoredCount(STORAGE.excerptBottom, EXCERPT_DEFAULT),
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

    function readStoredChoice(key, choices, fallback) {
      try {
        const stored = window.localStorage.getItem(key);
        if (stored === null || choices.indexOf(stored) === -1) return fallback;
        return stored;
      } catch {
        return fallback;
      }
    }

    function readStoredCount(key, fallback) {
      try {
        const stored = window.localStorage.getItem(key);
        if (stored === null) return fallback;
        const value = Number(stored);
        if (!Number.isFinite(value)) return fallback;
        return clampExcerptCount(value);
      } catch {
        return fallback;
      }
    }

    function clampExcerptCount(value) {
      return Math.min(EXCERPT_MAX, Math.max(EXCERPT_MIN, Math.round(value)));
    }

    function storePreference(key, value) {
      try {
        window.localStorage.setItem(key, value);
      } catch {
        /* Private mode: the control still works for this page. */
      }
    }

    function setPreference(name, enabled) {
      preferences[name] = enabled;
      storePreference(STORAGE[name], enabled ? "1" : "0");
      /* The status poll runs for the whole reading, so the bar keeps its pause
         state and Show in chat keeps its target. Disabling captions only drops
         the inline highlight. */
      if (name === "captions" && !enabled) clearHighlight();
      emit(preferences.listeners);
    }

    function setExcerptMode(mode) {
      if (EXCERPT_MODES.indexOf(mode) === -1) return;
      preferences.excerptMode = mode;
      storePreference(STORAGE.excerptMode, mode);
      emit(preferences.listeners);
    }

    function setExcerptCount(name, value) {
      const count = clampExcerptCount(value);
      preferences[name] = count;
      storePreference(STORAGE[name], String(count));
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
      /* The difference-blend pill: one fixed div with white fill and
         mix-blend-mode difference. Appended to document.documentElement so
         the word is inside the inverted backdrop. */
      mark: null,
      paintedRange: null,
      tween: null,
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

    /* The first occurrence of one fragment word inside the normal look-ahead. */
    function findForward(tokens, norm, from) {
      const highest = Math.min(tokens.length, from + ALIGN_WINDOW);
      for (let scan = from; scan < highest; scan += 1) {
        if (tokens[scan].norm === norm) return scan;
      }
      return -1;
    }

    /* Whether the words after one fragment word follow it inside the normal
       look-ahead. `at` is the candidate token of the first word. */
    function runMatches(fragmentWords, tokens, index, at, needed) {
      let cursor = at + 1;
      let matched = 1;
      for (let next = index + 1; next < fragmentWords.length && matched < needed; next += 1) {
        const norm = fragmentWords[next].norm;
        if (norm === "") continue;
        const found = findForward(tokens, norm, cursor);
        if (found < 0) return false;
        cursor = found + 1;
        matched += 1;
      }
      return matched >= needed;
    }

    /* Resume after a caption fragment skipped rendered text. Partial auto-read
       speaks the top and bottom paragraphs, so its caption text jumps from the
       top block to the bottom block with the omitted middle absent from the
       spoken words. The reply still renders that middle, so the normal window
       cannot reach the next spoken word. Accept a longer jump only when the
       skipped word starts a run the reply actually contains; a missing or
       rewritten word otherwise stays unmapped instead of snapping elsewhere. */
    function resyncForward(fragmentWords, tokens, index, from) {
      let remaining = 0;
      for (let next = index; next < fragmentWords.length; next += 1) {
        if (fragmentWords[next].norm !== "") remaining += 1;
      }
      /* A jump has to prove itself with a run; one or two leftover words cannot. */
      if (remaining < RESYNC_LOOKAHEAD) return -1;
      const norm = fragmentWords[index].norm;
      for (let scan = from; scan < tokens.length; scan += 1) {
        if (tokens[scan].norm !== norm) continue;
        if (runMatches(fragmentWords, tokens, index, scan, RESYNC_LOOKAHEAD)) return scan;
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
        let found = findForward(tokens, norm, token);
        if (found < 0) found = resyncForward(fragmentWords, tokens, index, token);
        if (found < 0) continue;
        map[index] = found;
        token = found + 1;
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
      /* The physical spans may be gone, so the painted pill must not outlive
         them. The caller repaints on the next caption snapshot. */
      clearHighlight();
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

    /* Clear the difference-blend pill and any running tween. */
    function clearHighlight() {
      cancelTween();
      if (reading.mark !== null) {
        reading.mark.remove();
        reading.mark = null;
      }
      reading.paintedRange = null;
    }

    /* Drop the mark element. */
    function removeMark() {
      if (reading.mark !== null) {
        reading.mark.remove();
        reading.mark = null;
      }
    }

    /* Whether the pill animates: animation switch on, mark exists, reduced
       motion off. */
    function markGlides() {
      return (
        preferences.animation &&
        reading.mark !== null &&
        !window.matchMedia("(prefers-reduced-motion: reduce)").matches
      );
    }

    /* CSS cubic-bezier(x1, y1, x2, y2) as an easing function over [0, 1]. */
    function cubicBezier(x1, y1, x2, y2) {
      const axis = function (a1, a2, t) {
        return ((1 - 3 * a2 + 3 * a1) * t + (3 * a2 - 6 * a1)) * t * t + 3 * a1 * t;
      };
      return function (x) {
        if (x <= 0) return 0;
        if (x >= 1) return 1;
        let low = 0;
        let high = 1;
        let t = x;
        for (let i = 0; i < 20; i += 1) {
          t = (low + high) / 2;
          if (axis(x1, x2, t) < x) low = t;
          else high = t;
        }
        return axis(y1, y2, t);
      };
    }

    /** Extra swift easing in: most of the travel happens early, then it settles. */
    const easeGlide = cubicBezier(0, 0.5, 0, 1);

    /* Get numeric value from a CSS pixel string. */
    function markOffset(value) {
      if (value === undefined || value === null || value === "") return NaN;
      const parsed = parseFloat(value);
      return Number.isNaN(parsed) ? NaN : parsed;
    }

    /* The layout rects of one word. Range.getClientRects is missing in jsdom
       and can throw on a stale node, so a thrown call reads as no rect. */
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

    /* Ensure the single difference-blend pill exists, positioned at the
       target word's viewport rect. The pill is fixed, so it needs no scroll
       offset. The pill is appended to document.documentElement so the word
       itself is inside the inverted backdrop. */
    function ensureMark(range) {
      const rects = wordRects(range);
      if (rects.length === 0) {
        removeMark();
        return;
      }
      const rect = rects[0];
      const left = Math.round(rect.left - MARK_PADDING_X) + "px";
      const top = Math.round(rect.top - MARK_PADDING_Y) + "px";
      const width = Math.round(rect.width + MARK_PADDING_X * 2) + "px";
      const height = Math.round(rect.height + MARK_PADDING_Y * 2) + "px";
      if (reading.mark !== null && reading.mark.isConnected) {
        /* Update the existing pill to the new word's position. */
        reading.mark.style.left = left;
        reading.mark.style.top = top;
        reading.mark.style.width = width;
        reading.mark.style.height = height;
        return;
      }
      removeMark();
      const mark = document.createElement("div");
      mark.className = "dsh-csva-mark";
      mark.setAttribute("aria-hidden", "true");
      mark.style.position = "fixed";
      mark.style.borderRadius = MARK_RADIUS_PX + "px";
      mark.style.background = "#fff";
      mark.style.mixBlendMode = "difference";
      mark.style.pointerEvents = "none";
      mark.style.boxSizing = "border-box";
      mark.style.zIndex = "2147483646";
      mark.style.display = "block";
      mark.style.left = left;
      mark.style.top = top;
      mark.style.width = width;
      mark.style.height = height;
      document.documentElement.appendChild(mark);
      reading.mark = mark;
    }

    /* Paint the difference-blend pill for the given token, with optional
       glide animation from the previous position. */
    function paintWord(token) {
      const range = tokenRange(token);
      if (!preferences.captions && !reading.forceCaptions) {
        followRange(token, range);
        return;
      }
      const previous = reading.paintedRange;
      const fromLeft = markOffset(reading.mark?.style.left);
      const fromTop = markOffset(reading.mark?.style.top);
      const fromWidth = markOffset(reading.mark?.style.width);
      const settlesInstantly = !markGlides() || previous === null || reading.mark === null;
      ensureMark(range);
      if (settlesInstantly) {
        cancelTween();
        reading.paintedRange = range;
      } else {
        startTween(range, { left: fromLeft, top: fromTop, width: fromWidth });
      }
      /* Learn the scrollport even with follow off, so a manual scroll keeps
         the fixed pill on the word. */
      ensureScroller(token);
      followRange(token, range);
    }

    /* Glide the pill position and width together in one rAF loop. */
    function startTween(range, from) {
      cancelTween();
      const mark = reading.mark;
      const rects = wordRects(range);
      if (mark === null || rects.length === 0) {
        reading.paintedRange = range;
        return;
      }
      const toLeft = markOffset(mark.style.left);
      const toTop = markOffset(mark.style.top);
      const toWidth = markOffset(mark.style.width);
      if (
        Number.isNaN(toLeft) ||
        Number.isNaN(toTop) ||
        Number.isNaN(toWidth) ||
        Number.isNaN(from.left) ||
        Number.isNaN(from.top) ||
        Number.isNaN(from.width)
      ) {
        reading.paintedRange = range;
        return;
      }
      const dx = from.left - toLeft;
      const dy = from.top - toTop;
      const w0 = from.width;
      const w1 = toWidth;
      const tween = {
        raf: 0,
        elapsed: 0,
        last: null,
        mark: mark,
        range: range,
        dx: dx,
        dy: dy,
        w0: w0,
        w1: w1
      };
      mark.style.transform = "translate(" + dx + "px," + dy + "px)";
      mark.style.width = w0 + "px";
      const frame = function () {
        if (reading.tween !== tween) return;
        if (!range.startContainer.isConnected) {
          cancelTween();
          return;
        }
        const now = performance.now();
        const step = tween.last === null ? 0 : Math.min(now - tween.last, MARK_FRAME_CAP_MS);
        tween.last = now;
        tween.elapsed = Math.min(tween.elapsed + step, MARK_TRANSITION_MS);
        const t = tween.elapsed / MARK_TRANSITION_MS;
        const p = easeGlide(t);
        mark.style.transform = "translate(" + dx * (1 - p) + "px," + dy * (1 - p) + "px)";
        mark.style.width = w0 + (w1 - w0) * p + "px";
        if (t < 1) {
          tween.raf = window.requestAnimationFrame(frame);
          return;
        }
        cancelTween();
        reading.paintedRange = tween.range;
        /* The target rect can move mid-glide (follow scroll, host layout). */
        repositionMark();
      };
      reading.tween = tween;
      tween.raf = window.requestAnimationFrame(frame);
    }

    function cancelTween() {
      const tween = reading.tween;
      if (tween === null) return;
      reading.tween = null;
      window.cancelAnimationFrame(tween.raf);
      if (tween.mark !== null && tween.mark.isConnected) {
        tween.mark.style.transform = "";
      }
    }

    /* A React re-render can drop the mark while the same word plays.
       Re-assert the painted word when that happened. */
    function keepWordPainted() {
      const paint = preferences.captions || reading.forceCaptions;
      if (!paint || reading.activeToken < 0 || reading.tokens === null) return;
      const token = reading.tokens[reading.activeToken];
      if (!token.node.isConnected) return;
      const placed =
        reading.mark !== null && reading.mark.isConnected && reading.paintedRange !== null;
      if (!placed) paintWord(token);
    }

    /* A fixed pill does not travel with the chat scrollport or the window, so
       re-read the active word's rect after a scroll. A running glide owns the
       transform, and its end repositions instead. */
    function repositionMark() {
      if (reading.mark === null || !reading.mark.isConnected) return;
      if (reading.tween !== null) return;
      if (reading.activeToken < 0 || reading.tokens === null) return;
      const token = reading.tokens[reading.activeToken];
      if (!token.node.isConnected) return;
      ensureMark(tokenRange(token));
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
      /* The fixed pill must stay on the word when the chat content moves. */
      scroller.addEventListener("scroll", repositionMark, { passive: true });
    }

    function unwatchScroller() {
      const scroller = reading.watchedScroller;
      if (scroller === null) return;
      scroller.removeEventListener("wheel", pauseFollow);
      scroller.removeEventListener("touchstart", pauseFollow);
      scroller.removeEventListener("keydown", pauseFollowKey, true);
      scroller.removeEventListener("scroll", repositionMark);
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
        reading.speechTokens.map(function (token) {
          return token.norm;
        })
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

    function post(path, body, signal) {
      return fetch(CONTROL_URL + path, {
        method: "POST",
        mode: "no-cors",
        cache: "no-store",
        headers: { "content-type": "text/plain" },
        body: JSON.stringify(body),
        signal: signal
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

    function stopReading(requestId, signal) {
      return post("/stop", { request_id: requestId }, signal);
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
       reading passes its cloned range instead of a message id and node key.
       Calls are serialized through playback.beginChain so overlapping starts
       cannot post two /speak calls for the same stop. */
    function beginReading(messageId, nodeKey, speech, range = null) {
      const epoch = playback.stopEpoch;
      const run = function () {
        /* A stop between the call and the turn in the chain cancels this start. */
        if (epoch !== playback.stopEpoch) return null;
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
          /* Create an AbortController for this /speak request so stopReadingNow
             can abort it if the user stops playback before it finishes. */
          const controller = new AbortController();
          playback.speakAbort = controller;
          return post(
            "/speak",
            { text: speech, request_id: requestId, wait: true },
            controller.signal
          ).then(function (finished) {
            if (playback.speakAbort === controller) playback.speakAbort = null;
            if (playback.requestId !== requestId) return null;
            forget();
            /* An aborted request is the stop the user asked for, not a failure. */
            if (controller.signal.aborted) return null;
            return finished ? null : "error.offline";
          });
        });
      };
      // Append to the chain so concurrent calls queue up.
      const next = playback.beginChain.then(run, run);
      playback.beginChain = next;
      return next;
    }

    function stopReadingNow() {
      const requestId = playback.requestId;
      /* Bump the epoch even when idle, so a reading still queued behind an
         earlier start cannot begin after this stop. */
      playback.stopEpoch += 1;
      if (requestId === null) {
        forget();
        return Promise.resolve();
      }
      /* Abort the in-flight /speak request so the server stops generation. */
      if (playback.speakAbort !== null) {
        playback.speakAbort.abort();
        playback.speakAbort = null;
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

    /* Keep the first and last paragraphs of a reply for automatic reading.
       Paragraphs are the blank-line-separated blocks left after toSpeech. */
    function excerptSpeech(text, mode, topCount, bottomCount) {
      const blocks = text
        .split(/\n\s*\n/)
        .map(function (block) {
          return block.trim();
        })
        .filter(function (block) {
          return block !== "";
        });
      if (blocks.length < 2) return text;
      const top = clampExcerptCount(topCount);
      const bottom = clampExcerptCount(bottomCount);
      if (mode === "top") return blocks.slice(0, top).join("\n\n");
      if (mode === "bottom") return blocks.slice(-bottom).join("\n\n");
      if (top + bottom >= blocks.length) return text;
      return blocks.slice(0, top).concat(blocks.slice(-bottom)).join("\n\n");
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
     * icon-only controls follow the host theme. Every mark is filled, so the
     * set reads as one family at bar size.
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

    /* The rewind and skip marks: the app's own filled seek icons, the exact
       Material artwork its player buttons use (fast_rewind / fast_forward;
       src/lib/components/icons/skip-back.svelte and skip-forward.svelte,
       google/material-design-icons, Apache-2.0). One path per mark, drawn at
       16 px in currentColor so the controls follow the host theme. */
    function SkipGlyph(props) {
      const back = props.back === true;
      return h(
        "svg",
        {
          viewBox: "0 0 24 24",
          width: 16,
          height: 16,
          className: "dsh-csva-glyph",
          "aria-hidden": true,
          focusable: "false"
        },
        h("path", {
          d: back
            ? "M11 18V6l-8.5 6 8.5 6zm.5-6l8.5 6V6l-8.5 6z"
            : "M4 18l8.5-6L4 6v12zm9-12v12l8.5-6L13 6z",
          fill: "currentColor"
        })
      );
    }

    /* Show in chat: a solid speech bubble, the same mark chat surfaces use.
       The 1 px round-joined stroke rounds the corners like the other fills. */
    function ChatGlyph() {
      return h(
        ControlIcon,
        null,
        h("path", {
          d: "M3.6 3.1H12.4A1.9 1.9 0 0 1 14.3 5V9A1.9 1.9 0 0 1 12.4 10.9H7.6L4.9 13.3V10.9H3.6A1.9 1.9 0 0 1 1.7 9V5A1.9 1.9 0 0 1 3.6 3.1Z",
          fill: "currentColor",
          stroke: "currentColor",
          strokeWidth: 1,
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

    /* The Session facts the watcher needs, as one comparable value: whether the
       Session's history finished opening, and whether a turn runs right now.
       Chat history arrives after the dock mounts, so the reply ids it carries
       look new; only a turn that runs while watched is a new turn. */
    function sessionWatch(snapshot) {
      if (snapshot == null) return "cold";
      if (snapshot.openState !== "open") return "loading";
      return snapshot.running ? "running" : "ready";
    }

    /**
     * Per-session watcher under the composer: when a turn completes and
     * auto-read is on, speak its final reply. It renders nothing.
     */
    function AutoReader(props) {
      const useChat = props.useChat;
      const useSession = props.useSession;
      const latest = React.useRef(null);
      const latestId = useChat
        ? useChat(function (snapshot) {
            latest.current = closingReply(snapshot);
            return latest.current === null ? null : latest.current.messageId;
          })
        : null;
      const watch = useSession ? useSession(sessionWatch) : "ready";

      /* Reply id already accounted for, so one reply reads once. */
      const seen = React.useRef(null);
      /* Set once the Session's history opened: reply ids before this point are
         loaded history, and no turn has run for them. */
      const primed = React.useRef(false);
      /* Set while a turn runs, and kept while an auto-read starts. A reply after
         history opened, without a turn seen running, is loaded history too. */
      const armed = React.useRef(false);

      React.useEffect(
        function () {
          if (watch === "running") armed.current = true;
          if (!primed.current) {
            if (watch !== "ready" && watch !== "running") return;
            primed.current = true;
            seen.current = latestId;
            return;
          }
          if (latestId === null || latestId === seen.current) return;
          seen.current = latestId;
          if (!armed.current) return;
          /* A queued turn keeps the Session running, so a reply that lands while
             another turn runs leaves the watcher ready for that turn's reply. */
          armed.current = watch === "running";
          if (!preferences.autoRead) return;
          const found = latest.current;
          if (found === null) return;
          /* The watcher reads the same rendered Markdown as the action, so it
             must strip the markup exactly the same way. */
          const full = toSpeech(assistantText(found.blocks));
          const speech = preferences.excerpt
            ? excerptSpeech(
                full,
                preferences.excerptMode,
                preferences.excerptTop,
                preferences.excerptBottom
              )
            : full;
          if (speech === "") return;
          void beginReading(found.messageId, found.key, speech).then(function (error) {
            if (error !== null) console.error("[copyspeak-voice] auto-read failed:", error);
          });
        },
        [latestId, watch]
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

    function snapshotPreferences() {
      return {
        autoRead: preferences.autoRead,
        captions: preferences.captions,
        follow: preferences.follow,
        animation: preferences.animation,
        excerpt: preferences.excerpt,
        excerptMode: preferences.excerptMode,
        excerptTop: preferences.excerptTop,
        excerptBottom: preferences.excerptBottom
      };
    }

    function usePreferencesState() {
      const [snapshot, setSnapshot] = React.useState(snapshotPreferences);
      React.useEffect(function () {
        return subscribe(preferences.listeners, function () {
          setSnapshot(snapshotPreferences());
        });
      }, []);
      return snapshot;
    }

    function useControlServerStatus() {
      const [online, setOnline] = React.useState(null);
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
      return online;
    }

    function statusCopy(online) {
      return {
        key: online === null ? "status.checking" : online ? "status.online" : "status.offline",
        state: online === null ? "checking" : online ? "online" : "offline"
      };
    }

    function SwitchRow(props) {
      const t = props.t;
      return h(
        "li",
        { className: "dsh-csva-row" },
        h(
          "span",
          { className: "dsh-csva-rowText" },
          h(
            "span",
            { className: "dsh-csva-rowTitle" },
            t(props.title),
            props.badge === true
              ? h("span", { className: "dsh-csva-badge" }, t("excerpt.badge"))
              : null
          ),
          h("span", { className: "dsh-csva-rowDescription" }, t(props.description))
        ),
        h(Switch, {
          checked: props.checked,
          label: t(props.title) + ": " + t(props.checked ? "switch.on" : "switch.off"),
          onChange: props.onChange
        })
      );
    }

    function ExcerptCountField(props) {
      const t = props.t;
      return h(
        "label",
        { className: "dsh-csva-field" },
        h("span", { className: "dsh-csva-fieldLabel" }, t(props.label)),
        h("input", {
          className: "dsh-csva-number",
          type: "number",
          min: String(EXCERPT_MIN),
          max: String(EXCERPT_MAX),
          step: "1",
          value: String(props.value),
          onChange: function (event) {
            const next = event.target.valueAsNumber;
            if (!Number.isFinite(next)) return;
            props.onChange(next);
          }
        })
      );
    }

    function ExcerptOptions(props) {
      const t = props.t;
      const mode = props.state.excerptMode;
      return h(
        "div",
        { className: "dsh-csva-options" },
        h(
          "label",
          { className: "dsh-csva-field" },
          h("span", { className: "dsh-csva-fieldLabel" }, t("excerpt.mode")),
          h(
            "select",
            {
              className: "dsh-csva-select",
              value: mode,
              onChange: function (event) {
                setExcerptMode(event.target.value);
              }
            },
            h("option", { value: "top" }, t("excerpt.mode.top")),
            h("option", { value: "bottom" }, t("excerpt.mode.bottom")),
            h("option", { value: "both" }, t("excerpt.mode.both"))
          )
        ),
        mode === "bottom"
          ? null
          : h(ExcerptCountField, {
              t: t,
              label: "excerpt.top",
              value: props.state.excerptTop,
              onChange: function (next) {
                setExcerptCount("excerptTop", next);
              }
            }),
        mode === "top"
          ? null
          : h(ExcerptCountField, {
              t: t,
              label: "excerpt.bottom",
              value: props.state.excerptBottom,
              onChange: function (next) {
                setExcerptCount("excerptBottom", next);
              }
            })
      );
    }

    /** CopySpeak options and app status on the CopySpeak sidebar panel. */
    function CopySpeakScreen(props) {
      const t = props.t;
      const state = usePreferencesState();
      const status = statusCopy(useControlServerStatus());

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
          { className: "dsh-csva-status", "data-state": status.state, role: "status" },
          h("span", { className: "dsh-csva-dot" }),
          h("span", null, t(status.key))
        ),
        h("ul", { className: "dsh-csva-rows" }, [
          h(SwitchRow, {
            key: "auto-read",
            t: t,
            title: "autoRead.title",
            description: "autoRead.description",
            checked: state.autoRead,
            onChange: function () {
              setPreference("autoRead", !preferences.autoRead);
            }
          }),
          h(SwitchRow, {
            key: "captions",
            t: t,
            title: "captions.title",
            description: "captions.description",
            checked: state.captions,
            onChange: function () {
              setPreference("captions", !preferences.captions);
            }
          }),
          h(SwitchRow, {
            key: "animation",
            t: t,
            title: "animation.title",
            description: "animation.description",
            checked: state.animation,
            onChange: function () {
              setPreference("animation", !preferences.animation);
            }
          }),
          h(SwitchRow, {
            key: "follow",
            t: t,
            title: "follow.title",
            description: "follow.description",
            checked: state.follow,
            onChange: function () {
              setPreference("follow", !preferences.follow);
            }
          }),
          state.autoRead
            ? h(SwitchRow, {
                key: "excerpt",
                t: t,
                title: "excerpt.title",
                description: "excerpt.description",
                badge: true,
                checked: state.excerpt,
                onChange: function () {
                  setPreference("excerpt", !preferences.excerpt);
                }
              })
            : null,
          state.autoRead && state.excerpt
            ? h(
                "li",
                { key: "excerpt-options", className: "dsh-csva-row dsh-csva-rowOptions" },
                h(ExcerptOptions, { t: t, state: state })
              )
            : null
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

        /* A host that scrolls the document moves the words too. The chat
           scrollport gets its own listener in watchScroller. */
        ctx.effect(function () {
          window.addEventListener("scroll", repositionMark, { passive: true });
          return function () {
            window.removeEventListener("scroll", repositionMark);
          };
        }, PACKAGE + ": pill scroll");

        ctx.effect(function () {
          return function () {
            /* This plugin owns its reading; unloading it must stop the server
               playback, not just the timer. */
            const requestId = playback.requestId;
            if (playback.speakAbort !== null) {
              playback.speakAbort.abort();
              playback.speakAbort = null;
            }
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
