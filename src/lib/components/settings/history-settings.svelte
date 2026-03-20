<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogTrigger
  } from "$lib/components/ui/alert-dialog/index.js";
  import type { AutoDeleteMode } from "$lib/types";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable(), onRunCleanup } = $props();

  // Type guards for AutoDeleteMode
  function isKeepLatest(mode: AutoDeleteMode): mode is { keep_latest: number } {
    return typeof mode === "object" && "keep_latest" in mode;
  }

  function isAfterDays(mode: AutoDeleteMode): mode is { after_days: number } {
    return typeof mode === "object" && "after_days" in mode;
  }
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label for="history-enabled">{$_("settings.history.enabled")}</Label>
      <InfoTooltip text={$_("settings.history.enabledDescription")} />
    </div>
    <Switch id="history-enabled" bind:checked={localConfig.history.enabled} />
  </div>

  {#if localConfig.history.enabled}
    {#if localConfig.history.storage_mode === "persistent"}
      <div class="space-y-2">
        <div class="flex items-center gap-1.5">
          <Label for="persistent-dir">{$_("settings.history.generationsFolder")}</Label>
          <InfoTooltip text={$_("settings.history.generationsFolderDescription")} />
        </div>
        <div class="flex gap-2">
          <input
            id="persistent-dir"
            type="text"
            class="border-input placeholder:text-muted-foreground focus-visible:ring-ring flex h-9 w-full rounded-md border bg-transparent px-3 py-1 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium focus-visible:ring-1 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
            placeholder={$_("settings.history.generationsFolderPlaceholder")}
            bind:value={localConfig.history.persistent_dir}
          />
        </div>
      </div>
    {/if}

    <hr class="border-border" />

    <h4 class="text-card-foreground text-sm font-medium">
      {$_("settings.history.autoDeleteTitle")}
    </h4>

    <div class="space-y-4">
      <label class="flex items-center space-x-2">
        <input
          type="radio"
          name="auto_delete"
          checked={isKeepLatest(localConfig.history.auto_delete)}
          onchange={() => {
            localConfig.history.auto_delete = { keep_latest: 15 };
          }}
        />
        <span class="text-sm"
          >{$_("settings.history.autoDeleteKeepLatest")}
          {isKeepLatest(localConfig.history.auto_delete)
            ? localConfig.history.auto_delete.keep_latest
            : 15}</span
        >
      </label>
      {#if isKeepLatest(localConfig.history.auto_delete)}
        <Slider
          min={1}
          max={1000}
          step={1}
          value={localConfig.history.auto_delete.keep_latest}
          onchange={(val: number) => {
            localConfig.history.auto_delete = {
              keep_latest: val
            };
          }}
        />
      {/if}

      <label class="flex items-center space-x-2">
        <input
          type="radio"
          name="auto_delete"
          checked={localConfig.history.auto_delete === "never"}
          onchange={() => {
            localConfig.history.auto_delete = "never";
          }}
        />
        <span class="text-sm">{$_("settings.history.autoDeleteNever")}</span>
      </label>

      <label class="flex items-center space-x-2">
        <input
          type="radio"
          name="auto_delete"
          checked={isAfterDays(localConfig.history.auto_delete)}
          onchange={() => {
            localConfig.history.auto_delete = { after_days: 30 };
          }}
        />
        <span class="text-sm"
          >{$_("settings.history.autoDeleteAfterDays")}
          {isAfterDays(localConfig.history.auto_delete)
            ? localConfig.history.auto_delete.after_days
            : 30}
          {$_("settings.history.days")}</span
        >
      </label>
      {#if isAfterDays(localConfig.history.auto_delete)}
        <Slider
          min={1}
          max={365}
          step={1}
          value={localConfig.history.auto_delete.after_days}
          onchange={(val: number) => {
            localConfig.history.auto_delete = {
              after_days: val
            };
          }}
        />
      {/if}
    </div>

    <hr class="border-border" />

    <div class="space-y-2">
      <h4 class="text-card-foreground text-sm font-medium">
        {$_("settings.history.manualCleanupTitle")}
      </h4>
      <p class="text-muted-foreground text-xs">
        {$_("settings.history.manualCleanupDescription")}
      </p>
      <AlertDialog>
        <AlertDialogTrigger>
          <Button variant="outline" size="sm">{$_("settings.history.runCleanup")}</Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{$_("settings.history.cleanupDialog.title")}</AlertDialogTitle>
            <AlertDialogDescription>
              {$_("settings.history.cleanupDialog.description")}
              <ul class="mt-2 list-inside list-disc space-y-1">
                {#if localConfig.history.cleanup_orphaned_files}
                  <li>{$_("settings.history.cleanupDialog.orphanedFiles")}</li>
                {/if}
              </ul>
              {$_("settings.history.cleanupDialog.cannotUndo")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{$_("settings.history.cleanupDialog.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onclick={() => {
                if (onRunCleanup) {
                  onRunCleanup();
                }
              }}
              class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {$_("settings.history.cleanupDialog.confirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  {/if}
</div>
