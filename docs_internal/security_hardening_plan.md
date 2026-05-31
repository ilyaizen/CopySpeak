# Security Hardening Implementation Plan

**Status**: Critical Priority - Block Public Release
**Timeline**: 2-3 weeks focused work
**Owner**: Core development team

This document provides detailed implementation steps for addressing critical security vulnerabilities identified in the security audit.

---

## Overview

CopySpeak currently has **5 critical security issues** that must be resolved before public release:

1. Command injection vulnerability in CLI TTS backend
2. Plaintext API key storage
3. Insecure temp file handling
4. Missing user consent for clipboard monitoring
5. No rate limiting for TTS requests

Each section below provides:

- Problem description and attack vectors
- Technical implementation approach
- Code changes required with file locations
- Testing criteria
- Timeline estimate

---

## 1. Encrypt API Keys Using Windows DPAPI

**Priority**: 🔴 CRITICAL
**Estimated Time**: 3-4 days
**Files Affected**: `src-tauri/src/config.rs`, `src-tauri/Cargo.toml`

### Problem

API keys for OpenAI and ElevenLabs are stored as plaintext strings in `%APPDATA%/CopySpeak/config.json`:

```json
{
  "openai": {
    "api_key": "sk-proj-abc123..."
  }
}
```

**Attack Vector**: Any malware with user-level permissions can harvest API keys.

### Solution: Windows DPAPI Encryption

Use Windows Data Protection API (DPAPI) to encrypt sensitive fields before saving to disk.

#### Implementation Steps

**Step 1: Add DPAPI dependency**

```toml
# src-tauri/Cargo.toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = [
    "Win32_Security_Cryptography",
    "Win32_Foundation"
]}
```

**Step 2: Create encryption module**

Create new file: `src-tauri/src/crypto.rs`

```rust
#[cfg(windows)]
use windows::{
    core::PSTR,
    Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTOAPI_BLOB, CRYPT_PROTECT_UI_FORBIDDEN,
    },
};

/// Encrypt data using Windows DPAPI (user-scoped)
#[cfg(windows)]
pub fn encrypt_string(plaintext: &str) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::LocalFree;

    let plaintext_bytes = plaintext.as_bytes();
    let mut input = CRYPTOAPI_BLOB {
        cbData: plaintext_bytes.len() as u32,
        pbData: plaintext_bytes.as_ptr() as *mut u8,
    };

    let mut output = CRYPTOAPI_BLOB::default();

    unsafe {
        CryptProtectData(
            &mut input,
            None, // No description
            None, // No additional entropy
            None, // Reserved
            None, // No prompt struct
            CRYPT_PROTECT_UI_FORBIDDEN, // No UI prompts
            &mut output,
        )
        .map_err(|e| format!("DPAPI encryption failed: {:?}", e))?;

        let encrypted = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(Some(output.pbData as isize));
        Ok(encrypted)
    }
}

/// Decrypt data using Windows DPAPI
#[cfg(windows)]
pub fn decrypt_string(ciphertext: &[u8]) -> Result<String, String> {
    use windows::Win32::Foundation::LocalFree;

    let mut input = CRYPTOAPI_BLOB {
        cbData: ciphertext.len() as u32,
        pbData: ciphertext.as_ptr() as *mut u8,
    };

    let mut output = CRYPTOAPI_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &mut input,
            None,
            None,
            None,
            None,
            CRYPT_PROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| format!("DPAPI decryption failed: {:?}", e))?;

        let decrypted_bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let decrypted_string = String::from_utf8(decrypted_bytes.to_vec())
            .map_err(|e| format!("Invalid UTF-8 after decryption: {}", e))?;

        LocalFree(Some(output.pbData as isize));
        Ok(decrypted_string)
    }
}

#[cfg(not(windows))]
pub fn encrypt_string(_plaintext: &str) -> Result<Vec<u8>, String> {
    Err("DPAPI encryption only available on Windows".to_string())
}

#[cfg(not(windows))]
pub fn decrypt_string(_ciphertext: &[u8]) -> Result<String, String> {
    Err("DPAPI decryption only available on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_encrypt_decrypt_roundtrip() {
        let secret = "sk-proj-test123456789";
        let encrypted = encrypt_string(secret).unwrap();
        let decrypted = decrypt_string(&encrypted).unwrap();
        assert_eq!(secret, decrypted);
    }
}
```

**Step 3: Create SecureString type**

Add to `src-tauri/src/config.rs`:

