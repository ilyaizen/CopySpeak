<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import { Play, Settings, Clock, AudioLines, Cpu } from "@lucide/svelte";
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
      href: "/",
      icon: Play
    },
    ...(historyEnabled
      ? [
          {
            id: "history",
            label: $_("header.history"),
            href: "/history",
            icon: Clock
          }
        ]
      : []),
    {
      id: "voices",
      label: $_("header.voices"),
      href: "/voices",
      icon: AudioLines
    },
    {
      id: "engines",
      label: $_("header.engines"),
      href: "/engines",
      icon: Cpu
    },
    {
      id: "settings",
      label: $_("header.settings"),
      href: "/settings",
      icon: Settings
    }
  ]);
</script>

<header class="border-border bg-background z-50 border-b" data-testid="app-header">
  <div class="flex min-w-0 flex-wrap items-center justify-between gap-3 px-4 py-3 sm:px-6">
    <div class="flex items-center gap-3">
      <a href="/" class="flex items-center gap-3">
        <img src="/app-logo.png" alt={$_("app.logoAlt")} class="h-10 w-10" />
        <div>
          <h1 class="text-foreground font-mono text-xl font-semibold tracking-tight">CopySpeak</h1>
          <p class="text-muted-foreground text-xs">{$_("header.tagline")}</p>
        </div>
      </a>
    </div>
    <div class="max-w-full min-w-0">
      <nav aria-label={$_("header.navAria")} class="flex flex-wrap items-center gap-1">
        {#each navItems as item}
          {@const isActive =
            item.href === "/" ? page.url.pathname === "/" : page.url.pathname.startsWith(item.href)}
          {@const Icon = item.icon}
          <a
            href={item.href}
            data-testid="nav-{item.id}"
            class="focus-visible:ring-ring inline-flex items-center justify-center rounded-md px-2 py-2 text-sm font-medium whitespace-nowrap transition-colors focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50 {isActive
              ? 'bg-muted text-foreground'
              : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
            aria-current={isActive ? "page" : undefined}
          >
            <div class="flex items-center gap-2">
              <Icon size={14} />
              <span>{item.label}</span>
            </div>
          </a>
        {/each}
      </nav>
    </div>
  </div>
</header>
