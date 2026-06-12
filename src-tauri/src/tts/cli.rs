// CLI TTS backend.
// Spawns an external TTS command (e.g. kokoro-tts), waits for it to write a WAV file,
// reads the file, and returns the bytes. The command and args are fully configurable
// via the args_template in config, with {input}, {output}, and {voice} placeholders.
//
// Note: kokoro-tts reads from a FILE, not a command-line text argument, so we write
// the text to a temp file and pass its path via {input}.

use super::{TtsBackend, TtsError};
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// H1/A2/H2/H5: All server state and startup logic moved to piper_server module.

#[derive(serde::Serialize)]
pub struct PiperServerStatus {
    pub running: bool,
    pub model: Option<String>,
    pub port: Option<u16>,
    pub cuda: bool,
    pub ready: bool,
}

pub fn unload_piper_model_internal() -> bool {
    crate::tts::piper_server::unload_piper_model()
}

pub fn restart_piper_server(command: String, voice: String, data_dir: String, cuda: bool) {
    log::info!("[Piper] Restart requested — voice: {}, cuda: {}", voice, cuda);
    let _ = crate::tts::piper_server::unload_piper_model();
    prewarm_piper_server(command, voice, data_dir, cuda);
}

pub fn get_piper_server_status() -> PiperServerStatus {
    crate::tts::piper_server::get_piper_server_status()
}

pub fn prewarm_piper_server(command: String, voice: String, data_dir: String, cuda: bool) {
    std::thread::spawn(move || {
        let _ = crate::tts::piper_server::ensure_running(command, voice, data_dir, cuda);
    });
}

pub fn prewarm_local_server(engine: String, command: String, script_args: Vec<String>) {
    crate::tts::local_tts_server::prewarm(engine, command, script_args);
}

pub fn unload_local_server(engine: &str) -> bool {
    crate::tts::local_tts_server::unload(engine)
}

pub fn restart_local_server(engine: String, command: String, script_args: Vec<String>) {
    log::info!(
        "[LocalServer] Restart requested for {}",
        engine
    );
    let _ = crate::tts::local_tts_server::unload(&engine);
    crate::tts::local_tts_server::prewarm(engine, command, script_args);
}

#[cfg(windows)]
fn get_expanded_path() -> String {
    static EXPANDED_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    EXPANDED_PATH.get_or_init(|| {
        use std::collections::HashSet;
        use std::env;

        let current_path = env::var("PATH").unwrap_or_default();
        let mut paths: Vec<String> = current_path.split(';').map(|s| s.to_string()).collect();
        let mut seen: HashSet<String> = paths.iter().cloned().collect();

        let home = dirs::home_dir();

        let extra_paths: Vec<String> = vec![
        home.as_ref()
            .map(|h| h.join(".local").join("bin").to_string_lossy().into_owned()),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("uv")
                .join("tools")
                .join("kokoro-tts")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("uv")
                .join("tools")
                .join("piper")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Local")
                .join("bin")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("Python")
                .join("Python314")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("Python")
                .join("Python313")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("Python")
                .join("Python312")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("Python")
                .join("Python311")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Roaming")
                .join("Python")
                .join("Python310")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Local")
                .join("Python")
                .join("pythoncore-3.14-64")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Local")
                .join("Python")
                .join("pythoncore-3.13-64")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Local")
                .join("Python")
                .join("pythoncore-3.12-64")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        home.as_ref().map(|h| {
            h.join("AppData")
                .join("Local")
                .join("Python")
                .join("pythoncore-3.11-64")
                .join("Scripts")
                .to_string_lossy()
                .into_owned()
        }),
        Some(r"C:\Python314\Scripts".to_string()),
        Some(r"C:\Python313\Scripts".to_string()),
        Some(r"C:\Python312\Scripts".to_string()),
        Some(r"C:\Python311\Scripts".to_string()),
        Some(r"C:\Python310\Scripts".to_string()),
    ]
    .into_iter()
    .flatten()
    .collect();

    for p in extra_paths {
        if !seen.contains(&p) {
            seen.insert(p.clone());
            paths.push(p);
        }
    }

    paths.join(";")
    }).clone()
}