```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// API key stored as encrypted bytes (base64-encoded in JSON)
#[derive(Clone)]
pub struct SecureString {
    encrypted: Vec<u8>,
}

impl SecureString {
    /// Create from plaintext (encrypts immediately)
    pub fn new(plaintext: &str) -> Result<Self, String> {
        let encrypted = crate::crypto::encrypt_string(plaintext)?;
        Ok(Self { encrypted })
    }

    /// Decrypt and return plaintext
    pub fn decrypt(&self) -> Result<String, String> {
        crate::crypto::decrypt_string(&self.encrypted)
    }

    /// Create from already-encrypted bytes (for deserialization)
    pub fn from_encrypted(encrypted: Vec<u8>) -> Self {
        Self { encrypted }
    }
}

impl Serialize for SecureString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Store as base64-encoded string in JSON
        let base64_str = BASE64.encode(&self.encrypted);
        serializer.serialize_str(&base64_str)
    }
}

impl<'de> Deserialize<'de> for SecureString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let base64_str = String::deserialize(deserializer)?;
        let encrypted = BASE64.decode(&base64_str)
            .map_err(serde::de::Error::custom)?;
        Ok(SecureString::from_encrypted(encrypted))
    }
}
```

**Step 4: Update config structs**

In `src-tauri/src/config.rs`, replace `String` with `SecureString` for API keys:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiSettings {
    pub enabled: bool,
    pub api_key: SecureString,  // Changed from String
    pub model: String,
    pub voice: String,
    pub speed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevenLabsSettings {
    pub enabled: bool,
    pub api_key: SecureString,  // Changed from String
    pub voice_id: String,
    pub model_id: String,
    pub stability: f32,
    pub similarity_boost: f32,
}
```

**Step 5: Update IPC commands**

In `src-tauri/src/commands.rs`, modify `set_config` to handle plaintext input:

```rust
#[tauri::command]
pub async fn set_openai_api_key(
    key: String,
    config: State<'_, Mutex<AppConfig>>,
) -> Result<(), String> {
    let secure_key = SecureString::new(&key)?;
    let mut cfg = config.lock().unwrap();
    cfg.openai.api_key = secure_key;
    config::save(&cfg)?;
    Ok(())
}
```

**Step 6: Config migration**

Add migration for existing plaintext configs:

```rust
impl AppConfig {
    pub fn migrate_plaintext_keys(&mut self) -> Result<(), String> {
        // Check if keys are already encrypted by attempting decryption
        // If decryption fails, assume plaintext and encrypt

        // This is handled automatically by the SecureString deserialization
        // Old configs with plaintext will fail to deserialize
        // We need a custom migration loader
        Ok(())
    }
}

pub fn load_with_migration() -> Result<AppConfig, String> {
    let path = get_config_path();

    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read config: {}", e))?;

    // Try normal deserialization first
    match serde_json::from_str::<AppConfig>(&content) {
        Ok(config) => Ok(config),
        Err(_) => {
            // Might be old format with plaintext keys
            // Try deserializing as OldConfig, then convert
            migrate_from_plaintext(&content)
        }
    }
}

fn migrate_from_plaintext(json: &str) -> Result<AppConfig, String> {
    // Define old config struct with String keys
    #[derive(Deserialize)]
    struct OldConfig {
        openai: OldOpenAiSettings,
        elevenlabs: OldElevenLabsSettings,
        // ... other fields
    }

    #[derive(Deserialize)]
    struct OldOpenAiSettings {
        api_key: String,
        // ... other fields
    }

    let old: OldConfig = serde_json::from_str(json)
        .map_err(|e| format!("Config migration failed: {}", e))?;

    // Convert to new format with encrypted keys
    let mut new_config = AppConfig::default();
    new_config.openai.api_key = SecureString::new(&old.openai.api_key)?;
    // ... convert other fields

    // Backup old config
    let backup_path = get_config_path().with_extension("json.backup");
    std::fs::copy(get_config_path(), backup_path)
        .map_err(|e| format!("Backup failed: {}", e))?;

    // Save new encrypted config
    save(&new_config)?;

    log::info!("Migrated config from plaintext to encrypted keys");
    Ok(new_config)
}
```

#### Testing Criteria

- [ ] New installations encrypt keys on first save
- [ ] Existing configs migrate automatically on next load
- [ ] Keys decrypt correctly when needed for API calls
- [ ] Config file shows base64-encoded data, not plaintext
- [ ] Backup created before migration
- [ ] Unit tests pass for encrypt/decrypt roundtrip

#### Security Validation

```bash
# After implementation, verify:
# 1. Check config file - should NOT contain plaintext keys
type %APPDATA%\CopySpeak\config.json | findstr "sk-proj"
# Should return nothing

# 2. Verify encrypted format
type %APPDATA%\CopySpeak\config.json
# Should see base64 strings like "YWJjMTIzNDU2Nzg5..."

# 3. Test migration
# Manually edit config to add plaintext key, restart app, verify it migrates
```

---

## 2. Implement Secure Temp File Handling

**Priority**: 🔴 CRITICAL
**Estimated Time**: 2 days
**Files Affected**: `src-tauri/src/tts/cli.rs`

### Problem

Fixed temp file paths are predictable and vulnerable to:

- Symlink attacks (attacker pre-creates malicious symlinks)
- Race conditions (multiple instances overwrite each other)
- Data leakage (temp files persist after app exit)
- No secure deletion (content recoverable from disk)

Current code:

```rust
let input_path = tmp.join("copyspeak_tts_input.txt");
let output_path = tmp.join("copyspeak_tts_out.wav");
```

### Solution: Random Temp Files with Secure Deletion

#### Implementation Steps

**Step 1: Add tempfile dependency**

```toml
# src-tauri/Cargo.toml
[dependencies]
tempfile = "3.10"
```

**Step 2: Replace fixed paths with NamedTempFile**

In `src-tauri/src/tts/cli.rs`:

```rust
use tempfile::NamedTempFile;

impl CliBackend {
    pub fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, TtsError> {
        // Create random temp files (automatically deleted on drop)
        let mut input_file = NamedTempFile::new()
            .map_err(|e| TtsError::IoError(format!("Failed to create temp input: {}", e)))?;
        let output_file = NamedTempFile::new()
            .map_err(|e| TtsError::IoError(format!("Failed to create temp output: {}", e)))?;

        // Write input text
        input_file.write_all(text.as_bytes())
            .map_err(|e| TtsError::IoError(format!("Failed to write input: {}", e)))?;
        input_file.flush()
            .map_err(|e| TtsError::IoError(format!("Failed to flush input: {}", e)))?;

        // Get paths as strings
        let input_path = input_file.path().to_string_lossy().to_string();
        let output_path = output_file.path().to_string_lossy().to_string();

        // Build command args with template substitution
        let args: Vec<String> = self.args_template
            .iter()
            .map(|arg| {
                arg.replace("{input}", &input_path)
                   .replace("{text}", &input_path)
                   .replace("{output}", &output_path)
                   .replace("{voice}", voice)
                   .replace("{speed}", &speed.to_string())
            })
            .collect();

        // Execute TTS command
        let output = Command::new(&self.command)
            .args(&args)
            .output()
            .map_err(|e| TtsError::CommandFailed(format!("Failed to execute: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TtsError::CommandFailed(format!("TTS failed: {}", stderr)));
        }

        // Read output audio
        let audio_bytes = std::fs::read(output_file.path())
            .map_err(|e| TtsError::OutputNotFound(format!("Failed to read output: {}", e)))?;

        // Explicitly zero out input file before deletion
        secure_delete_file(input_file)?;

        // output_file auto-deleted on drop
        Ok(audio_bytes)
    }
}

/// Securely delete temp file by overwriting with zeros
fn secure_delete_file(mut file: NamedTempFile) -> Result<(), TtsError> {
    use std::io::{Seek, SeekFrom, Write};

    // Get file size
    let size = file.seek(SeekFrom::End(0))
        .map_err(|e| TtsError::IoError(format!("Seek failed: {}", e)))?;

    // Overwrite with zeros
    file.seek(SeekFrom::Start(0))
        .map_err(|e| TtsError::IoError(format!("Seek failed: {}", e)))?;
    let zeros = vec![0u8; size as usize];
    file.write_all(&zeros)
        .map_err(|e| TtsError::IoError(format!("Overwrite failed: {}", e)))?;
    file.flush()
        .map_err(|e| TtsError::IoError(format!("Flush failed: {}", e)))?;

    // File is deleted when dropped
    Ok(())
}
```

**Step 3: Handle streaming mode**

For streaming synthesis (stdout-based):

```rust
pub fn synthesize_streaming(&self, text: &str, voice: &str, speed: f32) -> Result<StreamingSynthesis, TtsError> {
    // Still need temp input file
    let mut input_file = NamedTempFile::new()
        .map_err(|e| TtsError::IoError(format!("Failed to create temp input: {}", e)))?;

    input_file.write_all(text.as_bytes())?;
    input_file.flush()?;

    let input_path = input_file.path().to_string_lossy().to_string();

    // Build args (no output file for streaming)
    let args: Vec<String> = self.args_template
        .iter()
        .map(|arg| {
            arg.replace("{input}", &input_path)
               .replace("{text}", &input_path)
               .replace("{output}", "-")  // stdout
               .replace("{voice}", voice)
               .replace("{speed}", &speed.to_string())
        })
        .collect();

    // Spawn process
    let child = Command::new(&self.command)
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| TtsError::CommandFailed(format!("Failed to spawn: {}", e)))?;

    Ok(StreamingSynthesis {
        process: child,
        temp_input: Some(input_file),  // Keep alive until process exits
    })
}

pub struct StreamingSynthesis {
    process: Child,
    temp_input: Option<NamedTempFile>,  // Auto-deleted when dropped
}
```

#### Testing Criteria

- [ ] Temp files have random names (not predictable)
- [ ] Temp files are deleted after synthesis
- [ ] Input files are zeroed before deletion
- [ ] Multiple simultaneous syntheses don't conflict
- [ ] No temp file leaks on app crash (OS cleanup after reboot)

#### Security Validation

```bash
# Monitor temp directory during synthesis
cd %TEMP%
dir /o:d

# Should see random names like:
# copyspeak_ABC123XYZ.txt
# copyspeak_DEF456UVW.wav

# After synthesis completes, files should be gone
```

---

## 3. Add User Consent Dialog for Clipboard Monitoring

**Priority**: 🔴 CRITICAL
**Estimated Time**: 3 days
**Files Affected**: `src/lib/components/ConsentDialog.svelte`, `src-tauri/src/config.rs`, `src-tauri/src/main.rs`

### Problem

App starts clipboard monitoring immediately without:

- User consent or explanation
- Privacy disclosure
- Revocable permission

**Privacy Violation**: Continuously reading clipboard = keylogger-like behavior.

### Solution: Explicit Consent UI on First Run

#### Implementation Steps

**Step 1: Add consent flag to config**

In `src-tauri/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // ... existing fields

    #[serde(default)]
    pub privacy: PrivacySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    /// User has explicitly consented to clipboard monitoring
    pub clipboard_consent_granted: bool,

    /// Timestamp when consent was granted
    pub clipboard_consent_timestamp: Option<String>,

    /// User has completed first-run setup
    pub first_run_complete: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            clipboard_consent_granted: false,
            clipboard_consent_timestamp: None,
            first_run_complete: false,
        }
    }
}
```

**Step 2: Gate clipboard monitoring on consent**

In `src-tauri/src/main.rs`:

```rust
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let config_state = app.state::<Mutex<AppConfig>>();
            let config = config_state.lock().unwrap();

            // Only start clipboard monitor if consent granted
            if config.privacy.clipboard_consent_granted {
                start_clipboard_monitor(app.app_handle())?;
            } else {
                log::info!("Clipboard monitoring disabled - awaiting user consent");
            }

            Ok(())
        })
        // ... rest of setup
}
```

**Step 3: Create consent dialog component**

Create `src/lib/components/ConsentDialog.svelte`:

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { Button } from "$lib/components/ui/button";

  let visible = $state(true);
  let consentGranted = $state(false);

  async function grantConsent() {
    consentGranted = true;
    await invoke("grant_clipboard_consent");
    visible = false;

    // Emit event to start clipboard monitoring
    window.dispatchEvent(new CustomEvent("clipboard-consent-granted"));
  }

  function denyConsent() {
    visible = false;
    // User can still use manual trigger (Speak Now button)
  }
</script>

{#if visible}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/80">
    <div class="bg-background border-foreground max-w-2xl border-2 p-8">
      <h2 class="mb-4 text-2xl font-bold">Privacy Notice: Clipboard Access</h2>

      <div class="mb-6 space-y-4 text-sm">
        <p>
          <strong>CopySpeak needs permission to monitor your clipboard</strong> to enable the double-copy
          trigger feature.
        </p>

        <div class="border-warning border-l-4 pl-4">
          <p class="font-semibold">What data is accessed:</p>
          <ul class="mt-2 ml-5 list-disc">
            <li>Text content copied to clipboard (system-wide)</li>
            <li>Timestamps of clipboard changes</li>
            <li>Application name that triggered clipboard change</li>
          </ul>
        </div>

        <div class="border-success border-l-4 pl-4">
          <p class="font-semibold">How your data is protected:</p>
          <ul class="mt-2 ml-5 list-disc">
            <li>
              <strong>Local only</strong>: Data never leaves your computer (unless you enable cloud
              TTS)
            </li>
            <li>
              <strong>Filtered</strong>: Sensitive patterns (passwords, credit cards) are blocked
            </li>
            <li><strong>App blacklist</strong>: Ignore clipboard from password managers</li>
            <li><strong>Temporary</strong>: Text is processed and discarded immediately</li>
            <li>
              <strong>Revocable</strong>: You can disable monitoring anytime in Settings → Privacy
            </li>
          </ul>
        </div>

        <div class="border-muted border-l-4 pl-4">
          <p class="font-semibold">If you decline:</p>
          <ul class="mt-2 ml-5 list-disc">
            <li>You can still use CopySpeak with the manual "Speak Now" button</li>
            <li>Hotkeys will continue to work</li>
            <li>You can grant permission later in Settings</li>
          </ul>
        </div>

        <p class="text-muted-foreground text-xs">
          <strong>Technical details</strong>: CopySpeak uses Windows' AddClipboardFormatListener API
          to detect clipboard changes. This is event-driven (not polling) and has minimal
          performance impact. Your clipboard history is NOT stored or logged unless you enable the
          History feature.
        </p>
      </div>

      <div class="flex justify-end gap-4">
        <Button variant="outline" onclick={denyConsent}>Decline (Manual Mode Only)</Button>
        <Button onclick={grantConsent}>I Understand - Grant Permission</Button>
      </div>

      {#if consentGranted}
        <p class="text-success mt-4 text-sm">
          ✓ Clipboard monitoring enabled. You can revoke this anytime in Settings → Privacy.
        </p>
      {/if}
    </div>
  </div>
{/if}
```

