<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Button } from "$lib/components/ui/button/index.js";

  let {
    batchTexts = $bindable(),
    isBatchProcessing,
    batchOutputDirectory = $bindable(),
    batchFilenamePrefix = $bindable(),
    audioFormatOptions,
    batchFormat = $bindable(),
    batchProgress,
    batchResults,
    cancelBatch,
    clearBatch,
    startBatch
  } = $props();
</script>

<div class="border-border bg-card rounded-lg border p-4 shadow-sm">
  <h3 class="text-card-foreground mb-4 text-lg font-medium">Batch TTS Processor</h3>
  <p class="text-muted-foreground mb-4 text-sm">
    Generate audio for multiple text snippets at once. Each line becomes a separate audio file.
  </p>

  <div class="space-y-4">
    <!-- Text input -->
    <div class="space-y-2">
      <Label for="batch-texts">Text Snippets (one per line)</Label>
      <textarea
        id="batch-texts"
        rows={6}
        class="border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring flex min-h-20 w-full rounded-md border px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
        placeholder="Enter text snippets, one per line..."
        bind:value={batchTexts}
        disabled={isBatchProcessing}
      ></textarea>
    </div>

    <!-- Output settings -->
    <div class="grid grid-cols-2 gap-4">
      <div class="space-y-2">
        <Label for="batch-directory">Output Directory</Label>
        <Input
          id="batch-directory"
          type="text"
          placeholder="C:\Users\You\Documents\TTS"
          bind:value={batchOutputDirectory}
          disabled={isBatchProcessing}
        />
      </div>

      <div class="space-y-2">
        <Label for="batch-prefix">Filename Prefix</Label>
        <Input
          id="batch-prefix"
          type="text"
          placeholder="batch"
          bind:value={batchFilenamePrefix}
          disabled={isBatchProcessing}
        />
      </div>
    </div>

    <div class="space-y-2">
      <Label for="batch-format">Audio Format</Label>
      <Select
        id="batch-format"
        options={audioFormatOptions}
        bind:value={batchFormat}
        disabled={isBatchProcessing}
      />
      <p class="text-muted-foreground text-xs">
        Files will be saved as: {batchFilenamePrefix || "batch"}_0001.{batchFormat}, {batchFilenamePrefix ||
          "batch"}_0002.{batchFormat}, etc.
      </p>
    </div>

    <!-- Progress indicator -->
    {#if batchProgress}
      <div class="bg-muted/50 space-y-2 rounded-lg p-3">
        <div class="flex justify-between text-sm">
          <span>
            {#if batchProgress.status === "running"}
              Processing...
            {:else if batchProgress.status === "completed"}
              Completed
            {:else if batchProgress.status === "cancelled"}
              Cancelled
            {/if}
          </span>
          <span>{batchProgress.current} / {batchProgress.total}</span>
        </div>
        <div class="bg-muted h-2 w-full overflow-hidden rounded-full">
          <div
            class="bg-primary h-full transition-all duration-300"
            style="width: {batchProgress.total > 0
              ? (batchProgress.current / batchProgress.total) * 100
              : 0}%"
          ></div>
        </div>
        {#if batchProgress.current_text}
          <p class="text-muted-foreground truncate text-xs">
            Current: {batchProgress.current_text}
          </p>
        {/if}
      </div>
    {/if}

    <!-- Results -->
    {#if batchResults.length > 0}
      <div class="space-y-2">
        <h4 class="text-sm font-medium">Results</h4>
        <div class="max-h-48 space-y-1 overflow-y-auto">
          {#each batchResults as result}
            <div
              class="flex items-center gap-2 rounded p-2 text-sm {result.success
                ? 'bg-green-500/10'
                : 'bg-red-500/10'}"
            >
              <span class={result.success ? "text-green-500" : "text-red-500"}>
                {result.success ? "✓" : "✗"}
              </span>
              <span class="text-muted-foreground">#{result.index + 1}</span>
              <span class="flex-1 truncate">{result.text}</span>
              {#if result.output_path}
                <span class="text-muted-foreground max-w-50 truncate text-xs"
                  >{result.output_path}</span
                >
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Actions -->
    <div class="flex justify-end gap-2">
      {#if isBatchProcessing}
        <Button variant="destructive" onclick={cancelBatch}>Cancel</Button>
      {:else}
        <Button
          variant="outline"
          onclick={clearBatch}
          disabled={!batchTexts && batchResults.length === 0}
        >
          Clear
        </Button>
        <Button
          onclick={startBatch}
          disabled={!batchTexts?.trim() || !batchOutputDirectory?.trim()}
        >
          Generate Audio
        </Button>
      {/if}
    </div>
  </div>
</div>
