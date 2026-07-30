// Engine installer launcher (streamed).
//
// Spawns the PowerShell installer for a local TTS engine with piped stdio and
// streams stdout/stderr line-by-line as `install-progress` Tauri events. The
// app-driven path no longer opens a detached console; running a script by hand
// still shows its interactive voice menu (no -Voices passed from the app).

use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Serialize;
use tauri::Emitter;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One line of installer output (or the terminal completion marker) pushed to
/// the frontend via the `install-progress` event. `done` + `exit_code` are only
/// set on the final event for a run.
#[derive(Clone, Serialize)]
struct InstallProgress {
    engine: String,
    line: Option<String>,
    done: bool,
    exit_code: Option<i32>,
}

/// Map a CopySpeak engine id to its installer script filename under `scripts/`.
fn installer_script_for(engine: &str) -> Result<&'static str, String> {
    match engine {
        "uv" => Ok("install-uv.ps1"),
        "kitten" | "kittentts" | "kitten-tts" => Ok("install-kittentts.ps1"),
        "piper" => Ok("install-piper.ps1"),
        "kokoro" | "kokoro-tts" => Ok("install-kokoro.ps1"),
        "edge" | "edge-tts" => Ok("install-edge-tts.ps1"),
        other => Err(format!("unknown engine installer: {other}")),
    }
}

/// Resolve `scripts/<name>` from dev (CARGO_MANIFEST_DIR) or exe-relative
/// candidates. Returns the first existing path.
fn resolve_script(filename: &str) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Dev: <repo>/scripts — CARGO_MANIFEST_DIR points at src-tauri.
    // ponytail: baked at compile time; fine for dev/alpha, not for packaged
    // installs. Bundle scripts as Tauri resources when distribution matters.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("..").join("scripts").join(filename));

    // Packaged: alongside the exe, or one dir up (resource dir layouts).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("scripts").join(filename));
            candidates.push(exe_dir.join("..").join("scripts").join(filename));
        }
    }

    candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| format!("installer script not found: {filename}"))
}

/// Launch an engine installer by id, streaming stdout/stderr as
/// `install-progress` events. Returns immediately after spawning; completion
/// is signalled by a terminal event (`done: true`). When `voice` is supplied
/// it is forwarded as repeated `-Voices <id>` args, bypassing the script's
/// interactive menu (uv/edge have no voice concept and omit it).
#[tauri::command]
pub fn install_engine(
    app: tauri::AppHandle,
    engine: String,
    voice: Option<Vec<String>>,
) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, voice);
        return Err("Engine installers are Windows-only.".into());
    }

    #[cfg(target_os = "windows")]
    {
        let filename = installer_script_for(&engine)?;
        let script_path = resolve_script(filename)?;
        log::info!(
            "Launching streamed installer for '{engine}': {}",
            script_path.display()
        );

        let mut cmd = Command::new(which_shell());
        cmd.args([
            "-ExecutionPolicy",
            "Bypass",
            "-NoProfile",
            "-File",
            &script_path.display().to_string(),
        ]);
        if let Some(voices) = voice.as_deref() {
            // Repeated `-Voices id` accumulates into the script's [string[]].
            for v in voices {
                cmd.args(["-Voices", v]);
            }
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to launch installer: {e}"))?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");

        // stderr drains on its own thread; both streams share the event.
        let app_err = app.clone();
        let engine_err = engine.clone();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines().flatten() {
                let _ = app_err.emit(
                    "install-progress",
                    InstallProgress {
                        engine: engine_err.clone(),
                        line: Some(line),
                        done: false,
                        exit_code: None,
                    },
                );
            }
        });

        let app_out = app.clone();
        let engine_out = engine;
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines().flatten() {
                let _ = app_out.emit(
                    "install-progress",
                    InstallProgress {
                        engine: engine_out.clone(),
                        line: Some(line),
                        done: false,
                        exit_code: None,
                    },
                );
            }
            let exit_code = child.wait().ok().and_then(|s| s.code());
            let _ = app_out.emit(
                "install-progress",
                InstallProgress {
                    engine: engine_out,
                    line: None,
                    done: true,
                    exit_code,
                },
            );
        });

        Ok(())
    }
}

/// Resolve the shell: pwsh if available, else Windows PowerShell.
#[cfg(target_os = "windows")]
fn which_shell() -> &'static str {
    if Command::new("pwsh.exe").arg("--version").output().is_ok() {
        "pwsh.exe"
    } else {
        "powershell.exe"
    }
}

/// Engine subdirectory under %LOCALAPPDATA%\CopySpeak\engines\ holding the
/// installer's manifest. None for engines without a voice concept (uv, edge).
fn engine_dir_name(engine: &str) -> Option<&'static str> {
    match engine {
        "kitten" | "kittentts" | "kitten-tts" => Some("kitten"),
        "piper" => Some("piper"),
        "kokoro" | "kokoro-tts" => Some("kokoro"),
        _ => None,
    }
}

/// Read the voice ids the installer recorded in its manifest, so the frontend
/// can pre-check the "add a voice later" dialog. Empty when no manifest exists
/// (engine not installed yet) or the engine has no voice concept.
#[tauri::command]
pub fn installed_voices(engine: String) -> Result<Vec<String>, String> {
    let Some(dir_name) = engine_dir_name(&engine) else {
        return Ok(Vec::new());
    };
    let local_appdata = match std::env::var("LOCALAPPDATA") {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };
    let manifest = PathBuf::from(local_appdata)
        .join("CopySpeak")
        .join("engines")
        .join(dir_name)
        .join("manifest.json");
    let Ok(content) = std::fs::read_to_string(&manifest) else {
        return Ok(Vec::new());
    };
    #[derive(serde::Deserialize)]
    struct Manifest {
        voices_installed: Vec<String>,
    }
    let parsed: Manifest =
        serde_json::from_str(&content).unwrap_or(Manifest { voices_installed: Vec::new() });
    Ok(parsed.voices_installed)
}
