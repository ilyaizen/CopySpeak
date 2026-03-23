<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { openExternal } from "$lib/utils/external-link";
  import { VERSION } from "$lib/version";
  import { Github } from "@lucide/svelte";
  import { _ } from "svelte-i18n";
</script>

<div class="space-y-4">
  <!-- Version -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label>{$_("settings.about.version")}</Label>
      <InfoTooltip text={$_("settings.about.versionDescription")} />
    </div>
    <span class="text-muted-foreground font-mono text-sm">v{VERSION}</span>
  </div>

  <!-- Source Code -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label>{$_("settings.about.sourceCode")}</Label>
      <InfoTooltip text={$_("settings.about.sourceCodeDescription")} />
    </div>
    <Button
      variant="outline"
      size="sm"
      onclick={() => openExternal("https://github.com/ilyaizen/CopySpeak")}
    >
      <Github class="mr-2 h-4 w-4" />
      {$_("settings.about.github")}
    </Button>
  </div>

  <!-- Acknowledgments Section -->
  <div class="border-border mt-6 border-t pt-4">
    <h3 class="mb-3 text-sm font-semibold">{$_("settings.about.acknowledgments")}</h3>
    <div class="space-y-3">
      <!-- OpenAI -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.openai.title")}</Label>
          <InfoTooltip text={$_("settings.about.openai.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.openai.detail")}
        </p>
      </div>

      <!-- ElevenLabs -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.elevenlabs.title")}</Label>
          <InfoTooltip text={$_("settings.about.elevenlabs.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.elevenlabs.detail")}
        </p>
      </div>

      <!-- Kokoro -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.kokoro.title")}</Label>
          <InfoTooltip text={$_("settings.about.kokoro.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.kokoro.detail")}
        </p>
      </div>

      <!-- Piper -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.piper.title")}</Label>
          <InfoTooltip text={$_("settings.about.piper.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.piper.detail")}
        </p>
      </div>
    </div>
  </div>
</div>
