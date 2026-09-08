// TTS engine health check commands.

use crate::config::{AppConfig, TtsConfig, TtsEngine};
use crate::tts::cli::CliTtsBackend;
use crate::tts::{TtsBackend, TtsError};
use std::sync::Mutex;
use tauri::State;

use super::helpers::{
    create_backend, create_backend_from_effective, resolve_effective, EffectiveTtsRequest,
};

/// Result of a TTS engine health check.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TtsHealthResult {
    pub success: bool,
    pub message: String,
    pub error_type: Option<String>,
}

/// Result of checking if a command exists in PATH.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandExistsResult {
    pub available: bool,
}

/// Check if a command exists in the system PATH.
/// This is used to check if local TTS engines are installed without fully testing them.
#[tauri::command]
pub fn check_command_exists(command: String) -> Result<CommandExistsResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] check_command_exists called for: {}", command);
    }

    // Try to find the command in PATH using `which` on Unix or `where` on Windows.
    // CREATE_NO_WINDOW prevents a flash of terminal window on Windows.
    #[cfg(target_os = "windows")]
    let result = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("where")
            .arg(&command)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    };
    #[cfg(not(target_os = "windows"))]
    let result = std::process::Command::new("which").arg(&command).output();

    match result {
        Ok(output) => {
            let available = output.status.success();
            if crate::logging::is_debug_mode() {
                log::debug!("[IPC] check_command_exists({}): {}", command, available);
            }
            Ok(CommandExistsResult { available })
        }
        Err(e) => {
            log::warn!("[IPC] check_command_exists failed for {}: {}", command, e);
            Ok(CommandExistsResult { available: false })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CartesiaEngineOptions, ProfileEngineOptions};

    /// A config as it comes back off disk: the `cartesia` object exists (it holds
    /// the persisted api_key), so `model_id`'s *field-level* `#[serde(default)]`
    /// wins over the container default and yields `""`. Omitting the object
    /// entirely would instead give `CartesiaConfig::default()` — not what a real
    /// install looks like.
    fn loaded_config() -> TtsConfig {
        serde_json::from_str(r#"{ "cartesia": { "api_key": "sk-test" } }"#).expect("deserializes")
    }

    fn cartesia_effective(model_id: Option<&str>) -> EffectiveTtsRequest {
        EffectiveTtsRequest {
            profile_id: None,
            profile_name: None,
            engine: TtsEngine::Cartesia,
            voice: String::new(),
            voice_label: None,
            pitch: 1.0,
            effects: Default::default(),
            text_processing: Default::default(),
            engine_options: ProfileEngineOptions::Cartesia(CartesiaEngineOptions {
                model_id: model_id.map(String::from),
                ..Default::default()
            }),
        }
    }

    /// A config round-tripped through serde loses model/voice fields (they are
    /// `skip_serializing` — the profile owns them). The label must come from the
    /// profile, not the blanked global config. Regression for `Cartesia ()`.
    #[test]
    fn label_prefers_profile_options_over_blanked_config() {
        let loaded = loaded_config();
        assert!(
            loaded.cartesia.model_id.is_empty(),
            "precondition: global model_id deserializes blank"
        );

        let eff = cartesia_effective(Some("sonic-3.5"));
        assert_eq!(
            effective_backend_name(&eff, &loaded),
            "Cartesia (sonic-3.5)"
        );
    }

    #[test]
    fn label_says_unset_when_nothing_is_configured() {
        let loaded = loaded_config();
        let eff = cartesia_effective(None);
        assert_eq!(effective_backend_name(&eff, &loaded), "Cartesia (unset)");
    }

    #[test]
    fn first_set_skips_blank_and_whitespace() {
        assert_eq!(first_set(&["", "  ", "b"]), "b");
        assert_eq!(first_set(&["a", "b"]), "a");
        assert_eq!(first_set(&[""]), "unset");
    }
}

fn parse_engine(engine: &str) -> Result<TtsEngine, String> {
    match engine {
        "local" => Ok(TtsEngine::Local),
        "http" => Ok(TtsEngine::Http),
        "openai" => Ok(TtsEngine::OpenAI),
        "elevenlabs" => Ok(TtsEngine::ElevenLabs),
        "cartesia" => Ok(TtsEngine::Cartesia),
        "google" => Ok(TtsEngine::Google),
        "microsoft" => Ok(TtsEngine::Microsoft),
        "edge" => Ok(TtsEngine::Edge),
        "kitten" => Ok(TtsEngine::Kitten),
        "piper" => Ok(TtsEngine::Piper),
        "kokoro" => Ok(TtsEngine::Kokoro),
        "pocket" => Ok(TtsEngine::Pocket),
        _ => Err(format!("unknown engine: {}", engine)),
    }
}

#[tauri::command]
pub fn test_tts_engine(config: State<'_, Mutex<AppConfig>>) -> Result<TtsHealthResult, String> {
    if crate::logging::is_debug_mode() {
        log::debug!("[IPC] test_tts_engine called");
    }

    let (effective, tts_config) = {
        let cfg = config.lock().unwrap();
        let tts = cfg.tts.clone();
        (resolve_effective(&tts), tts)
    };

    let backend: Box<dyn crate::tts::TtsBackend> =
        create_backend_from_effective(&effective, &tts_config);

    let backend_name = effective_backend_name(&effective, &tts_config);

    health_result(backend, backend_name)
}

/// First non-blank candidate, or "unset".
fn first_set<'a>(candidates: &[&'a str]) -> &'a str {
    candidates
        .iter()
        .copied()
        .find(|s| !s.trim().is_empty())
        .unwrap_or("unset")
}