**Step 4: Add IPC command for consent**

In `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub fn grant_clipboard_consent(
    app: tauri::AppHandle,
    config: State<'_, Mutex<AppConfig>>,
) -> Result<(), String> {
    let mut cfg = config.lock().unwrap();

    cfg.privacy.clipboard_consent_granted = true;
    cfg.privacy.clipboard_consent_timestamp = Some(
        chrono::Utc::now().to_rfc3339()
    );

    config::save(&cfg)?;

    // Start clipboard monitoring now
    crate::clipboard::start_monitoring(app)?;

    log::info!("User granted clipboard monitoring consent");
    Ok(())
}

#[tauri::command]
pub fn revoke_clipboard_consent(
    app: tauri::AppHandle,
    config: State<'_, Mutex<AppConfig>>,
) -> Result<(), String> {
    let mut cfg = config.lock().unwrap();

    cfg.privacy.clipboard_consent_granted = false;
    config::save(&cfg)?;

    // Stop clipboard monitoring
    crate::clipboard::stop_monitoring(app)?;

    log::info!("User revoked clipboard monitoring consent");
    Ok(())
}
```

**Step 5: Show dialog on first run**

In `src/routes/+page.svelte`:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import ConsentDialog from "$lib/components/ConsentDialog.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import StatusDashboard from "$lib/components/StatusDashboard.svelte";

  let showConsent = $state(false);

  onMount(async () => {
    const config = await invoke("get_config");
    if (!config.privacy.first_run_complete) {
      showConsent = true;
    }
  });
