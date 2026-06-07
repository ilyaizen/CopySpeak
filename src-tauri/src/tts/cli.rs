// CLI TTS backend.
// Spawns an external TTS command (e.g. kokoro-tts), waits for it to write a WAV file,
// reads the file, and returns the bytes. The command and args are fully configurable
// via the args_template in config, with {input}, {output}, and {voice} placeholders.
//
// Note: kokoro-tts reads from a FILE, not a command-line text argument, so we write
// the text to a temp file and pass its path via {input}.

use super::{TtsBackend, TtsError};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct PiperServerState {
    pub child: std::process::Child,
    pub model_name: String,
    pub port: u16,
    pub cuda: bool,
    pub client: reqwest::blocking::Client,
}

pub static PIPER_SERVER: OnceLock<Mutex<Option<PiperServerState>>> = OnceLock::new();

pub fn get_piper_server() -> &'static Mutex<Option<PiperServerState>> {
    PIPER_SERVER.get_or_init(|| Mutex::new(None))
}

pub fn unload_piper_model_internal() -> bool {
    let mut server = get_piper_server().lock().unwrap();
    if let Some(mut state) = server.take() {
        log::info!(
            "[Piper] Unloading model: killing server on port {}",
            state.port
        );
        let _ = state.child.kill();
        let _ = state.child.wait();
        log::info!("[Piper] Server process terminated on port {}", state.port);
        true
    } else {
        false
    }
}

/// Restart the Piper HTTP server with new settings (model, CUDA, etc).
/// Kills any existing server first, then starts a fresh one.
/// Called when engine config changes (voice, CUDA) via set_config.
pub fn restart_piper_server(command: String, voice: String, data_dir: String, cuda: bool) {
    log::info!("[Piper] Restart requested — voice: {}, cuda: {}", voice, cuda);

    // Kill existing server first
    let had_server = unload_piper_model_internal();
    if had_server {
        log::info!("[Piper] Killed existing server before restart");
    }

    // Start new server in background
    prewarm_piper_server(command, voice, data_dir, cuda);
}

/// Return the current Piper server status for the control server /health check.
#[derive(serde::Serialize)]
pub struct PiperServerStatus {
    pub running: bool,
    pub model: Option<String>,
    pub port: Option<u16>,
    pub cuda: bool,
    pub ready: bool,
}

pub fn get_piper_server_status() -> PiperServerStatus {
    let mut server = get_piper_server().lock().unwrap();
    if let Some(ref mut state) = *server {
        let is_alive = matches!(state.child.try_wait(), Ok(None));
        PiperServerStatus {
            running: is_alive,
            model: Some(state.model_name.clone()),
            port: Some(state.port),
            cuda: state.cuda,
            ready: is_alive,
        }
    } else {
        PiperServerStatus {
            running: false,
            model: None,
            port: None,
            cuda: false,
            ready: false,
        }
    }
}

fn get_free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .ok()
}