/// Label for the backend `create_backend_from_effective` actually built.
///
/// The profile's engine options win over the global `TtsConfig`, mirroring the
/// override order in `create_backend_from_effective`. Reading the global config
/// alone is wrong: model/voice/format fields there are `skip_serializing` (the
/// profile owns them), so a loaded config deserializes them blank — which is
/// where the `Cartesia ()` in the logs came from.
fn effective_backend_name(eff: &EffectiveTtsRequest, tts: &TtsConfig) -> String {
    let opts = &eff.engine_options;
    // Bind the profile-side Strings so the &str candidates can borrow them.
    let (openai, elevenlabs, cartesia, http, google, microsoft, edge, local) = (
        opts.openai()
            .and_then(|o| o.model.clone())
            .unwrap_or_default(),
        opts.elevenlabs()
            .and_then(|o| o.model_id.clone())
            .unwrap_or_default(),
        opts.cartesia()
            .and_then(|o| o.model_id.clone())
            .unwrap_or_default(),
        opts.http()
            .and_then(|o| o.url_template.clone())
            .unwrap_or_default(),
        opts.google()
            .and_then(|o| o.model.clone())
            .unwrap_or_default(),
        opts.microsoft()
            .and_then(|o| o.model.clone())
            .unwrap_or_default(),
        opts.edge()
            .and_then(|o| o.voice.clone())
            .unwrap_or_default(),
        opts.local()
            .and_then(|o| o.command.clone())
            .unwrap_or_default(),
    );

    match eff.engine {
        TtsEngine::Local => first_set(&[&local, &tts.command]).to_string(),
        TtsEngine::OpenAI => format!("OpenAI ({})", first_set(&[&openai, &tts.openai.model])),
        TtsEngine::ElevenLabs => format!(
            "ElevenLabs ({})",
            first_set(&[&elevenlabs, &tts.elevenlabs.model_id])
        ),
        TtsEngine::Cartesia => format!(
            "Cartesia ({})",
            first_set(&[&cartesia, &tts.cartesia.model_id])
        ),
        TtsEngine::Http => format!("HTTP ({})", first_set(&[&http, &tts.http.url_template])),
        TtsEngine::Google => format!("Google ({})", first_set(&[&google, &tts.google.model])),
        TtsEngine::Microsoft => format!(
            "Microsoft ({})",
            first_set(&[&microsoft, &tts.microsoft.model])
        ),
        TtsEngine::Edge => format!("Edge-TTS ({})", first_set(&[&edge, &tts.edge.voice])),
        TtsEngine::Kitten => "Kitten TTS".to_string(),
        TtsEngine::Piper => "Piper".to_string(),
        TtsEngine::Kokoro => "Kokoro".to_string(),
        TtsEngine::Pocket => "Pocket".to_string(),
    }
}

#[tauri::command]
pub fn test_tts_engine_config(
    config: State<'_, Mutex<AppConfig>>,
    engine: String,
    preset: Option<String>,
) -> Result<TtsHealthResult, String> {
    let engine = parse_engine(&engine)?;
    let mut tts_config = config.lock().map_err(|e| e.to_string())?.tts.clone();
    if let Some(preset) = preset {
        tts_config.preset = preset;
    }
    let backend = create_backend(&engine, &tts_config);
    // No profile here — this tests an explicit engine against the global config.
    // Those model fields are `skip_serializing`, so guard the blanks; the label
    // reaches a user-facing toast.
    let backend_name = match engine {
        TtsEngine::Local => first_set(&[&tts_config.command]).to_string(),
        TtsEngine::OpenAI => format!("OpenAI ({})", first_set(&[&tts_config.openai.model])),
        TtsEngine::ElevenLabs => format!(
            "ElevenLabs ({})",
            first_set(&[&tts_config.elevenlabs.model_id])
        ),
        TtsEngine::Cartesia => {
            format!("Cartesia ({})", first_set(&[&tts_config.cartesia.model_id]))
        }
        TtsEngine::Http => format!("HTTP ({})", first_set(&[&tts_config.http.url_template])),
        TtsEngine::Google => format!("Google ({})", first_set(&[&tts_config.google.model])),
        TtsEngine::Microsoft => {
            format!("Microsoft ({})", first_set(&[&tts_config.microsoft.model]))
        }
        TtsEngine::Kitten => "Kitten TTS".to_string(),
        TtsEngine::Piper => "Piper".to_string(),
        TtsEngine::Kokoro => "Kokoro".to_string(),
        TtsEngine::Pocket => "Pocket".to_string(),
        TtsEngine::Edge => format!("Edge-TTS ({})", first_set(&[&tts_config.edge.voice])),
    };
    health_result(backend, backend_name)
}