</script>

{#if showConsent}
  <ConsentDialog />
{:else}
  <div class="app-container">
    <StatusDashboard />
    <SettingsPanel />
  </div>
{/if}
```

**Step 6: Add revoke option in settings**

Add to `src/lib/components/settings/PrivacySettings.svelte`:

```svelte
<div class="setting-card">
  <h3>Clipboard Monitoring</h3>

  {#if config.privacy.clipboard_consent_granted}
    <div class="border-success rounded border p-4">
      <p class="text-success font-semibold">✓ Permission Granted</p>
      <p class="text-muted-foreground mt-1 text-xs">
        Granted on: {new Date(config.privacy.clipboard_consent_timestamp).toLocaleString()}
      </p>

      <Button variant="destructive" onclick={revokeConsent} class="mt-4">Revoke Permission</Button>
    </div>
  {:else}
    <div class="border-warning rounded border p-4">
      <p class="text-warning font-semibold">⚠ Permission Not Granted</p>
      <p class="text-muted-foreground mt-1 text-xs">
        Double-copy trigger is disabled. Using manual mode only.
      </p>

      <Button onclick={showConsentDialog} class="mt-4">Grant Permission</Button>
    </div>
  {/if}
</div>
```

#### Testing Criteria

- [ ] First-run users see consent dialog before clipboard monitoring starts
- [ ] Dialog explains what data is accessed and how it's protected
- [ ] Declining consent allows manual mode to work
- [ ] Granting consent starts clipboard monitoring immediately
- [ ] Users can revoke consent in Settings → Privacy
- [ ] Consent state persists across app restarts

---

## 4. Implement Command Allowlisting for TTS Executables

**Priority**: 🔴 CRITICAL
**Estimated Time**: 2 days
**Files Affected**: `src-tauri/src/tts/cli.rs`, `src-tauri/src/config.rs`

### Problem

Users can configure arbitrary command paths in TTS settings:

```json
{
  "command": "C:\\malware\\evil.exe",
  "args_template": ["--steal-data"]
}
```

If config file is compromised, attacker can execute arbitrary commands.

### Solution: Allowlist Known TTS Engines

#### Implementation Steps

**Step 1: Define allowlist**

In `src-tauri/src/tts/cli.rs`:

```rust
use std::path::PathBuf;

pub struct TtsCommandValidator {
    allowed_executables: Vec<AllowedExecutable>,
}

#[derive(Clone)]
pub struct AllowedExecutable {
    pub name: String,
    pub executable_name: String,
    pub expected_args: Vec<String>,
}

impl TtsCommandValidator {
    pub fn new() -> Self {
        Self {
            allowed_executables: vec![
                AllowedExecutable {
                    name: "Kokoro TTS".to_string(),
                    executable_name: "kokoro-tts.exe".to_string(),
                    expected_args: vec!["--input", "--output", "--voice", "--speed"].iter()
                        .map(|s| s.to_string()).collect(),
                },
                AllowedExecutable {
                    name: "Piper TTS".to_string(),
                    executable_name: "piper.exe".to_string(),
                    expected_args: vec!["--model", "--output_file"].iter()
                        .map(|s| s.to_string()).collect(),
                },
                AllowedExecutable {
                    name: "eSpeak NG".to_string(),
                    executable_name: "espeak-ng.exe".to_string(),
                    expected_args: vec!["-w", "-v", "-s"].iter()
                        .map(|s| s.to_string()).collect(),
                },
                AllowedExecutable {
                    name: "Festival".to_string(),
                    executable_name: "festival.exe".to_string(),
                    expected_args: vec!["--tts"].iter()
                        .map(|s| s.to_string()).collect(),
                },
            ],
        }
    }

    /// Validate that command path is an allowed TTS executable
    pub fn validate_command(&self, command_path: &str) -> Result<(), String> {
        let path = PathBuf::from(command_path);

        // Extract filename
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "Invalid command path".to_string())?;

        // Check if filename matches any allowed executable
        let is_allowed = self.allowed_executables.iter()
            .any(|allowed| allowed.executable_name.eq_ignore_ascii_case(filename));

        if !is_allowed {
            return Err(format!(
                "Executable '{}' is not in the allowed list. Allowed: {}",
                filename,
                self.allowed_executables.iter()
                    .map(|a| a.executable_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Additional check: verify file exists and is executable
        if !path.exists() {
            return Err(format!("Command not found: {}", command_path));
        }

        Ok(())
    }

    /// Validate args template for suspicious patterns
    pub fn validate_args(&self, args: &[String]) -> Result<(), String> {
        for arg in args {
            // Block dangerous patterns
            if arg.contains("&&") || arg.contains("||") || arg.contains(";") {
                return Err("Command chaining not allowed in args".to_string());
            }

            if arg.contains("$(") || arg.contains("`") {
                return Err("Command substitution not allowed in args".to_string());
            }

            // Block redirections
            if arg.contains(">") || arg.contains("<") || arg.contains("|") {
                return Err("I/O redirection not allowed in args".to_string());
            }
        }

        Ok(())
    }
}
```

**Step 2: Validate on config load**

In `src-tauri/src/config.rs`:

```rust
impl AppConfig {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // ... existing validation

        // Validate TTS command if CLI backend is enabled
        if self.tts.preset == "cli" {
            let validator = crate::tts::cli::TtsCommandValidator::new();

            if let Err(e) = validator.validate_command(&self.tts.cli.command) {
                errors.push(format!("TTS command validation failed: {}", e));
            }

            if let Err(e) = validator.validate_args(&self.tts.cli.args_template) {
                errors.push(format!("TTS args validation failed: {}", e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
```

**Step 3: Add UI for custom executables**

In `src/lib/components/settings/TtsSettings.svelte`:

```svelte
<script lang="ts">
  async function requestCustomExecutable() {
    const confirmed = await confirm(
      `You are about to add a custom TTS executable:\n\n` +
        `${customExecutablePath}\n\n` +
        `This executable will run with your user permissions and can access your files. ` +
        `Only proceed if you trust this program.\n\n` +
        `Continue?`
    );

    if (confirmed) {
      // Add to user's personal allowlist
      await invoke("add_custom_executable", { path: customExecutablePath });
    }
  }
</script>

<div class="setting-group">
  <label>TTS Executable</label>

  <Select bind:value={config.tts.cli.command}>
    <option value="kokoro-tts.exe">Kokoro TTS</option>
    <option value="piper.exe">Piper</option>
    <option value="espeak-ng.exe">eSpeak NG</option>
    <option value="festival.exe">Festival</option>
    <option value="custom">Custom (requires approval)</option>
  </Select>

  {#if config.tts.cli.command === "custom"}
    <div class="border-warning mt-2 border p-4">
      <p class="text-warning font-semibold">⚠ Custom Executable</p>
      <p class="mb-2 text-xs">
        Only add executables you trust. Malicious programs could harm your system.
      </p>

      <Input
        type="text"
        bind:value={customExecutablePath}
        placeholder="C:\path\to\custom-tts.exe"
      />

      <Button onclick={requestCustomExecutable} class="mt-2">Request Approval</Button>
    </div>
  {/if}
</div>
```

**Step 4: User-specific allowlist**

Add to config:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    /// User-approved custom executables (beyond default allowlist)
    pub custom_allowed_executables: Vec<String>,
}

impl TtsCommandValidator {
    pub fn validate_command_with_custom(&self, command_path: &str, custom_allowed: &[String]) -> Result<(), String> {
        // First check default allowlist
        if self.validate_command(command_path).is_ok() {
            return Ok(());
        }

        // Then check user's custom allowlist
        let path = PathBuf::from(command_path);
        let canonical = path.canonicalize()
            .map_err(|e| format!("Cannot resolve path: {}", e))?;

        let is_custom_allowed = custom_allowed.iter()
            .any(|allowed_path| {
                PathBuf::from(allowed_path).canonicalize()
                    .map(|p| p == canonical)
                    .unwrap_or(false)
            });

        if is_custom_allowed {
            Ok(())
        } else {
            Err("Executable not in allowed list".to_string())
        }
    }
}
```

#### Testing Criteria

- [ ] Default TTS engines (kokoro, piper, espeak) are allowed
- [ ] Arbitrary executables are blocked
- [ ] Command chaining (`&&`, `||`, `;`) is blocked in args
- [ ] Command substitution (`$()`, backticks) is blocked
- [ ] Users can add custom executables with explicit approval
- [ ] Custom allowlist persists across restarts

---

## 5. Add Rate Limiting for TTS Requests

**Priority**: 🔴 CRITICAL
**Estimated Time**: 1 day
**Files Affected**: `src-tauri/src/clipboard.rs`, `src-tauri/src/commands.rs`

### Problem

No rate limiting on TTS synthesis:

- Users can spam Ctrl+C rapidly → resource exhaustion
- Cloud API abuse → expensive billing
- Temp file spam → filesystem pollution

### Solution: Token Bucket Rate Limiter

#### Implementation Steps

**Step 1: Add rate limiter struct**

Create `src-tauri/src/rate_limit.rs`:

```rust
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    state: Mutex<RateLimitState>,
}

struct RateLimitState {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,  // tokens per second
    last_refill: Instant,
}

impl RateLimiter {
    /// Create rate limiter (max_requests per time_window)
    pub fn new(max_requests: u32, time_window: Duration) -> Self {
        let refill_rate = max_requests as f64 / time_window.as_secs_f64();

        Self {
            state: Mutex::new(RateLimitState {
                tokens: max_requests as f64,
                max_tokens: max_requests as f64,
                refill_rate,
                last_refill: Instant::now(),
            }),
        }
    }

    /// Try to consume a token. Returns Ok if allowed, Err if rate limited.
    pub fn try_acquire(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();

        // Refill tokens based on time elapsed
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.tokens = (state.tokens + elapsed * state.refill_rate).min(state.max_tokens);
        state.last_refill = now;

        // Try to consume a token
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            Ok(())
        } else {
            let wait_time = (1.0 - state.tokens) / state.refill_rate;
            Err(format!("Rate limit exceeded. Try again in {:.1}s", wait_time))
        }
    }

    /// Get current token count (for debugging)
    pub fn available_tokens(&self) -> f64 {
        let state = self.state.lock().unwrap();
        state.tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_rate_limit() {
        let limiter = RateLimiter::new(2, Duration::from_secs(1));

        // First two requests should succeed
        assert!(limiter.try_acquire().is_ok());
        assert!(limiter.try_acquire().is_ok());

        // Third should fail
        assert!(limiter.try_acquire().is_err());

        // Wait for refill
        thread::sleep(Duration::from_millis(600));

        // Should succeed after refill
        assert!(limiter.try_acquire().is_ok());
    }
}
```

**Step 2: Add to app state**

In `src-tauri/src/main.rs`:

```rust
use crate::rate_limit::RateLimiter;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // ... existing setup

            // Rate limiter: max 5 TTS requests per 10 seconds
            let tts_rate_limiter = RateLimiter::new(5, Duration::from_secs(10));
            app.manage(tts_rate_limiter);

            Ok(())
        })
        // ...
}
```

**Step 3: Apply to clipboard trigger**

In `src-tauri/src/clipboard.rs`:

```rust
fn handle_clipboard_change(app: &AppHandle, new_text: String) {
    // ... existing double-copy detection

    if should_trigger_tts {
        // Check rate limit before synthesizing
        let rate_limiter = app.state::<RateLimiter>();

        match rate_limiter.try_acquire() {
            Ok(()) => {
                trigger_tts(app, &new_text);
            }
            Err(msg) => {
                log::warn!("TTS rate limit exceeded: {}", msg);

                // Show user notification
                app.emit("rate-limit-exceeded", msg).ok();
            }
        }
    }
}
```

**Step 4: Apply to manual commands**

In `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub async fn speak_now(
    text: String,
    rate_limiter: State<'_, RateLimiter>,
    // ... other params
) -> Result<(), String> {
    // Check rate limit
    rate_limiter.try_acquire()?;

    // Proceed with synthesis
    synthesize_and_play(&text, /* ... */).await
}
```

**Step 5: Make rate limit configurable**

Add to config:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // ...

    #[serde(default)]
    pub rate_limits: RateLimitSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitSettings {
    /// Max TTS requests in time window
    #[serde(default = "default_max_requests")]
    pub max_requests: u32,

    /// Time window in seconds
    #[serde(default = "default_time_window_secs")]
    pub time_window_secs: u32,

    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_max_requests() -> u32 { 5 }
fn default_time_window_secs() -> u32 { 10 }
fn default_true() -> bool { true }
```

**Step 6: Add UI notification**

In `src/lib/components/StatusDashboard.svelte`:

```svelte
<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  let rateLimitMessage = $state("");

  onMount(() => {
    listen("rate-limit-exceeded", (event) => {
      rateLimitMessage = event.payload;

      // Clear after 5 seconds
      setTimeout(() => {
        rateLimitMessage = "";
      }, 5000);
    });
  });
</script>

{#if rateLimitMessage}
  <div class="border-warning bg-warning/10 rounded border p-3">
    <p class="text-warning text-sm">
      ⚠ {rateLimitMessage}
    </p>
  </div>
{/if}
```

#### Testing Criteria

- [ ] Rapid clipboard copying triggers rate limit
- [ ] User sees notification when rate limited
- [ ] Tokens refill over time (can retry after cooldown)
- [ ] Rate limit is configurable in settings
- [ ] Rate limit can be disabled for testing

---

## Implementation Timeline

### Week 1

- **Days 1-2**: API key encryption (DPAPI implementation)
- **Days 3-4**: Secure temp file handling
- **Day 5**: Rate limiting

### Week 2

- **Days 1-3**: Clipboard consent dialog + UI
- **Days 4-5**: Command allowlisting + validation

### Week 3

- **Days 1-2**: Integration testing + bug fixes
- **Days 3-4**: Security validation + penetration testing
- **Day 5**: Documentation + release preparation

---

## Testing Strategy

### Unit Tests

```bash
cd src-tauri
cargo test --lib
```

**Required tests:**

- `crypto::test_encrypt_decrypt_roundtrip`
- `rate_limit::test_rate_limit`
- `tts::cli::test_command_validation`
- `tts::cli::test_args_validation`

### Integration Tests

```bash
cargo test --test integration
```

**Test scenarios:**

- Config migration from plaintext to encrypted keys
- Temp file creation/deletion under load
- Rate limit enforcement across multiple threads
- Consent dialog flow (UI automation with Tauri test)

### Security Validation

- [ ] Manual penetration testing
- [ ] Code review by second developer
- [ ] Static analysis (cargo clippy, cargo audit)
- [ ] Dependency vulnerability scan

---

## Rollout Plan

### Phase 1: Internal Testing (Week 1-2)

- Implement all 5 critical fixes
- Test on development machines
- Verify no regressions

### Phase 2: Limited Beta (Week 3)

- Release to 10-20 trusted users
- Monitor for issues
- Gather feedback on consent dialog UX

### Phase 3: Public Release (Week 4+)

- Update README with security improvements
- Publish security disclosure
- Release v0.1 with hardened security

---

## Success Criteria

All items must be ✅ before public release:

- [ ] API keys encrypted with DPAPI
- [ ] Temp files use random names + secure deletion
- [ ] Clipboard consent required before monitoring
- [ ] TTS commands validated against allowlist
- [ ] Rate limiting prevents abuse
- [ ] Config migration tested (plaintext → encrypted)
- [ ] All unit tests passing
- [ ] Integration tests passing
- [ ] Security audit completed
- [ ] Documentation updated

---

## Maintenance

### Post-Release Monitoring

- Monitor GitHub issues for security reports
- Set up automated dependency scanning (Dependabot)
- Plan quarterly security reviews

### Future Enhancements

- Consider Windows Store distribution (adds sandboxing)
- Implement audit logging for forensics
- Add telemetry (opt-in) for crash reports
- Support for hardware security keys (YubiKey, etc.)

---

_Document version_: 1.0
_Last updated_: 2026-02-11
_Next review_: After implementation completion