#[cfg(windows)]
fn get_nvidia_dll_paths(python_executable: &str) -> Option<String> {
    use std::sync::OnceLock;
    static NVIDIA_PATHS: OnceLock<Option<String>> = OnceLock::new();

    NVIDIA_PATHS
        .get_or_init(|| {
            let output = Command::new(python_executable)
                .args([
                    "-c",
                    "import os, nvidia; print(';'.join([os.path.join(os.path.dirname(nvidia.__file__), p, 'bin') for p in ['cublas', 'cuda_nvrtc', 'cuda_runtime', 'cudnn', 'cufft', 'curand', 'cusolver', 'cusparse', 'nvjitlink'] if os.path.exists(os.path.join(os.path.dirname(nvidia.__file__), p, 'bin'))]))"
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok()?;

            if output.status.success() {
                let paths_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !paths_str.is_empty() {
                    return Some(paths_str);
                }
            }
            None
        })
        .clone()
}

pub struct CliTtsBackend {
    pub command: String,
    pub args_template: Vec<String>,
    pub cuda: bool,
    pub preset: String,
}

impl CliTtsBackend {
    pub fn new(command: String, args_template: Vec<String>, cuda: bool, preset: String) -> Self {
        Self {
            command,
            args_template,
            cuda,
            preset,
        }
    }

    /// Check if this is kokoro-tts and model paths are missing
    fn is_kokoro_missing_models(&self) -> bool {
        let cmd_lower = self.command.to_lowercase();
        let is_kokoro = cmd_lower.contains("kokoro");
        let has_model_arg = self.args_template.iter().any(|arg| arg.contains("--model"));
        let has_voices_arg = self
            .args_template
            .iter()
            .any(|arg| arg.contains("--voices"));
        is_kokoro && (!has_model_arg || !has_voices_arg)
    }

    /// Check if this is piper TTS — uses ground-truth preset, not a substring
    /// heuristic over the command name (which could match "bagpiper", etc.).
    fn is_piper(&self) -> bool {
        self.preset == "piper"
    }

    fn is_kokoro(&self) -> bool {
        self.preset == "kokoro-tts"
    }

    fn is_kitten(&self) -> bool {
        self.preset == "kitten-tts"
    }

    fn is_pocket(&self) -> bool {
        self.preset == "pocket-tts"
    }

    fn local_server_engine(&self) -> Option<&str> {
        if self.is_kokoro() {
            Some("kokoro")
        } else if self.is_kitten() {
            Some("kitten")
        } else if self.is_pocket() {
            Some("pocket")
        } else {
            None
        }
    }

    pub fn kokoro_model_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        if let Some((model_path, voices_path)) = self.find_kokoro_models() {
            args.push("--model".to_string());
            args.push(model_path);
            args.push("--voices".to_string());
            args.push(voices_path);
        }
        args
    }

    pub fn kitten_model_args(&self) -> Vec<String> {
        // Extract --model from args_template if present
        let model_idx = self.args_template.iter().position(|a| a == "--model");
        if let Some(idx) = model_idx {
            if let Some(val) = self.args_template.get(idx + 1) {
                if val != "{raw_text}" && val != "{input}" && val != "{output}" {
                    return vec!["--model".to_string(), val.clone()];
                }
            }
        }
        Vec::new()
    }

    /// Find kokoro-tts model files in common locations
    fn find_kokoro_models(&self) -> Option<(String, String)> {
        let model_name = "kokoro-v1.0.onnx";
        let voices_name = "voices-v1.0.bin";

        // Check common installation locations
        let search_paths = [
            // Project root kokoro/ folder (dev environment)
            std::env::current_dir().ok().map(|d| d.join("kokoro")),
            // Project root relative to src-tauri/ (CARGO_MANIFEST_DIR)
            Some(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .map(|p| p.join("kokoro"))
                    .unwrap_or_default(),
            ),
            // src-tauri/kokoro/ (adjacent to manifest)
            Some(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("kokoro")),
            // Windows: pip user install
            dirs::home_dir().map(|h| h.join(".local").join("bin")),
            dirs::home_dir().map(|h| h.join("AppData").join("Local").join("bin")),
            dirs::home_dir().map(|h| {
                h.join("AppData")
                    .join("Roaming")
                    .join("Python")
                    .join("Scripts")
            }),
            // pip global install (Windows)
            Some(std::path::PathBuf::from(r"C:\Python310\Scripts")),
            Some(std::path::PathBuf::from(r"C:\Python311\Scripts")),
            Some(std::path::PathBuf::from(r"C:\Python312\Scripts")),
            // Unix-like paths
            Some(std::path::PathBuf::from("/usr/local/bin")),
            Some(std::path::PathBuf::from("/usr/bin")),
            Some(std::path::PathBuf::from("/opt/kokoro-tts")),
        ];

        for base_path in search_paths.iter().flatten() {
            let model_path = base_path.join(model_name);
            let voices_path = base_path.join(voices_name);

            if model_path.exists() && voices_path.exists() {
                return Some((
                    model_path.to_string_lossy().to_string(),
                    voices_path.to_string_lossy().to_string(),
                ));
            }
        }

        None
    }

    /// Build the actual argument list by replacing placeholders.
    /// Placeholders: {input} (input text file path), {output}, {voice}, {data_dir}, {home_dir}, {raw_text}
    /// {raw_text} is the actual text content (not a file path), for engines like pocket-tts that
    /// accept inline text via --text.
    /// {home_dir} resolves to the user's home directory.
    /// {data_dir} resolves to ~/piper-voices for Piper model storage.
    fn build_args(
        &self,
        input_path: &str,
        output_path: &str,
        voice: &str,
        raw_text: &str,
    ) -> Vec<String> {
        let data_dir = Self::data_dir();
        let home_dir = Self::home_dir();
        let mut args: Vec<String> = self
            .args_template
            .iter()
            .map(|arg| {
                let s = arg
                    .replace("{input}", input_path)
                    .replace("{text}", input_path)
                    .replace("{raw_text}", raw_text)
                    .replace("{output}", output_path)
                    .replace("{voice}", voice)
                    .replace("{data_dir}", &data_dir)
                    .replace("{home_dir}", &home_dir);

                // Strip surrounding literal quotes which are common when pasting paths in Windows
                if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                    s[1..s.len() - 1].to_string()
                } else {
                    s
                }
            })
            .collect();

        // Auto-inject kokoro-tts model paths if missing
        if self.is_kokoro_missing_models() {
            if let Some((model_path, voices_path)) = self.find_kokoro_models() {
                log::info!(
                    "[CLI TTS] Auto-adding kokoro-tts model paths: model={}, voices={}",
                    model_path,
                    voices_path
                );
                args.push("--model".to_string());
                args.push(model_path);
                args.push("--voices".to_string());
                args.push(voices_path);
            }
        }

        // Auto-inject --cuda for piper if enabled
        if self.is_piper() && self.cuda {
            args.push("--cuda".to_string());
        }

        args
    }


    /// Returns the Piper voices directory (e.g. C:\Users\<User>\piper-voices on Windows).
    /// Used to resolve the {data_dir} placeholder so TTS engines can locate model files
    /// stored in the user's home directory without requiring the user to enter a full path.
    pub(crate) fn data_dir() -> String {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("piper-voices")
            .to_string_lossy()
            .into_owned()
    }

    /// Returns the user's home directory.
    /// Used for {home_dir} placeholder in CLI templates.
    fn home_dir() -> String {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .to_string_lossy()
            .into_owned()
    }

    fn synthesize_via_server(&self, text: &str, voice: &str, _speed: f32) -> Result<Vec<u8>, TtsError> {
        let data_dir = Self::data_dir();
        let t_total = std::time::Instant::now();

        // R6: Pre-flight check — verify the voice model exists before talking to
        // the server. Piper's HTTP server silently falls back to its default voice
        // (HTTP 200) for unknown voices, so without this check the user hears the
        // wrong voice with no error.
        let voice_stem = if voice.ends_with(".onnx") {
            voice.trim_end_matches(".onnx").to_string()
        } else {
            voice.to_string()
        };
        let model_path = std::path::Path::new(&data_dir).join(format!("{}.onnx", voice_stem));
        if !model_path.exists() && !std::path::Path::new(voice).exists() {
            return Err(TtsError::CommandFailed(format!(
                "Piper voice model not found: {voice}\n\n\
                 Download it with:\n  python -m piper.download_voices {voice}\n\n\
                 Then place the .onnx and .onnx.json files in:\n  {data_dir}",
            )));
        }

        // 1. Ensure server is running and get handle
        let handle = crate::tts::piper_server::ensure_running(
            self.command.clone(),
            voice.to_string(),
            data_dir,
            self.cuda,
        ).map_err(TtsError::Server)?;

        // 2. HTTP synthesis request
        // Speed is a playback-only concept; always synthesize at 1.0 normal rate.
        // The frontend applies speed via playbackRate on the <audio> element.
        let url = format!("http://127.0.0.1:{}/", handle.port);
        let body = serde_json::json!({ "text": text, "voice": voice, "length_scale": 1.0 });

        // R3: Adaptive request deadline — scales with text length so legitimate
        // long synthesis isn't killed, but a wedged server doesn't hold the queue
        // forever (H2 improvement over connect-only timeout).
        let text_chars = text.chars().count() as u64;
        let per_char_ms = if self.cuda { 5 } else { 30 };
        let deadline_ms = (5000u64 + text_chars * per_char_ms).clamp(10_000, 180_000);
        let deadline = std::time::Duration::from_millis(deadline_ms);

        let t_req = std::time::Instant::now();
        let response = handle.client
            .post(&url)
            .timeout(deadline)
            .json(&body)
            .send()
            .map_err(|e| {
                // On abort the server was already unloaded and a fresh prewarm
                // may be starting — don't cancel it with another unload.
                if !crate::ABORT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = crate::tts::piper_server::unload_piper_model();
                }
                TtsError::Server(format!("HTTP request failed: {}", e))
            })?;
        let req_ms = t_req.elapsed().as_millis();

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().unwrap_or_default();
            return Err(TtsError::Server(format!("HTTP error {}: {}", status, err_text)));
        }

        let t_read = std::time::Instant::now();
        let bytes = response
            .bytes()
            .map_err(|e| {
                // R3: Mid-stream read error → dying server. Unload so next
                // attempt gets a fresh process rather than talking to a zombie.
                // (Skip on abort — the abort path already unloaded and may be
                // prewarming a replacement.)
                if !crate::ABORT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = crate::tts::piper_server::unload_piper_model();
                }
                TtsError::Server(format!("Failed to read response bytes: {}", e))
            })?
            .to_vec();
        let read_ms = t_read.elapsed().as_millis();

        let total_ms = t_total.elapsed().as_millis();
        log::info!(
            "[Piper] Synth — total:{:.0}ms req:{:.0}ms read:{:.0}ms size:{}B chars:{} speed:1.0 voice:{} cuda:{}",
            total_ms, req_ms, read_ms, bytes.len(), text.len(), voice, self.cuda
        );

        Ok(bytes)
    }

    fn synthesize_via_local_server(
        &self,
        engine: &str,
        text: &str,
        voice: &str,
        _speed: f32,
    ) -> Result<Vec<u8>, TtsError> {
        let t_total = std::time::Instant::now();

        let script_args = match engine {
            "kokoro" => self.kokoro_model_args(),
            "kitten" => self.kitten_model_args(),
            _ => Vec::new(),
        };

        let handle = crate::tts::local_tts_server::ensure_running(
            engine,
            self.command.clone(),
            script_args,
        )
        .map_err(TtsError::Server)?;

        let url = format!("http://127.0.0.1:{}/", handle.port);
        let body = serde_json::json!({ "text": text, "voice": voice, "length_scale": 1.0 });

        let text_chars = text.chars().count() as u64;
        let deadline_ms = (5000u64 + text_chars * 30).clamp(10_000, 180_000);
        let deadline = std::time::Duration::from_millis(deadline_ms);

        let t_req = std::time::Instant::now();
        let response = handle
            .client
            .post(&url)
            .timeout(deadline)
            .json(&body)
            .send()
            .map_err(|e| {
                if !crate::ABORT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = crate::tts::local_tts_server::unload(engine);
                }
                TtsError::Server(format!("HTTP request failed: {}", e))
            })?;
        let req_ms = t_req.elapsed().as_millis();

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().unwrap_or_default();
            return Err(TtsError::Server(format!(
                "HTTP error {}: {}",
                status, err_text
            )));
        }

        let bytes = response
            .bytes()
            .map_err(|e| {
                if !crate::ABORT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = crate::tts::local_tts_server::unload(engine);
                }
                TtsError::Server(format!("Failed to read response bytes: {}", e))
            })?
            .to_vec();

        let total_ms = t_total.elapsed().as_millis();
        log::info!(
            "[{}_server] Synth — total:{:.0}ms req:{:.0}ms size:{}B chars:{}",
            engine,
            total_ms,
            req_ms,
            bytes.len(),
            text.len()
        );

        Ok(bytes)
    }
}