/// Pre-warm the Piper HTTP server at app startup or after config change.
/// Starts the server in a background thread so the model is loaded into RAM
/// before the first synthesis request, eliminating the 2-10s cold-start penalty.
pub fn prewarm_piper_server(command: String, voice: String, data_dir: String, cuda: bool) {
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        log::info!("[Piper] Pre-warming server — voice: {}, cuda: {}", voice, cuda);

        let voice_file = if voice.ends_with(".onnx") {
            voice.clone()
        } else {
            format!("{}.onnx", voice)
        };
        let mut model_path = std::path::PathBuf::from(&data_dir).join(&voice_file);
        if !model_path.exists() {
            let alt_path = std::path::PathBuf::from(&voice);
            if alt_path.exists() {
                model_path = alt_path;
            } else {
                log::warn!("[Piper] Pre-warm failed: model file not found at {}", model_path.display());
                return;
            }
        }

        let port = match get_free_port() {
            Some(p) => p,
            None => {
                log::warn!("[Piper] Pre-warm failed: no free port available");
                return;
            }
        };

        let model_display = model_path.display();
        log::info!("[Piper] Starting HTTP server on port {} — model: {}, cuda: {}", port, model_display, cuda);
        let server_start = std::time::Instant::now();

        let mut cmd = std::process::Command::new(&command);
        let mut args = vec![
            "-m".to_string(),
            "piper.http_server".to_string(),
            "-m".to_string(),
            model_path.to_string_lossy().to_string(),
            "--port".to_string(),
            port.to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
        ];

        if !data_dir.is_empty() {
            args.push("--data-dir".to_string());
            args.push(data_dir.clone());
        }

        if cuda {
            args.push("--cuda".to_string());
        }

        cmd.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
            if cuda {
                if let Some(nvidia_paths) = get_nvidia_dll_paths(&command) {
                    let current_path = std::env::var("PATH").unwrap_or_default();
                    let new_path = format!("{};{}", nvidia_paths, current_path);
                    cmd.env("PATH", new_path);
                }
            }
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[Piper] Pre-warm failed: spawn error — {}", e);
                return;
            }
        };

        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                let _ = child.kill();
                log::warn!("[Piper] Pre-warm failed: could not build HTTP client");
                return;
            }
        };

        let url = format!("http://127.0.0.1:{}/voices", port);
        let poll_start = std::time::Instant::now();
        let mut ready = false;

        while poll_start.elapsed() < std::time::Duration::from_secs(15) {
            if let Ok(Some(status)) = child.try_wait() {
                log::warn!("[Piper] Pre-warm failed: server exited with {:?} after {:?}", status.code(), poll_start.elapsed());
                return;
            }
            if client.get(&url).send().is_ok() {
                ready = true;
                break;
            }
            let elapsed = poll_start.elapsed().as_millis();
            let delay = if elapsed < 2000 { 50 } else { 200 };
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }

        if !ready {
            let _ = child.kill();
            log::warn!("[Piper] Pre-warm failed: timed out after {:?}", poll_start.elapsed());
            return;
        }

        let total_duration = start.elapsed();
        log::info!(
            "[Piper] Server ready on port {} — model loaded in {:.1}s (startup: {:.1}s, poll: {:.1}s)",
            port,
            total_duration.as_secs_f64(),
            server_start.elapsed().as_secs_f64(),
            poll_start.elapsed().as_secs_f64()
        );

        // Warm-up: send a minimal text to force ONNX runtime JIT / GPU kernel init.
        // This makes the FIRST real synthesis fast instead of paying the warm-up penalty.
        let warmup_client = reqwest::blocking::Client::new();
        let warmup_url = format!("http://127.0.0.1:{}/", port);
        let warmup_body = serde_json::json!({ "text": "Hello" });
        let warmup_start = std::time::Instant::now();
        match warmup_client.post(&warmup_url).json(&warmup_body).send() {
            Ok(resp) => {
                let _ = resp.bytes(); // discard — not saved, not played
                log::info!(
                    "[Piper] Warm-up synthesis completed in {:.1}s (1st-inference JIT/GPU init)",
                    warmup_start.elapsed().as_secs_f64()
                );
            }
            Err(e) => {
                log::warn!("[Piper] Warm-up synthesis failed: {}. First real synthesis will be slower.", e);
            }
        }

        let mut server = get_piper_server().lock().unwrap();
        *server = Some(PiperServerState {
            child,
            model_name: voice.clone(),
            port,
            cuda,
            client: reqwest::blocking::Client::new(),
        });
    });
}

#[cfg(windows)]
fn get_expanded_path() -> String {
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
}