/// Test a local (uv-installed) engine by running a real synthesis through its
/// stable CLI wrapper, the same way `cli.rs` does at runtime. Returns a
/// `TtsHealthResult` so the Engines-page Test button can reuse the cloud-test
/// UI verbatim. Unlike `health_check` (which only probes binary/path presence
/// for local engines), this proves the engine actually produces audio.
///
/// `engine` is the preset id from the Engines page: piper | kokoro | kitten.
#[tauri::command]
pub fn test_local_engine(engine: String) -> Result<TtsHealthResult, String> {
    let spec = match local_engine_spec(&engine) {
        Some(s) => s,
        None => {
            return Ok(TtsHealthResult {
                success: false,
                message: format!("Unknown local engine: {engine}"),
                error_type: Some("unknown".into()),
            });
        }
    };
    let backend = CliTtsBackend::new(spec.command.clone(), spec.args_template.clone());
    let backend_name = format!("Local ({})", engine);

    log::info!(
        "[IPC] test_local_engine '{}' — synthesizing test clip (voice: {})",
        engine,
        spec.voice
    );

    // synthesize() blocks (uv run + python); run it inline like the cloud
    // health checks. First-run engines may download models here, which can be
    // slow — the UI shows a spinner.
    let result = backend.synthesize("Hello.", &spec.voice);

    match result {
        Ok(bytes) => {
            // Validate the bytes look like a real audio file: non-empty and
            // either a WAV (RIFF....) or any non-trivial blob for mp3 engines.
            let looks_ok = bytes.len() > 44
                && (bytes.starts_with(b"RIFF")
                    || bytes.starts_with(&[0x49, 0x44, 0x33]) // ID3 (mp3)
                    || bytes.starts_with(&[0xFF, 0xFB])       // mp3 frame
                    || bytes.starts_with(&[0xFF, 0xF3])
                    || bytes.starts_with(&[0xFF, 0xF2]));
            if looks_ok {
                log::info!(
                    "[IPC] test_local_engine '{}' produced {} bytes — OK",
                    engine,
                    bytes.len()
                );
                Ok(TtsHealthResult {
                    success: true,
                    message: format!(
                        "{} synthesized a test clip successfully ({} bytes).",
                        backend_name,
                        bytes.len()
                    ),
                    error_type: None,
                })
            } else {
                log::warn!(
                    "[IPC] test_local_engine '{}' produced {} bytes — too small or unrecognized",
                    engine,
                    bytes.len()
                );
                Ok(TtsHealthResult {
                    success: false,
                    message: format!(
                        "{} produced no audio ({} bytes). The engine ran but did not generate valid output.",
                        backend_name,
                        bytes.len()
                    ),
                    error_type: Some("unknown".into()),
                })
            }
        }
        Err(e) => {
            log::warn!("[IPC] test_local_engine '{}' failed: {}", engine, e);
            // Reuse the cloud-test error mapping for consistent UI messages.
            synthesize_health_failure(&backend_name, &e)
        }
    }
}

/// Stable per-engine CLI spec, mirroring the profile snippet each installer
/// emits. Kept here (not in catalog.rs) because this is a *test* fixture, not
/// a runtime catalog entry — it only needs to drive one short synthesis.
struct LocalEngineSpec {
    command: String,
    args_template: Vec<String>,
    voice: String,
}

