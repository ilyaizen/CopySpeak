<script lang="ts">
  import { page } from "$app/state";
  import { Play, Settings, Cpu } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  const navItems = $derived([
    {
      id: "generate",
      label: $_("header.play"),
      href: "/",
      icon: Play
    },
    {
      id: "engine",
      label: $_("header.engine"),
      href: "/engine",
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

<header class="border-border bg-background sticky top-0 z-50 border-b" data-testid="app-header">
  <div class="flex items-center justify-between px-6 py-3">
    <div class="flex items-center gap-3">
      <a href="/" class="flex items-center gap-3">
        <img src="/app-logo.png" alt="CopySpeak Logo" class="h-10 w-10" />
        <div>
          <h1 class="text-foreground font-mono text-xl font-semibold tracking-tight">CopySpeak</h1>
          <p class="text-muted-foreground text-xs">{$_("header.tagline")}</p>
        </div>
      </a>
    </div>
    <div class="flex items-center gap-4">
      <nav class="flex items-center gap-1">
        {#each navItems as item}
          {@const isActive =
            item.href === "/" ? page.url.pathname === "/" : page.url.pathname.startsWith(item.href)}
          {@const Icon = item.icon}
          <a
            href={item.href}
            data-testid="nav-{item.id}"
            class="focus-visible:ring-ring inline-flex items-center justify-center rounded-md px-3 py-1.5 text-sm font-medium whitespace-nowrap transition-colors focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50 {isActive
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
