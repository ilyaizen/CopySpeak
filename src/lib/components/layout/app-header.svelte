<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import { _ } from "svelte-i18n";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { isTauri } from "#lib/services/tauri.js";
  import type { AppConfig } from "#lib/types";
  import { isHistoryEffective } from "#lib/experimental-gates";

  let historyEnabled = $state(false);
  let unlistenConfig: (() => void) | null = null;
  let destroyed = false;
  let fetchSeq = 0;

  async function updateConfig() {
    const seq = ++fetchSeq;
    try {
      const config = await invoke<AppConfig>("get_config");
      if (destroyed || seq !== fetchSeq) return;
      historyEnabled = isHistoryEffective(config);
    } catch {
      // Ignore errors
    }
  }

  onMount(async () => {
    if (!isTauri) return;
    updateConfig();
    try {
      const unlisten = await listen("config-changed", () => {
        updateConfig();
      });
      if (destroyed) {
        unlisten();
      } else {
        unlistenConfig = unlisten;
      }
    } catch {
      // Ignore listener errors
    }
  });

  onDestroy(() => {
    destroyed = true;
    if (unlistenConfig) {
      unlistenConfig();
      unlistenConfig = null;
    }
  });

  const navItems = $derived([
    {
      id: "generate",
      label: $_("header.play"),
      href: "/"
    },
    ...(historyEnabled
      ? [
          {
            id: "history",
            label: $_("header.history"),
            href: "/history"
          }
        ]
      : []),
    {
      id: "voices",
      label: $_("header.voices"),
      href: "/voices"
    },
    {
      id: "engines",
      label: $_("header.engines"),
      href: "/engines"
    },
    {
      id: "settings",
      label: $_("header.settings"),
      href: "/settings"
    }
  ]);
</script>

<header class="app-surface border-border z-50 border-b" data-testid="app-header">
  <div class="flex min-w-0 flex-wrap items-center justify-between gap-3 px-4 py-3 sm:px-6">
    <div class="flex items-center gap-3">
      <a href={page.url.pathname === "/" ? undefined : "/"} class="flex items-center gap-3">
        <img src="/app-logo.png" alt={$_("app.logoAlt")} class="h-10 w-10" />
        <div>
          <h1 class="text-foreground app-brand text-xl font-semibold tracking-tight">CopySpeak</h1>
          <p class="text-muted-foreground text-xs">{$_("header.tagline")}</p>
        </div>
      </a>
    </div>
    <div class="max-w-full min-w-0">
      <nav aria-label={$_("header.navAria")} class="flex flex-wrap items-center gap-1">
        {#each navItems as item}
          {@const isActive =
            page.url.pathname === item.href ||
            (item.href !== "/" && page.url.pathname.startsWith(`${item.href}/`))}
          <a
            href={isActive ? undefined : item.href}
            data-testid="nav-{item.id}"
            class="app-nav-link focus-visible:ring-ring inline-flex min-h-11 items-center justify-center rounded-md px-2 font-medium whitespace-nowrap transition-colors focus-visible:ring-2 focus-visible:outline-none {isActive
              ? 'text-foreground'
              : 'text-muted-foreground hover:text-foreground'}"
            aria-current={isActive ? "page" : undefined}
          >
            {item.label}
          </a>
        {/each}
      </nav>
    </div>
  </div>
</header>
