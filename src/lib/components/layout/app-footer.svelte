<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { cn } from "$lib/utils.js";
  import { listeningStore } from "$lib/stores/listening-store.svelte";
  import { VERSION } from "$lib/version";
  import type { AppConfig, VoiceProfile } from "$lib/types";
  import { isTauri } from "$lib/services/tauri.js";
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger
  } from "$lib/components/ui/dropdown-menu";
  import { Check } from "@lucide/svelte";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { toast } from "svelte-sonner";
  import UpdateChecker from "../update-checker.svelte";
  import { _ } from "svelte-i18n";

  let isListening = $derived(listeningStore.isListening);
  let error = $derived(listeningStore.error);

  let currentConfig = $state<AppConfig | null>(null);
  let isHudRoute = $state(false);
  let unlisten: (() => void) | null = null;
  let dropdownOpen = $state(false);

  type AvailabilityStatus = "unknown" | "checking" | "available" | "unavailable" | "error";
  let availability = $state<AvailabilityStatus>("unknown");

  const profiles = $derived(currentConfig?.tts.profiles ?? []);
  const activeProfile = $derived(
    profiles.find((profile) => profile.id === currentConfig?.tts.active_profile_id) ?? null
  );

  function profileVoiceLabel(profile: VoiceProfile | null): string | null {
    return profile?.voice_label || profile?.voice || null;
  }

  async function loadProfileInfo() {
    if (!isTauri || isHudRoute) return;

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      currentConfig = await invoke<AppConfig>("get_config");
    } catch {
      // Footer fallback is enough here.
    }
  }

  async function checkActiveProfileAvailability() {
    availability = "checking";
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<{ success: boolean }>("test_tts_engine");
      availability = result.success ? "available" : "unavailable";
    } catch (e) {
      console.error("Availability check failed:", e);
      availability = "error";
    }
  }

  async function switchProfile(profile: VoiceProfile) {
    if (profile.id === currentConfig?.tts.active_profile_id) return;

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("set_active_profile", { id: profile.id });
      await loadProfileInfo();
      await checkActiveProfileAvailability();
    } catch (e) {
      console.error("Failed to switch profile:", e);
      toast.error(`Failed to switch profile: ${e}`);
    }
  }

  onMount(async () => {
    isHudRoute = window.location.pathname.startsWith("/hud");
    if (isHudRoute || !isTauri) return;

    await loadProfileInfo();
    await checkActiveProfileAvailability();

    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen("config-changed", async () => {
        await loadProfileInfo();
        await checkActiveProfileAvailability();
      });
    } catch (e) {
      console.error("Failed to listen for config-changed:", e);
    }
  });

  onDestroy(() => {
    if (unlisten) {
      unlisten();
    }
  });
</script>

{#if !isHudRoute}
  <footer
    class="border-border bg-card/95 z-50 border-t px-4 py-1.5 shadow-[0_-2px_10px_rgba(0,0,0,0.08)] backdrop-blur-sm"
  >
    <div class="flex items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-2">
        {#if activeProfile}
          <div
            class={cn(
              "h-2.5 w-2.5 shrink-0 rounded-full",
              isListening ? "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]" : "bg-muted"
            )}
            title={isListening ? $_("footer.listening") : $_("footer.paused")}
          ></div>

          <DropdownMenu bind:open={dropdownOpen}>
            <DropdownMenuTrigger
              class="hover:bg-muted/50 focus:ring-ring cursor-pointer rounded px-1 py-0.5 text-xs transition-colors focus:ring-2 focus:ring-offset-1 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
              aria-label={`Switch profile: ${activeProfile.name}`}
            >
              <span class="text-card-foreground truncate">{activeProfile.name}</span>
            </DropdownMenuTrigger>

            <DropdownMenuContent align="start" class="min-w-56">
              {#each profiles as profile}
                {@const isSelected = profile.id === currentConfig?.tts.active_profile_id}
                <DropdownMenuItem
                  class="flex items-center justify-between gap-2"
                  onclick={() => switchProfile(profile)}
                >
                  <div class="flex min-w-0 items-center gap-2">
                    {#if isSelected && availability === "checking"}
                      <Spinner class="text-muted-foreground h-3 w-3" />
                    {:else if isSelected && availability === "available"}
                      <div class="h-2.5 w-2.5 shrink-0 rounded-full bg-green-500"></div>
                    {:else if isSelected && (availability === "unavailable" || availability === "error")}
                      <div class="h-2.5 w-2.5 shrink-0 rounded-full bg-red-500"></div>
                    {:else}
                      <div class="h-2.5 w-2.5 shrink-0 rounded-full bg-gray-400"></div>
                    {/if}

                    <span class="truncate">
                      {profile.name}
                      {#if profileVoiceLabel(profile)}
                        <span class="text-muted-foreground">({profileVoiceLabel(profile)})</span>
                      {/if}
                    </span>
                  </div>

                  {#if isSelected}
                    <Check class="text-primary h-4 w-4 shrink-0" />
                  {/if}
                </DropdownMenuItem>
              {/each}
            </DropdownMenuContent>
          </DropdownMenu>
        {:else}
          <div
            class={cn(
              "h-2.5 w-2.5 shrink-0 rounded-full",
              isListening ? "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]" : "bg-muted"
            )}
          ></div>
          <span class="text-card-foreground truncate text-xs">
            {isListening ? $_("footer.listening") : $_("footer.paused")}
          </span>
        {/if}
      </div>

      <div class="flex items-center gap-2">
        <UpdateChecker />
        <span class="bg-border mx-2 h-3 w-px"></span>
        <span class="text-muted-foreground shrink-0 text-xs">v{VERSION}</span>
      </div>
    </div>

    {#if error}
      <p class="text-destructive mt-1 text-xs">
        {error}
      </p>
    {/if}
  </footer>
{/if}
