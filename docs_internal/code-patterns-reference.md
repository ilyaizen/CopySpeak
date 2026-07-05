# Code Patterns Reference

> Copy-paste reference for Svelte 5, Rust, and Tauri IPC patterns used in CopySpeak.
> Trimmed from AGENTS.md to keep agent instructions compact.

## Imports

- Frontend: `$lib/` local imports
- Backend: `crate::module` relative imports
- Group order: external → internal aliases → relative
- **Lucide Icons**: Always use `@lucide/svelte` (NOT `lucide-svelte`)
  - Correct: `import { Upload } from "@lucide/svelte";`
  - Wrong: `import { Upload } from "lucide-svelte";`

## Error Handling

- Rust: `Result<T, String>` for IPC, `?` with `map_err`
- TypeScript: Discriminated unions `{ ok: true; value: T } | { ok: false; error: string }`

## Svelte 5 Component Skeleton

```svelte
<script lang="ts">
  import { page } from "$app/state";

  let count = $state(0);
  let doubled = $derived(count * 2);
  let { title, onClick } = $props<{ title: string; onClick: () => void }>();

  $effect(() => {
    console.log("Count changed:", count);
  });

  function handleClick() {
    count++;
  }
</script>

<button onclick={handleClick}>{title}</button><p>Doubled: {doubled()}</p>
```

## Slider Bindings with Optional Config Values

For optional config sliders (e.g., `voice_style?: number`), use local `$state` + `$effect` for cancel support, `onchange` for user changes:

```ts
let styleValue = $state(localConfig.tts.elevenlabs.voice_style ?? 0);

// Sync FROM config when parent cancels/resets localConfig
$effect(() => {
  const cfg = localConfig;
  const configValue = cfg.tts.elevenlabs.voice_style ?? 0;
  if (styleValue !== configValue) {
    styleValue = configValue;
  }
});
```

```svelte
<!-- Sync TO config via onchange (NOT $effect - avoids race condition with cancel) -->
<Slider
  bind:value={styleValue}
  onchange={(v) => {
    localConfig.tts.elevenlabs.voice_style = v;
  }}
/>
```

**Why this pattern?** `$effect` sync TO config can race: parent cancel replaces `localConfig`, effect runs old `styleValue`, overwrites reset. `onchange` syncs only explicit user interaction.

## Rust Tauri Command

```rust
#[tauri::command]
pub async fn speak_now(
    config: State<'_, Mutex<AppConfig>>,
) -> Result<(), String> {
    let cfg = config.lock().map_err(|e| e.to_string())?;
    Ok(())
}
```

## Tauri IPC Pattern

1. Define in `commands.rs`:

```rust
#[tauri::command]
pub async fn my_command(state: State<'_, Mutex<MyState>>) -> Result<T, String> {
    // implementation
}
```

2. Register in `main.rs`:

```rust
.invoke_handler(tauri::generate_handler![commands::my_command])
```

3. Call from frontend:

```typescript
import { invoke } from "@tauri-apps/api/core";
await invoke("my_command");
```
