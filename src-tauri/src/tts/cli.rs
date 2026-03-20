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

pub struct CliTtsBackend {
    pub command: String,
    pub args_template: Vec<String>,
}

impl CliTtsBackend {
    pub fn new(command: String, args_template: Vec<String>) -> Self {
        Self {
            command,
            args_template,
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

        for path_opt in search_paths.iter() {
            if let Some(base_path) = path_opt {
                let model_path = base_path.join(model_name);
                let voices_path = base_path.join(voices_name);

                if model_path.exists() && voices_path.exists() {
                    return Some((
                        model_path.to_string_lossy().to_string(),
                        voices_path.to_string_lossy().to_string(),
                    ));
                }
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
    fn data_dir() -> String {
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
}

impl TtsBackend for CliTtsBackend {
    fn name(&self) -> &str {
        &self.command
    }

    fn synthesize(&self, text: &str, voice: &str, _speed: f32) -> Result<Vec<u8>, TtsError> {
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
        cmd.creation_flags(CREATE_NO_WINDOW);
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

        // Check if command exists
        #[allow(unused_mut)]
        let mut cmd = Command::new(&self.command);
        cmd.arg("--help");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.output().map_err(|e| {
            log::error!(
                "[CLI TTS] Health check failed - command not found: {}",
                self.command
            );
            TtsError::Unavailable(format!("{} not found: {e}", self.command))
        })?;

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
        let backend =
            CliTtsBackend::new("test-tts".into(), vec!["{text}".into(), "{output}".into()]);
        let args = backend.build_args("/tmp/input.txt", "/tmp/out.wav", "af_heart", "hello world");
        assert_eq!(args, vec!["/tmp/input.txt", "/tmp/out.wav"]);
    }

    #[test]
    fn test_voice_display_name_extracts_middle_segment() {
        let backend = CliTtsBackend::new("piper".into(), vec![]);

        // Piper voice format: en_US-joe-medium
        assert_eq!(backend.voice_display_name("en_US-joe-medium"), "joe");
        assert_eq!(backend.voice_display_name("en_US-amy-medium"), "amy");

        // Kokoro/Pocket format: af_heart (single segment)
        assert_eq!(backend.voice_display_name("af_heart"), "af_heart");
        assert_eq!(backend.voice_display_name("alba"), "alba");
    }
}
