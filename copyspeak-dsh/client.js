/**
 * Browser half of the CopySpeak voice bundle.
 *
 * Adds a speaker action to every finalized assistant message, plus an
 * auto-read toggle at the DSH sidebar foot. Both surfaces share one reading
 * path to the CopySpeak control server:
 *
 *   POST http://127.0.0.1:43117/speak
 *   {"text": "...", "request_id": "dsh.<stamp>.<n>", "wait": true}
 *
 * The request uses `mode: 'no-cors'` and a `text/plain` content type, so the
 * browser sends no CORS preflight; CopySpeak parses the JSON body without
 * reading the content type. The response is opaque, so a success only proves
 * the request reached the app. `wait: true` makes it resolve at playback end,
 * which restores the idle action state.
 */
window.__ModuleLoader__.load({
  id: "@local/copyspeak-voice",
  factory(require) {
    const React = require("react");
    const h = React.createElement;

    const NS = "copyspeak-voice";
    const PACKAGE = "@local/copyspeak-voice";
    const CONTROL_URL = "http://127.0.0.1:43117";
    const STORE_KEY = "dsh.copyspeak-voice.auto-read";
    const STYLE_KEY = PACKAGE + "/action.css";

    /*
     * CopySpeak app-logo mark: the speaker and its two sound waves, with the
     * gradient card and every layer color dropped. One currentColor fill, so
     * the glyph follows the host theme in light and dark modes.
     */
    const GLYPH_VIEW_BOX = "14.86 14.717 48.611 48.611";
    const GLYPH_PATHS = [
      "m37.35 20.74c-0.74-0.04-1.46 0.21-2.02 0.71l-7.68 7.13c-0.24 0.23-0.56 0.37-0.9 0.39l-5.58 0.21c-2.55 0.1-4.51 2.23-4.51 5v7.99c0 2.81 2.1 4.95 4.74 5.09l5.3 0.28c0.28 0.02 0.54 0.15 0.74 0.35l7.94 8.12c0.71 0.71 1.67 1.04 2.55 1.02 1.52-0.04 2.65-1.2 2.65-2.76v-30.18c0-1.84-1.65-3.27-3.23-3.35z",
      "m49.61 19.43c-1.5-0.74-3.32-0.37-4.11 1.04-0.8 1.45-0.21 3.13 1.27 3.97 5.25 2.81 8.39 8.04 8.39 14.44 0 6.47-3.45 12.05-8.57 14.61-1.47 0.81-2 2.47-1.33 3.85 0.72 1.4 2.78 2.03 4.44 1.41 7.13-3.14 11.97-10.79 11.97-19.57 0-8.67-5.02-16.48-12.06-19.75z",
      "m46.38 28.77c-1.27-0.75-2.96-0.42-3.67 0.89-0.75 1.28-0.3 2.86 0.86 3.78 1.73 1.33 2.77 3.26 2.77 5.48 0 2.27-1.14 4.28-2.83 5.45-1.2 0.9-1.48 2.53-0.7 3.78 0.84 1.23 2.55 1.74 3.95 1.16 3.49-1.78 5.54-5.56 5.54-9.89 0-4.62-2.49-8.79-5.92-10.65z"
    ];

    const en = {
      "action.read": "Read aloud with CopySpeak",
      "action.stop": "Stop reading",
      "error.empty": "Nothing to read aloud",
      "error.offline": "CopySpeak is not running",
      "error.failed": "CopySpeak request failed",
      "toggle.label": "Read replies aloud",
      "toggle.on": "On",
      "toggle.off": "Off"
    };

    const CSS = [
      ".dsh-csva-glyph{display:block}",
      ".dsh-csva-root{display:inline-flex;align-items:center;gap:8px}",
      ".dsh-csva-button{width:28px;height:28px;border-radius:var(--dsw-radius-sm,6px);border:none;background:0 0;padding:6px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:var(--dsw-alias-label-secondary);}",
      ".dsh-csva-button svg{width:15px;height:15px}",
      ".dsh-csva-button:hover:not(:disabled){background:var(--dsw-alias-bg-layer-2);color:var(--dsw-alias-label-primary)}",
      ".dsh-csva-button[data-active]{color:var(--dsw-alias-brand-primary)}",
      ".dsh-csva-button:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:2px}",
      ".dsh-csva-button:disabled{opacity:.4;cursor:default}",
      ".dsh-csva-note{font-size:12px;line-height:16px;color:var(--dsw-alias-state-error-primary);white-space:nowrap}",
      ".dsh-csva-foot{flex:none;display:flex;align-items:center;width:100%;height:42px;margin:8px 0 0;position:relative}",
      ".dsh-csva-footRail{width:36px;height:36px;margin:0}",
      ".dsh-csva-footButton{width:calc(100% + 4px);height:42px;margin:0 -2px;padding:0 10px 0 8px;border:none;border-radius:12px;background:0 0;color:var(--dsw-alias-label-secondary);font:inherit;font-size:14px;display:inline-flex;align-items:center;gap:8px;cursor:pointer;overflow:hidden}",
      ".dsh-csva-footButton:hover{background:var(--dsw-alias-interactive-bg-hover,var(--dsw-alias-bg-layer-2));color:var(--dsw-alias-label-primary)}",
      ".dsh-csva-footButton[data-on]{color:var(--dsw-alias-label-primary)}",
      ".dsh-csva-footButton[data-on] .dsh-csva-glyph{color:var(--dsw-alias-brand-primary)}",
      ".dsh-csva-footButton:focus-visible{outline:2px solid var(--dsw-alias-brand-primary);outline-offset:2px}",
      ".dsh-csva-footRail .dsh-csva-footButton{width:36px;height:36px;margin:0;padding:0;border-radius:50%;justify-content:center}",
      ".dsh-csva-footLabel{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}",
      ".dsh-csva-footState{margin-left:auto;flex:none;font-size:12px;line-height:16px;color:var(--dsw-alias-label-tertiary)}"
    ].join("\n");

    /* One reading at a time; every surface observes this record. */
    const playback = {
      messageId: null,
      requestId: null,
      listeners: new Set(),
      sequence: 1
    };

    function emit(listeners) {
      for (const listener of Array.from(listeners)) listener();
    }

    function subscribe(listeners, listener) {
      listeners.add(listener);
      return function () {
        listeners.delete(listener);
      };
    }

    function remember(messageId, requestId) {
      playback.messageId = messageId;
      playback.requestId = requestId;
      emit(playback.listeners);
    }

    function forget() {
      playback.messageId = null;
      playback.requestId = null;
      emit(playback.listeners);
    }

    function nextRequestId() {
      const stamp = Date.now().toString(36);
      const sequence = playback.sequence.toString(36);
      playback.sequence += 1;
      return "dsh." + stamp + "." + sequence;
    }

    /* Auto-read preference: page-local, so the toggle keeps its state across
       reloads without a Host round trip. */
    const autoRead = { enabled: readStoredFlag(), listeners: new Set() };

    function readStoredFlag() {
      try {
        return window.localStorage.getItem(STORE_KEY) === "1";
      } catch {
        return false;
      }
    }

    function setAutoRead(enabled) {
      autoRead.enabled = enabled;
      try {
        window.localStorage.setItem(STORE_KEY, enabled ? "1" : "0");
      } catch {
        /* Private mode: the toggle still works for this page. */
      }
      emit(autoRead.listeners);
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

    function stopReading(requestId) {
      return post("/stop", { request_id: requestId });
    }

    /* Start one reading and resolve with its failure code, or null. */
    function beginReading(messageId, text) {
      const previous = playback.requestId;
      const stopping = previous === null ? Promise.resolve(true) : stopReading(previous);
      return stopping.then(function (stopped) {
        if (!stopped) {
          forget();
          return "error.offline";
        }
        const requestId = nextRequestId();
        remember(messageId, requestId);
        return post("/speak", { text: text, request_id: requestId, wait: true }).then(
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

    function CsGlyph(props) {
      const size = props.size;
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

    function StopIcon() {
      return h(
        "svg",
        { viewBox: "0 0 24 24", "aria-hidden": true, focusable: "false" },
        h("rect", { x: 6, y: 6, width: 12, height: 12, rx: 2, fill: "currentColor" })
      );
    }

    function CopySpeakAction(props) {
      const messageId = props.messageId;
      const useChat = props.useChat;
      const t = props.t;

      /* Selector stays cheap: Chat snapshots change on every streamed token. */
      const text = useChat
        ? useChat(function (snapshot) {
            const node = assistantNode(snapshot, messageId);
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
      const disabled = busy || (!active && !hasText);

      function onClick() {
        if (busy) return;
        if (active) {
          setBusy(true);
          void stopReadingNow().then(function () {
            setBusy(false);
          });
          return;
        }
        const speech = toSpeech(text);
        if (speech === "") {
          fail("error.empty");
          return;
        }
        setBusy(true);
        void beginReading(messageId, speech).then(function (error) {
          setBusy(false);
          if (error !== null) fail(error);
        });
      }

      return h(
        "span",
        { className: "dsh-csva-root" },
        h(
          "button",
          {
            type: "button",
            className: "dsh-csva-button",
            "aria-label": label,
            "aria-pressed": active,
            title: label,
            "data-active": active || undefined,
            disabled: disabled,
            onClick: onClick
          },
          active ? h(StopIcon) : h(CsGlyph, { size: 15 })
        ),
        failure === null
          ? null
          : h("span", { className: "dsh-csva-note", role: "status" }, t(failure))
      );
    }

    /** Sidebar-foot switch for reading each new reply as it settles. */
    function AutoReadToggle(props) {
      const wide = props.wide;
      const t = props.t;
      const [enabled, setEnabledState] = React.useState(autoRead.enabled);

      React.useEffect(function () {
        return subscribe(autoRead.listeners, function () {
          setEnabledState(autoRead.enabled);
        });
      }, []);

      const state = enabled ? t("toggle.on") : t("toggle.off");
      const name = t("toggle.label");
      return h(
        "div",
        { className: wide ? "dsh-csva-foot" : "dsh-csva-foot dsh-csva-footRail" },
        h(
          "button",
          {
            type: "button",
            className: "dsh-csva-footButton",
            "aria-pressed": enabled,
            "aria-label": name + ": " + state,
            title: name + ": " + state,
            "data-on": enabled || undefined,
            onClick: function () {
              setAutoRead(!autoRead.enabled);
            }
          },
          h(CsGlyph, { size: wide ? 16 : 18 }),
          wide ? h("span", { className: "dsh-csva-footLabel" }, name) : null,
          wide ? h("span", { className: "dsh-csva-footState" }, state) : null
        )
      );
    }

    /**
     * Per-session watcher under the composer: when a new reply settles and
     * auto-read is on, speak it. It renders nothing.
     */
    function AutoReader(props) {
      const useChat = props.useChat;
      const latest = React.useRef(null);
      const latestId = useChat
        ? useChat(function (snapshot) {
            const node = assistantNode(snapshot);
            latest.current =
              node === null
                ? null
                : {
                    messageId: node.data.finalNode.messageId,
                    text: assistantText(node.data.blocks)
                  };
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
          if (!autoRead.enabled) return;
          const found = latest.current;
          if (found === null || found.text === "") return;
          void beginReading(found.messageId, found.text).then(function (error) {
            if (error !== null) console.error("[copyspeak-voice] auto-read failed:", error);
          });
        },
        [latestId]
      );

      return null;
    }

    return {
      inject: ["slots", "locale"],
      apply(ctx) {
        ctx.effect(function () {
          const style = document.createElement("style");
          style.dataset.plugin = PACKAGE;
          style.dataset.pluginCss = STYLE_KEY;
          style.textContent = CSS;
          document.head.appendChild(style);
          return function () {
            style.remove();
          };
        }, PACKAGE + ": styles");

        ctx.effect(function () {
          return ctx.locale.register(NS, { en: en });
        }, PACKAGE + ": dictionaries");

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

        ctx.slots.inject("sidebar.footer.action", function () {
          return ctx.slots.register(
            {
              name: "sidebar.footer.action",
              id: "copyspeak-auto-read",
              order: 5,
              locale: NS
            },
            AutoReadToggle
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
      }
    };
  }
});