impl TtsBackend for CliTtsBackend {
    fn name(&self) -> &str {
        &self.command
    }

    fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, TtsError> {
        if self.is_piper() {
            log::debug!("[Piper] Using persistent server — voice:{} speed:{:.1} cuda:{}", voice, speed, self.cuda);
            let synth_start = std::time::Instant::now();
            match self.synthesize_via_server(text, voice, speed) {
                Ok(bytes) => {
                    log::debug!("[Piper] Server synth done — {}B in {:.1}s", bytes.len(), synth_start.elapsed().as_secs_f64());
                    return Ok(bytes);
                }
                Err(e) => {
                    if crate::ABORT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err(e);
                    }
                    log::warn!(
                        "[Piper] Server synthesis failed: {}. Falling back to CLI.",
                        e
                    );
                }
            }
        }

        // Route Kitten/Kokoro/Pocket through persistent local HTTP server
        if let Some(engine) = self.local_server_engine() {
            log::debug!(
                "[LocalServer] Using persistent {} server — voice:{} speed:{:.1}",
                engine,
                voice,
                speed
            );
            let synth_start = std::time::Instant::now();
            match self.synthesize_via_local_server(engine, text, voice, speed) {
                Ok(bytes) => {
                    log::debug!(
                        "[LocalServer] {} synth done — {}B in {:.1}s",
                        engine,
                        bytes.len(),
                        synth_start.elapsed().as_secs_f64()
                    );
                    return Ok(bytes);
                }
                Err(e) => {
                    if crate::ABORT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err(e);
                    }
                    log::warn!(
                        "[LocalServer] {} server synthesis failed: {}. Falling back to CLI.",
                        engine,
                        e
                    );
                }
            }
        }

        let input_file = tempfile::Builder::new()
            .prefix("copyspeak_tts_input_")
            .suffix(".txt")
            .tempfile()
            .map_err(TtsError::Io)?;
        let output_file = tempfile::Builder::new()
            .prefix("copyspeak_tts_out_")
            .suffix(".wav")
            .tempfile()
            .map_err(TtsError::Io)?;

        let input_path = input_file.path().to_string_lossy().to_string();
        let output_path = output_file.path().to_string_lossy().to_string();

        // Write input text to temp file
        if crate::logging::is_debug_mode() {
            log::debug!(
                "[CLI TTS] Writing input text ({} chars) to: {}",
                text.len(),
                input_path
            );
        }
        std::fs::write(&input_path, text).map_err(TtsError::Io)?;

        let args = self.build_args(&input_path, &output_path, voice, text);

        log::info!("[CLI TTS] Starting synthesis");
        log::info!("[CLI TTS] Command: {}", self.command);
        log::info!("[CLI TTS] Args: {:?}", args);
        log::info!("[CLI TTS] Voice: {}", voice);

        let exec_start = std::time::Instant::now();
        #[allow(unused_mut)]
        let mut cmd = Command::new(&self.command);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
            let mut path = get_expanded_path();
            if self.is_piper() && self.cuda {
                if let Some(nvidia_paths) = get_nvidia_dll_paths(&self.command) {
                    path = format!("{};{}", nvidia_paths, path);
                }
            }
            cmd.env("PATH", path);
        }
        let child = cmd
            .spawn()
            .map_err(|e| TtsError::Unavailable(format!("{}: {e}", self.command)))?;

        // Store PID so abort_synthesis can kill this process
        crate::ACTIVE_CLI_PID.store(child.id(), std::sync::atomic::Ordering::Relaxed);

        let result = child
            .wait_with_output()
            .map_err(|e| TtsError::Unavailable(format!("{}: {e}", self.command)))?;

        // Clear PID now that process has exited
        crate::ACTIVE_CLI_PID.store(0, std::sync::atomic::Ordering::Relaxed);

        let exec_duration = exec_start.elapsed();

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let stdout = String::from_utf8_lossy(&result.stdout);
            let output_combined = format!("{} {}", stdout.trim(), stderr.trim());

            log::error!("[CLI TTS] Command failed after {:?}", exec_duration);
            log::error!("[CLI TTS] Exit status: {}", result.status);
            log::error!("[CLI TTS] Stderr: {}", stderr.trim());
            log::error!("[CLI TTS] Stdout: {}", stdout.trim());
            log::error!("[CLI TTS] Command: {}", self.command);
            log::error!("[CLI TTS] Args: {:?}", args);



            // Check if this is piper TTS missing voice model
            if self.is_piper()
                && (output_combined.contains("Unable to find voice")
                    || output_combined.contains("use piper.download_voices"))
            {
                let data_dir = Self::data_dir();
                return Err(TtsError::CommandFailed(format!(
                    "Piper TTS voice model not found: {}\n\n\
                    Voice models must be downloaded separately.\n\n\
                    To download this voice, run:\n\
                    python3 -m piper.download_voices {}\n\n\
                    Then move the downloaded .onnx and .onnx.json files to:\n\
                    {}\n\n\
                    Available voices: amy, arctic, bryce, danny, hfc_female, hfc_male, joe, john, kathleen, kristin, kusal, l2arctic, lessac, libritts, libritts_r, ljspeech, norman, reza_ibrahim, ryan, sam",
                    voice, voice, data_dir
                )));
            }

            // Check if this is kokoro-tts missing model files (and we haven't auto-added them)
            let models_were_auto_added = args.iter().any(|arg| arg.contains("kokoro-v1.0.onnx"));
            if !models_were_auto_added
                && self.is_kokoro_missing_models()
                && (output_combined.contains("model files are missing")
                    || output_combined.contains("kokoro-v1.0.onnx")
                    || output_combined.contains("voices-v1.0.bin"))
            {
                // Try to find models for a helpful error message
                if let Some((model_path, voices_path)) = self.find_kokoro_models() {
                    return Err(TtsError::CommandFailed(format!(
                        "Kokoro-TTS auto-configuration failed.\n\n\
                        The app found models at:\n\
                        Model: {}\n\
                        Voices: {}\n\n\
                        But the TTS engine still couldn't load them.\n\
                        Try manually adding these arguments to your TTS settings:\n\
                        --model \"{}\" --voices \"{}\"",
                        model_path, voices_path, model_path, voices_path
                    )));
                } else {
                    return Err(TtsError::CommandFailed(
                        "Kokoro-TTS is missing required model files.\n\n\
                        Please download them:\n\
                        1. kokoro-v1.0.onnx\n\
                        2. voices-v1.0.bin\n\n\
                        From: https://github.com/nazdridoy/kokoro-tts/releases\n\n\
                        Place them in the same folder as kokoro-tts.exe."
                            .to_string(),
                    ));
                }
            }

            return Err(TtsError::CommandFailed(format!(
                "exit {}: {}",
                result.status,
                stderr.trim()
            )));
        }

        if crate::logging::is_debug_mode() {
            log::debug!(
                "[CLI TTS] Command completed successfully in {:?}",
                exec_duration
            );
        }

        // Read output file with enhanced error handling
        if crate::logging::is_debug_mode() {
            log::debug!("[CLI TTS] Reading output file: {}", output_path);
        }

        // Check if output file exists
        if !std::path::Path::new(&output_path).exists() {

            log::error!(
                "[CLI TTS] Output file not found after successful command: {}",
                output_path
            );
            return Err(TtsError::OutputNotFound(format!(
                "TTS command succeeded but output file was not created: {}. Check TTS engine configuration.",
                output_path
            )));
        }

        let bytes = std::fs::read(&output_path).map_err(|e| {

            log::error!(
                "[CLI TTS] Failed to read output file '{}': {}",
                output_path,
                e
            );
            match e.kind() {
                std::io::ErrorKind::PermissionDenied => TtsError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Permission denied reading TTS output file: {}", output_path),
                )),
                _ => TtsError::OutputNotFound(format!(
                    "Failed to read TTS output file '{}': {}. The file may be corrupted or locked.",
                    output_path, e
                )),
            }
        })?;

        // Validate output
        if bytes.is_empty() {

            log::error!("[CLI TTS] Output file is empty: {}", output_path);
            return Err(TtsError::OutputNotFound("TTS engine created an empty audio file. The TTS synthesis may have failed.".to_string()));
        }

        if crate::logging::is_debug_mode() {
            log::debug!("[CLI TTS] Read {} bytes from output file", bytes.len());
            log::debug!("[CLI TTS] Cleaning up temp files");
        }



        if crate::logging::is_debug_mode() {
            log::debug!(
                "[CLI TTS] Synthesis complete - returned {} bytes",
                bytes.len()
            );
        }

        Ok(bytes)
    }

    fn health_check(&self) -> Result<(), TtsError> {
        if crate::logging::is_debug_mode() {
            log::debug!(
                "[CLI TTS] Running health check for command: {}",
                self.command
            );
        }

        // Check if command exists with version args (e.g., py -3.12)
        let is_python =
            self.command == "py" || self.command == "python" || self.command.starts_with("python");

        if is_python {
            // H5: Piper fast path — if the persistent server is already running, a
            // quick ping is sufficient. No need to spawn a full CLI synthesis
            // (seconds of model load) for a health check.
            if self.is_piper() {
                let status = crate::tts::piper_server::get_piper_server_status();
                if status.ready {
                    if let Some(port) = status.port {
                        let client = reqwest::blocking::Client::builder()
                            .timeout(std::time::Duration::from_secs(2))
                            .build()
                            .map_err(|e| TtsError::Unavailable(format!("Failed to build health client: {e}")))?;
                        let url = format!("http://127.0.0.1:{}/voices", port);
                        if client.get(&url).send().is_ok() {
                            return Ok(());
                        }
                    }
                }
                if status.running {
                    return Ok(()); // Server is starting — making progress
                }
                // Server is Stopped — fall through to cheap probe below
            }

            // For Python, we need to validate the full command with the script
            // Build args similar to synthesize, but use a test text
            let test_text = "test";

            let test_voice = if self.is_piper() {
                // Find any downloaded voice to use for testing
                let data_dir = Self::data_dir();
                let path = std::path::Path::new(&data_dir);
                let mut found_voice = None;
                if path.exists() {
                    if let Ok(entries) = std::fs::read_dir(path) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            let p = entry.path();
                            if p.is_file() && p.extension().is_some_and(|ext| ext == "onnx") {
                                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                                    found_voice = Some(stem.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
                found_voice.unwrap_or_else(|| "en_US-joe-medium".to_string())
            } else {
                "Rosie".to_string()
            };

            let input_file = tempfile::Builder::new()
                .prefix("copyspeak_tts_health_input_")
                .suffix(".txt")
                .tempfile()
                .map_err(|e| TtsError::Unavailable(format!("Failed to create temp input: {e}")))?;
            let output_file = tempfile::Builder::new()
                .prefix("copyspeak_tts_health_out_")
                .suffix(".wav")
                .tempfile()
                .map_err(|e| TtsError::Unavailable(format!("Failed to create temp output: {e}")))?;

            let test_input_path = input_file.path().to_string_lossy().to_string();
            let test_output = output_file.path().to_string_lossy().to_string();

            let args = self.build_args(&test_input_path, &test_output, &test_voice, test_text);

            if crate::logging::is_debug_mode() {
                log::debug!("[CLI TTS] Health check args: {:?}", args);
            }

            // Write a minimal input file
            std::fs::write(&test_input_path, test_text)
                .map_err(|e| TtsError::Unavailable(format!("Failed to write temp input: {e}")))?;

            #[allow(unused_mut)]
            let mut cmd = Command::new(&self.command);
            cmd.args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
                cmd.env("PATH", get_expanded_path());
            }

            let result = cmd.output().map_err(|e| {
                log::error!(
                    "[CLI TTS] Health check failed - command not found: {}",
                    self.command
                );
                TtsError::Unavailable(format!("{} not found: {e}", self.command))
            })?;

            // Check for Python version errors in stderr
            let stderr = String::from_utf8_lossy(&result.stderr);
            let _stdout = String::from_utf8_lossy(&result.stdout);

            if stderr.contains("No runtime installed") {
                log::error!("[CLI TTS] Health check failed - Python runtime not installed");
                return Err(TtsError::Unavailable(
                    "Python runtime is not installed or not found. Please install Python 3.8+ from https://www.python.org/downloads/".to_string()
                ));
            }

            if stderr.contains("is not recognized") || stderr.contains("not found") {
                log::error!("[CLI TTS] Health check failed - command not recognized");
                return Err(TtsError::Unavailable(format!(
                    "Command '{}' not found. Please ensure the TTS engine is installed and in PATH.",
                    self.command
                )));
            }

            if stderr.contains("ModuleNotFoundError") || stderr.contains("ImportError") {
                log::error!("[CLI TTS] Health check failed - missing Python module");
                return Err(TtsError::Unavailable(
                    "KittenTTS is not installed in the configured Python environment. Run the KittenTTS installer from CopySpeak TTS settings.".to_string()
                ));
            }

            if !result.status.success() {
                let error_msg = if stderr.is_empty() {
                    format!("Command exited with code {:?}", result.status.code())
                } else {
                    stderr.trim().to_string()
                };
                log::error!(
                    "[CLI TTS] Health check failed - command error: {}",
                    error_msg
                );
                return Err(TtsError::Unavailable(format!(
                    "TTS command failed: {}",
                    error_msg
                )));
            }

            // If command succeeded or script ran (even with other errors), consider it available
            if crate::logging::is_debug_mode() {
                log::debug!("[CLI TTS] Health check passed for Python command");
            }
        } else {
            // For non-Python commands, use simple --help check
            #[allow(unused_mut)]
            let mut cmd = Command::new(&self.command);
            cmd.arg("--help");
            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
                cmd.env("PATH", get_expanded_path());
            }
            cmd.output().map_err(|e| {
                log::error!(
                    "[CLI TTS] Health check failed - command not found: {}",
                    self.command
                );
                TtsError::Unavailable(format!("{} not found: {e}", self.command))
            })?;
        }

        // For Piper, check if voice files exist
        if self.is_piper() {
            // Check if the piper-voices directory exists
            let data_dir = Self::data_dir();
            let data_path = std::path::Path::new(&data_dir);
            if !data_path.exists() {
                log::warn!("[CLI TTS] Piper voices directory not found: {}", data_dir);
                return Err(TtsError::Unavailable(format!(
                    "Piper voices directory not found: {}\n\n\
                    Please create this directory and download voice models.\n\n\
                    Available voices: amy, arctic, bryce, danny, hfc_female, hfc_male, joe, john, kathleen, kristin, kusal, l2arctic, lessac, libritts, libritts_r, ljspeech, norman, reza_ibrahim, ryan, sam",
                    data_dir
                )));
            }

            // Check if there are any .onnx files in the directory
            let has_voices = data_path
                .read_dir()
                .map(|entries| {
                    entries.filter_map(|e| e.ok()).any(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "onnx")
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            if !has_voices {
                log::warn!("[CLI TTS] No Piper voice files found in: {}", data_dir);
                return Err(TtsError::Unavailable(format!(
                    "No Piper voice files found in: {}\n\n\
                    Please download voice models and place them in this directory.\n\n\
                    Available voices: amy, arctic, bryce, danny, hfc_female, hfc_male, joe, john, kathleen, kristin, kusal, l2arctic, lessac, libritts, libritts_r, ljspeech, norman, reza_ibrahim, ryan, sam",
                    data_dir
                )));
            }
        }

        if crate::logging::is_debug_mode() {
            log::debug!("[CLI TTS] Health check passed");
        }
        Ok(())
    }

    fn voice_display_name(&self, voice_id: &str) -> String {
        let parts: Vec<&str> = voice_id.split('-').collect();
        if parts.len() >= 2 {
            parts[1].to_lowercase()
        } else {
            voice_id.to_lowercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_args_replaces_placeholders() {
        let backend = CliTtsBackend::new(
            "test-tts".into(),
            vec![
                "{input}".into(),
                "{output}".into(),
                "--voice".into(),
                "{voice}".into(),
            ],
            false,
            "test".into(),
        );
        let args = backend.build_args("/tmp/input.txt", "/tmp/out.wav", "af_heart", "hello world");
        assert_eq!(
            args,
            vec!["/tmp/input.txt", "/tmp/out.wav", "--voice", "af_heart"]
        );
    }

    #[test]
    fn test_build_args_legacy_text_placeholder() {
        // Test backward compatibility: {text} should also work as input file path
        let backend = CliTtsBackend::new(
            "test-tts".into(),
            vec!["{text}".into(), "{output}".into()],
            false,
            "test".into(),
        );
        let args = backend.build_args("/tmp/input.txt", "/tmp/out.wav", "af_heart", "hello world");
        assert_eq!(args, vec!["/tmp/input.txt", "/tmp/out.wav"]);
    }

    #[test]
    fn test_voice_display_name_extracts_middle_segment() {
        let backend = CliTtsBackend::new("piper".into(), vec![], false, "piper".into());

        // Piper voice format: en_US-joe-medium
        assert_eq!(backend.voice_display_name("en_US-joe-medium"), "joe");
        assert_eq!(backend.voice_display_name("en_US-amy-medium"), "amy");

        // Kokoro/Pocket format: af_heart (single segment)
        assert_eq!(backend.voice_display_name("af_heart"), "af_heart");
        assert_eq!(backend.voice_display_name("alba"), "alba");
    }

    #[test]
    fn test_piper_request_body_serialization() {
        let text = "Hello world";
        let voice = "en_US-joe-medium";
        // Speed is playback-only; synthesis always uses length_scale: 1.0
        let body = serde_json::json!({ "text": text, "voice": voice, "length_scale": 1.0 });
        assert_eq!(body["text"], text);
        assert_eq!(body["voice"], voice);
        assert_eq!(body["length_scale"], 1.0);
    }
}