fn local_engine_spec(engine: &str) -> Option<LocalEngineSpec> {
    // Voice is the engine's English default from its installer menu. The
    // {engine_dir} placeholder is resolved by CliTtsBackend::build_args at
    // run time.
    let uv_run = |project: &str, wrapper: &str| {
        vec![
            "run".into(),
            "--project".into(),
            format!("{{engine_dir}}/{project}"),
            "python".into(),
            format!("{{engine_dir}}/{project}/scripts/{wrapper}"),
            "--text-file".into(),
            "{input}".into(),
            "--voice".into(),
            "{voice}".into(),
            "--output".into(),
            "{output}".into(),
        ]
    };
    let spec = match engine {
        "piper" => LocalEngineSpec {
            command: "uv".into(),
            args_template: uv_run("piper", "copyspeak-piper.py"),
            voice: "en_US-amy-medium".into(),
        },
        "kitten" => LocalEngineSpec {
            command: "uv".into(),
            args_template: uv_run("kitten", "copyspeak-kitten.py"),
            voice: "Rosie".into(),
        },
        "kokoro" => LocalEngineSpec {
            command: "uv".into(),
            // The wrapper resolves <engine_dir>/kokoro/models/ relative to
            // itself, so no explicit --model/--voices are needed here.
            args_template: uv_run("kokoro", "copyspeak-kokoro.py"),
            voice: "af_heart".into(),
        },
        "pocket" => LocalEngineSpec {
            command: "uv".into(),
            args_template: uv_run("pocket", "copyspeak-pocket.py"),
            voice: "alba".into(),
        },
        _ => return None,
    };
    Some(spec)
}

// Map a synthesis error to a TtsHealthResult. Localized for local engines:
// surfaces "run the installer" guidance instead of API-key chatter.
fn synthesize_health_failure(backend_name: &str, e: &TtsError) -> Result<TtsHealthResult, String> {
    let (message, error_type) = match e {
        TtsError::Unavailable(msg) => {
            if msg.contains("not found") || msg.contains("not recognized") {
                (
                    format!(
                        "{} not found. Run its installer from the Engines page first.",
                        backend_name
                    ),
                    "not_found",
                )
            } else {
                (
                    format!("{} unavailable: {}", backend_name, msg),
                    "unavailable",
                )
            }
        }
        TtsError::Io(io_err) => {
            if io_err.kind() == std::io::ErrorKind::NotFound {
                (
                    format!(
                        "{} not found. Run its installer from the Engines page first.",
                        backend_name
                    ),
                    "not_found",
                )
            } else {
                (format!("IO error: {}", io_err), "io_error")
            }
        }
        _ => (format!("{} test failed: {}", backend_name, e), "unknown"),
    };
    Ok(TtsHealthResult {
        success: false,
        message,
        error_type: Some(error_type.to_string()),
    })
}

fn health_result(
    backend: Box<dyn crate::tts::TtsBackend>,
    backend_name: String,
) -> Result<TtsHealthResult, String> {
    match backend.health_check() {
        Ok(()) => {
            log::info!("TTS engine health check passed: {}", backend_name);
            Ok(TtsHealthResult {
                success: true,
                message: format!("{} is available and configured correctly", backend_name),
                error_type: None,
            })
        }
        Err(e) => {
            log::warn!("TTS engine health check failed: {}", e);
            let (message, error_type) = match &e {
                TtsError::Unavailable(msg) => {
                    if msg.contains("API key") {
                        (
                            format!("{} - API key is missing or invalid", backend_name),
                            "api_key_missing",
                        )
                    } else if msg.contains("not found") || msg.contains("The system cannot find") {
                        (format!("Command '{}' not found. Please ensure the TTS engine is installed and in PATH.", backend_name), "not_found")
                    } else if msg.contains("Access is denied") || msg.contains("permission") {
                        (
                            format!(
                                "Permission denied accessing '{}'. Check permissions.",
                                backend_name
                            ),
                            "permission_denied",
                        )
                    } else {
                        (
                            format!("{} unavailable: {}", backend_name, msg),
                            "unavailable",
                        )
                    }
                }
                TtsError::Http(msg) => {
                    if msg.contains("401") || msg.contains("403") {
                        (
                            format!(
                                "{} - Authentication failed. Check your API key.",
                                backend_name
                            ),
                            "auth_failed",
                        )
                    } else if msg.contains("429") {
                        (
                            format!(
                                "{} - Rate limit exceeded. Please try again later.",
                                backend_name
                            ),
                            "rate_limit",
                        )
                    } else {
                        (
                            format!("{} - Network error: {}", backend_name, msg),
                            "http_error",
                        )
                    }
                }
                TtsError::Io(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        (format!("Command '{}' not found. Please ensure the TTS engine is installed.", backend_name), "not_found")
                    } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                        (
                            format!(
                                "Permission denied running '{}'. Check file permissions.",
                                backend_name
                            ),
                            "permission_denied",
                        )
                    } else {
                        (format!("IO error: {}", e), "io_error")
                    }
                }
                _ => (format!("TTS engine check failed: {}", e), "unknown"),
            };
            Ok(TtsHealthResult {
                success: false,
                message,
                error_type: Some(error_type.to_string()),
            })
        }
    }
}