impl CliTtsBackend {
    pub fn new(command: String, args_template: Vec<String>, cuda: bool) -> Self {
        Self {
            command,
            args_template,
            cuda,
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

    /// Check if this is piper TTS
    fn is_piper(&self) -> bool {
        let cmd_lower = self.command.to_lowercase();
        cmd_lower.contains("piper") || self.args_template.iter().any(|arg| arg.contains("piper"))
    }

    /// Find kokoro-tts model files in common locations
    fn find_kokoro_models(&self) -> Option<(String, String)> {
        let model_name = "kokoro-v1.0.onnx";
        let voices_name = "voices-v1.0.bin";

        // Check common installation locations
        let search_paths = [
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

    fn input_path() -> String {
        let tmp = std::env::temp_dir();
        tmp.join("copyspeak_tts_input.txt")
            .to_string_lossy()
            .into_owned()
    }

    fn output_path() -> String {
        let tmp = std::env::temp_dir();
        tmp.join("copyspeak_tts_out.wav")
            .to_string_lossy()
            .into_owned()
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

    fn synthesize_via_server(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, String> {
        let cuda = self.cuda;
        let data_dir = Self::data_dir();

        // Resolve model file path
        let voice_file = if voice.ends_with(".onnx") {
            voice.to_string()
        } else {
            format!("{}.onnx", voice)
        };
        let mut model_path = std::path::PathBuf::from(&data_dir).join(&voice_file);
        if !model_path.exists() {
            let alt_path = std::path::PathBuf::from(voice);
            if alt_path.exists() {
                model_path = alt_path;
            } else {
                return Err(format!("Model file not found: {}", model_path.display()));
            }
        }

        let mut server = get_piper_server().lock().unwrap();
        let mut need_start = true;

        if let Some(ref mut state) = *server {
            let is_running = matches!(state.child.try_wait(), Ok(None));

            if is_running && state.model_name == voice && state.cuda == cuda {
                need_start = false;
            } else {
                log::info!(
                    "[Piper] Restarting server — model change (was: {}, cuda: {}) → (new: {}, cuda: {})",
                    state.model_name,
                    state.cuda,
                    voice,
                    cuda
                );
                let _ = state.child.kill();
                *server = None;
            }
        }

        let _port = if need_start {
            let new_port =
                get_free_port().ok_or_else(|| "Failed to find a free port".to_string())?;
            log::info!(
                "[Piper] Starting HTTP server on port {} — model: {}",
                new_port,
                voice
            );

            let mut cmd = Command::new(&self.command);
            let mut args = vec![
                "-m".to_string(),
                "piper.http_server".to_string(),
                "-m".to_string(),
                model_path.to_string_lossy().to_string(),
                "--port".to_string(),
                new_port.to_string(),
                "--host".to_string(),
                "127.0.0.1".to_string(),
            ];

            if !data_dir.is_empty() {
                args.push("--data-dir".to_string());
                args.push(data_dir.clone());
            }

            if cuda {
                args.push("--cuda".to_string());
            }

            cmd.args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
                if cuda {
                    if let Some(nvidia_paths) = get_nvidia_dll_paths(&self.command) {
                        let current_path = std::env::var("PATH").unwrap_or_default();
                        let new_path = format!("{};{}", nvidia_paths, current_path);
                        cmd.env("PATH", new_path);
                    }
                }
            }

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to spawn Piper server: {}", e))?;

            // Poll /voices to check if server is ready
            let health_client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_millis(500))
                .build()
                .map_err(|e| e.to_string())?;

            let url = format!("http://127.0.0.1:{}/voices", new_port);
            let start = std::time::Instant::now();
            let mut ready = false;

            while start.elapsed() < std::time::Duration::from_secs(15) {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(format!("Piper server exited prematurely: {:?}", status));
                }

                if health_client.get(&url).send().is_ok() {
                    ready = true;
                    break;
                }
                let elapsed = start.elapsed().as_millis();
                let delay = if elapsed < 2000 { 50 } else { 200 };
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }

            if !ready {
                let _ = child.kill();
                return Err("Timeout waiting for Piper server to start".to_string());
            }

            log::info!("[Piper] HTTP server ready on port {} — took {:.1}s", new_port, start.elapsed().as_secs_f64());
            *server = Some(PiperServerState {
                child,
                model_name: voice.to_string(),
                port: new_port,
                cuda,
                client: health_client,
            });
            new_port
        } else {
            server
                .as_ref()
                .ok_or_else(|| "Piper server state is unexpectedly missing".to_string())?
                .port
        };

        drop(server);

        // Reuse the server's HTTP client and perform synthesis
        let server_state = get_piper_server().lock().unwrap();
        let (client, port) = if let Some(ref state) = *server_state {
            (Some(state.client.clone()), state.port)
        } else {
            return Err("Piper server unexpectedly disappeared".to_string());
        };
        let client = client.ok_or_else(|| "Piper server client not available".to_string())?;
        drop(server_state);

        let url = format!("http://127.0.0.1:{}/", port);
        let body = serde_json::json!({
            "text": text,
            "length_scale": speed,
        });

        let response = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().unwrap_or_default();
            return Err(format!("HTTP error {}: {}", status, err_text));
        }

        let bytes = response
            .bytes()
            .map_err(|e| format!("Failed to read response bytes: {}", e))?;

        Ok(bytes.to_vec())
    }
}

impl TtsBackend for CliTtsBackend {
    fn name(&self) -> &str {
        &self.command
    }

    fn synthesize(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>, TtsError> {
        if self.is_piper() {
            log::info!("[Piper] Using persistent server for voice: {} (cuda: {})", voice, self.cuda);
            let synth_start = std::time::Instant::now();
            match self.synthesize_via_server(text, voice, speed) {
                Ok(bytes) => {
                    log::info!("[Piper] Synthesis via server completed — {} bytes in {:.1}s", bytes.len(), synth_start.elapsed().as_secs_f64());
                    return Ok(bytes);
                }
                Err(e) => {
                    log::warn!(
                        "[Piper] Server synthesis failed: {}. Falling back to CLI.",
                        e
                    );
                }
            }
        }

        let input_path = Self::input_path();
        let output_path = Self::output_path();

        // Write input text to temp file
        if crate::logging::is_debug_mode() {
            log::debug!(
                "[CLI TTS] Writing input text ({} chars) to: {}",
                text.len(),
                input_path
            );
        }
        std::fs::write(&input_path, text).map_err(TtsError::Io)?;

        // Clean up any existing output file
        let _ = std::fs::remove_file(&output_path);

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

            let _ = std::fs::remove_file(&input_path);

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
            let _ = std::fs::remove_file(&input_path);
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
            let _ = std::fs::remove_file(&input_path);
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
            let _ = std::fs::remove_file(&input_path);
            let _ = std::fs::remove_file(&output_path);
            log::error!("[CLI TTS] Output file is empty: {}", output_path);
            return Err(TtsError::OutputNotFound(format!(
                "TTS engine created an empty audio file. The TTS synthesis may have failed."
            )));
        }

        if crate::logging::is_debug_mode() {
            log::debug!("[CLI TTS] Read {} bytes from output file", bytes.len());
            log::debug!("[CLI TTS] Cleaning up temp files");
        }

        // Cleanup temp files
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);

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
            // For Python, we need to validate the full command with the script
            // Build args similar to synthesize, but use a test text
            let test_text = "test";
            let test_output = Self::output_path();

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

            let args = self.build_args(&Self::input_path(), &test_output, &test_voice, test_text);

            if crate::logging::is_debug_mode() {
                log::debug!("[CLI TTS] Health check args: {:?}", args);
            }

            // Clean up any existing test files
            let _ = std::fs::remove_file(Self::input_path());
            let _ = std::fs::remove_file(&test_output);

            // Write a minimal input file
            let _ = std::fs::write(Self::input_path(), test_text);

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

            // Clean up test files
            let _ = std::fs::remove_file(Self::input_path());
            let _ = std::fs::remove_file(&test_output);
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
        );
        let args = backend.build_args("/tmp/input.txt", "/tmp/out.wav", "af_heart", "hello world");
        assert_eq!(args, vec!["/tmp/input.txt", "/tmp/out.wav"]);
    }

    #[test]
    fn test_voice_display_name_extracts_middle_segment() {
        let backend = CliTtsBackend::new("piper".into(), vec![], false);

        // Piper voice format: en_US-joe-medium
        assert_eq!(backend.voice_display_name("en_US-joe-medium"), "joe");
        assert_eq!(backend.voice_display_name("en_US-amy-medium"), "amy");

        // Kokoro/Pocket format: af_heart (single segment)
        assert_eq!(backend.voice_display_name("af_heart"), "af_heart");
        assert_eq!(backend.voice_display_name("alba"), "alba");
    }
}
