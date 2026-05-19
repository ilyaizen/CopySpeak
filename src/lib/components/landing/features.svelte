<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Cpu, Headphones, History, Settings, Wand2, Zap } from "@lucide/svelte";

  const features = $derived([
    {
      icon: Zap,
      title: $_("landing.features.doubleCopy.title"),
      description: $_("landing.features.doubleCopy.description"),
      badge: null
    },
    {
      icon: Cpu,
      title: $_("landing.features.engines.title"),
      description: $_("landing.features.engines.description"),
      badge: null
    },
    {
      icon: Headphones,
      title: $_("landing.features.hud.title"),
      description: $_("landing.features.hud.description"),
      badge: null
    },
    {
      icon: History,
      title: $_("landing.features.history.title"),
      description: $_("landing.features.history.description"),
      badge: null
    },
    {
      icon: Wand2,
      title: $_("landing.features.sanitization.title"),
      description: $_("landing.features.sanitization.description"),
      badge: null
    },
    {
      icon: Settings,
      title: $_("landing.features.configurable.title"),
      description: $_("landing.features.configurable.description"),
      badge: null
    }
  ]);
</script>

<section class="border-border bg-muted/30 border-b px-6 py-16 sm:py-24">
  <div class="mx-auto max-w-5xl">
    <h2 class="text-foreground text-center font-mono text-2xl font-bold sm:text-3xl">
      {$_("landing.features.title")}
    </h2>

    <div class="mt-12 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
      {#each features as feature}
        {@const Icon = feature.icon}
        <div
          class="border-border bg-background hover:border-primary/50 border p-6 transition-colors"
        >
          <div class="mb-4 flex items-center gap-3">
            <div class="bg-primary/10 text-primary flex h-10 w-10 items-center justify-center">
              <Icon class="h-5 w-5" />
            </div>
            {#if feature.badge}
              <Badge variant="secondary">{feature.badge}</Badge>
            {/if}
          </div>

          <h3 class="text-foreground font-mono text-lg font-semibold">
            {feature.title}
          </h3>

          <p class="text-muted-foreground mt-2 text-sm">
            {feature.description}
          </p>
        </div>
      {/each}
    </div>
  </div>
</section>
