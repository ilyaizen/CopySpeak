// Engine installer launcher (streamed).
//
// Spawns the PowerShell installer for a local TTS engine with piped stdio and
// streams stdout/stderr line-by-line as `install-progress` Tauri events. The
// app-driven path no longer opens a detached console; running a script by hand
// still shows its interactive voice menu (no -Voices passed from the app).

use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use regex::Regex;
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

/// Strip ANSI CSI escape sequences (colors, cursor movement, etc.) from
/// PowerShell output so the frontend renders clean text. Only SGR (color)
/// codes appear in practice, but the regex covers all CSI sequences.
fn strip_ansi(line: &str) -> String {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let re = ANSI_RE.get_or_init(|| Regex::new("\x1b\\[[0-9;]*[a-zA-Z]").unwrap());
    re.replace_all(line, "").to_string()
}

/// Map a CopySpeak engine id to its installer script filename under `scripts/`.
/// `.ps1` on Windows, `.sh` on Linux (the platform's `uninstall-engine` twin
/// lives next to them and is named directly at the uninstall call site).
fn installer_script_for(engine: &str) -> Result<&'static str, String> {
    #[cfg(target_os = "windows")]
    {
        match engine {
            "uv" => Ok("install-uv.ps1"),
            "kitten" | "kittentts" | "kitten-tts" => Ok("install-kittentts.ps1"),
            "qwen" | "qwen3" | "qwen3-tts" => Ok("install-qwen.ps1"),
            "piper" => Ok("install-piper.ps1"),
            "kokoro" | "kokoro-tts" => Ok("install-kokoro.ps1"),
            "pocket" | "pocket-tts" => Ok("install-pocket.ps1"),
            other => Err(format!("unknown engine installer: {other}")),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        match engine {
            "uv" => Ok("install-uv.sh"),
            "kitten" | "kittentts" | "kitten-tts" => Ok("install-kittentts.sh"),
            "qwen" | "qwen3" | "qwen3-tts" => Ok("install-qwen.sh"),
            "piper" => Ok("install-piper.sh"),
            "kokoro" | "kokoro-tts" => Ok("install-kokoro.sh"),
            "pocket" | "pocket-tts" => Ok("install-pocket.sh"),
            other => Err(format!("unknown engine installer: {other}")),
        }
    }
}

/// Resolve `scripts/<name>` from the bundled resource dir (packaged), the repo
/// (dev), or exe-relative fallbacks. Returns the first existing path.
fn resolve_script(app: &tauri::AppHandle, filename: &str) -> Result<PathBuf, String> {
    use tauri::Manager;

    let mut candidates: Vec<PathBuf> = Vec::new();

    // Packaged: tauri.conf.json maps ../scripts/** into <resources>/scripts/.
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("scripts").join(filename));
    }

    // Dev: <repo>/scripts — CARGO_MANIFEST_DIR points at src-tauri.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("..").join("scripts").join(filename));

    // Older/alternate bundle layouts: alongside the exe, or one dir up.
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
        // tauri's resource_dir() is built from a canonicalized exe path, and
        // on Windows std::fs::canonicalize returns \\?\ verbatim paths.
        // PowerShell cannot dot-source or Join-Path those, and $PSScriptRoot
        // inherits the prefix, so strip it once here - every installer, dev
        // and packaged, gets a clean script path.
        .map(|p| {
            let display = p.display().to_string();
            PathBuf::from(display.strip_prefix(r"\\?\").unwrap_or(&display))
        })
}

/// Spawn a PowerShell script with piped stdio and stream stdout/stderr as
/// `install-progress` events tagged with `engine`. Shared by install and
/// uninstall so both drive the same frontend log/progress pipeline.
///
/// Returns as soon as the child is spawned; a terminal event (`done: true`)
/// carries the exit code.
#[cfg(target_os = "windows")]
fn spawn_streamed(
    app: tauri::AppHandle,
    engine: String,
    script_path: &PathBuf,
    extra_args: &[String],
) -> Result<(), String> {
    let mut cmd = Command::new(which_shell());
    cmd.args([
        "-ExecutionPolicy",
        "Bypass",
        "-NoProfile",
        "-NonInteractive",
        "-File",
        &script_path.display().to_string(),
    ]);
    cmd.args(extra_args);
    // stdin is closed below, so any Read-Host would hang. The shared installer
    // lib checks this and takes the default for every prompt.
    cmd.env("COPYSPEAK_NONINTERACTIVE", "1");
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch script: {e}"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    // stderr drains on its own thread; both streams share the event.
    let app_err = app.clone();
    let engine_err = engine.clone();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
        {
            let _ = app_err.emit(
                "install-progress",
                InstallProgress {
                    engine: engine_err.clone(),
                    line: Some(strip_ansi(&line)),
                    done: false,
                    exit_code: None,
                },
            );
        }
    });

    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            let _ = app.emit(
                "install-progress",
                InstallProgress {
                    engine: engine.clone(),
                    line: Some(strip_ansi(&line)),
                    done: false,
                    exit_code: None,
                },
            );
        }
        let exit_code = child.wait().ok().and_then(|s| s.code());
        let _ = app.emit(
            "install-progress",
            InstallProgress {
                engine,
                line: None,
                done: true,
                exit_code,
            },
        );
    });

    Ok(())
}

/// Spawn a bash installer script with piped stdio and stream stdout/stderr as
/// `install-progress` events. Linux twin of the Windows `spawn_streamed` —
/// same event shape, so the frontend pipeline is unchanged.
#[cfg(not(target_os = "windows"))]
fn spawn_streamed(
    app: tauri::AppHandle,
    engine: String,
    script_path: &PathBuf,
    extra_args: &[String],
) -> Result<(), String> {
    let mut cmd = Command::new("bash");
    cmd.arg(&script_path.display().to_string());
    cmd.args(extra_args);
    // stdin closed: any `read` in the script must fail fast. The shared
    // installer lib checks COPYSPEAK_NONINTERACTIVE and takes the default
    // for every prompt instead of blocking on the dead stdin.
    cmd.env("COPYSPEAK_NONINTERACTIVE", "1");
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch script: {e}"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    // stderr drains on its own thread; both streams share the event.
    let app_err = app.clone();
    let engine_err = engine.clone();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
        {
            let _ = app_err.emit(
                "install-progress",
                InstallProgress {
                    engine: engine_err.clone(),
                    line: Some(strip_ansi(&line)),
                    done: false,
                    exit_code: None,
                },
            );
        }
    });

    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            let _ = app.emit(
                "install-progress",
                InstallProgress {
                    engine: engine.clone(),
                    line: Some(strip_ansi(&line)),
                    done: false,
                    exit_code: None,
                },
            );
        }
        let exit_code = child.wait().ok().and_then(|s| s.code());
        let _ = app.emit(
            "install-progress",
            InstallProgress {
                engine,
                line: None,
                done: true,
                exit_code,
            },
        );
    });

    Ok(())
}

/// Launch an engine installer by id, streaming stdout/stderr as
/// `install-progress` events. Returns immediately after spawning; completion
/// is signalled by a terminal event (`done: true`). When `voice` is supplied
/// it is forwarded as one comma-joined `-Voices <id,id,...>` argument, bypassing the
/// script's interactive menu. `-File` binds that as a single string (and would drop
/// extra space-separated ids), so each installer splits it with `ConvertTo-VoiceIds` (uv/edge have no voice concept and omit it). `cuda` adds
/// the installers' shared `-Cuda` switch.
#[tauri::command]
pub fn install_engine(
    app: tauri::AppHandle,
    engine: String,
    voice: Option<Vec<String>>,
    cuda: bool,
) -> Result<(), String> {
    let filename = installer_script_for(&engine)?;
    let script_path = resolve_script(&app, filename)?;
    log::info!(
        "Launching streamed installer for '{engine}': {}",
        script_path.display()
    );

    let args = installer_args(voice.as_deref(), cuda);

    spawn_streamed(app, engine, &script_path, &args)
}

/// PowerShell's `-File` wants `-Voices a,b -Cuda`. The bash installers take
/// long flags with space-separated repeated values: `--voices a b c --cuda`.
fn installer_args(voices: Option<&[String]>, cuda: bool) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let mut args = match voices {
            Some(voices) if !voices.is_empty() => vec!["-Voices".into(), voices.join(",")],
            _ => Vec::new(),
        };
        if cuda {
            args.push("-Cuda".into());
        }
        args
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut args = match voices {
            Some(voices) if !voices.is_empty() => {
                let mut v = vec!["--voices".to_string()];
                v.extend(voices.iter().cloned());
                v
            }
            _ => Vec::new(),
        };
        if cuda {
            args.push("--cuda".into());
        }
        args
    }
}

/// Uninstall a local engine, streaming progress as `install-progress` events on
/// the same channel as `install_engine` (so the dialog renders both verbatim).
///
/// uv is refused: it is a shared prerequisite for every other local engine, and
/// removing it silently would break them all.
#[tauri::command]
pub fn uninstall_engine(app: tauri::AppHandle, engine: String) -> Result<(), String> {
    let name = canonical_engine(&engine)
        .ok_or_else(|| format!("unknown engine to uninstall: {engine}"))?;
    if name == "uv" {
        return Err(
            "uv is shared by every local engine and cannot be removed from CopySpeak.".into(),
        );
    }
    let uninstaller_script = uninstaller_script_for();
    let script_path = resolve_script(&app, uninstaller_script)?;
    log::info!(
        "Launching streamed uninstaller for '{name}': {}",
        script_path.display()
    );
    let args = vec!["--engine".to_string(), name.to_string()];
    // Tag events with the id the caller used so its dialog state matches.
    spawn_streamed(app, engine, &script_path, &args)
}

/// The uninstaller twin for this platform: `uninstall-engine.ps1` on Windows,
/// `uninstall-engine.sh` on Linux.
fn uninstaller_script_for() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "uninstall-engine.ps1"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "uninstall-engine.sh"
    }
}

/// Normalize the aliases the frontend may send into the canonical engine id
/// used by the uninstaller and the status probe.
fn canonical_engine(engine: &str) -> Option<&'static str> {
    match engine {
        "uv" => Some("uv"),
        "kitten" | "kittentts" | "kitten-tts" => Some("kitten"),
        "qwen" | "qwen3" | "qwen3-tts" => Some("qwen"),
        "piper" => Some("piper"),
        "kokoro" | "kokoro-tts" => Some("kokoro"),
        "edge" | "edge-tts" => Some("edge"),
        "pocket" | "pocket-tts" => Some("pocket"),
        _ => None,
    }
}

/// Resolve the shell: pwsh if available, else Windows PowerShell.
#[cfg(target_os = "windows")]
fn which_shell() -> &'static str {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if Command::new("pwsh.exe")
        .arg("--version")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok()
    {
        "pwsh.exe"
    } else {
        "powershell.exe"
    }
}

/// `~/.local/share` on Linux, `%LOCALAPPDATA%` on Windows — `dirs` resolves
/// both to the platform's user-local data root, so one code path serves
/// `~/.local/share/CopySpeak/engines/<engine>` and
/// `%LOCALAPPDATA%\CopySpeak\engines\<engine>` alike.
fn engine_dir(name: &str) -> Option<PathBuf> {
    Some(
        dirs::data_local_dir()?
            .join("CopySpeak")
            .join("engines")
            .join(name),
    )
}

/// Voice ids the installer recorded in `<engine_dir>/manifest.json`. Empty when
/// the engine is not installed or has no voice concept.
fn manifest_voices(name: &str) -> Vec<String> {
    let Some(path) = engine_dir(name).map(|d| d.join("manifest.json")) else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    #[derive(serde::Deserialize)]
    struct Manifest {
        voices_installed: Vec<String>,
    }
    serde_json::from_str::<Manifest>(&content)
        .map(|m| m.voices_installed)
        .unwrap_or_default()
}

/// Prepend uv's standard binary locations to this process's PATH.
///
/// Mirrors `Add-UvToPath` in scripts/lib/copyspeak-engine-install.ps1. Called
/// once at startup so command probing, health checks, and synthesis subprocesses
/// all see engines installed since the app last launched. Idempotent.
pub fn augment_path_for_local_engines() {
    let mut extra: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        extra.push(PathBuf::from(&home).join(".local").join("bin"));
    }
    #[cfg(target_os = "windows")]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        extra.push(
            PathBuf::from(&local)
                .join("Microsoft")
                .join("WinGet")
                .join("Links"),
        );
        extra.push(PathBuf::from(&local).join("Programs").join("uv"));
    }

    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<PathBuf> = std::env::split_paths(&current).collect();
    let mut added = false;
    for dir in extra {
        if dir.is_dir() && !paths.contains(&dir) {
            log::info!("Adding to PATH for local engines: {}", dir.display());
            paths.insert(0, dir);
            added = true;
        }
    }
    if added {
        if let Ok(joined) = std::env::join_paths(paths) {
            std::env::set_var("PATH", joined);
        }
    }
}

/// Is `bin` resolvable on PATH?
fn on_path(bin: &str) -> bool {
    #[cfg(target_os = "windows")]
    let out = {
        let mut c = Command::new("where");
        c.arg(bin).creation_flags(CREATE_NO_WINDOW).output()
    };
    #[cfg(not(target_os = "windows"))]
    let out = Command::new("which").arg(bin).output();
    out.map(|o| o.status.success()).unwrap_or(false)
}

/// What the frontend needs to decide between Install, Reinstall, and Uninstall.
#[derive(Clone, Serialize)]
pub struct EngineStatus {
    /// The engine is usable right now (binary present and/or models on disk).
    installed: bool,
    /// Voice ids recorded by the installer; empty for engines without one.
    voices: Vec<String>,
}

/// Runtime verdict for the engines page: is the engine installed, and when it
/// last ran, did ONNX execute on CPU or CUDA?
#[derive(Clone, Serialize)]
pub struct EngineRuntimeStatus {
    installed: bool,
    /// "cpu" | "cuda" — resolved from the newest `providers:` line in the app
    /// log. `None` when the engine never ran (no line yet).
    provider: Option<String>,
}

/// Parse the ONNX execution provider from a daemon `providers: [...]` log line.
/// E.g. `providers: ['CPUExecutionProvider']` -> "cpu";
/// `providers: ['CUDAExecutionProvider', 'CPUExecutionProvider']` -> "cuda".
fn provider_verdict(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if lower.contains("cudaexecutionprovider") {
        Some("cuda".into())
    } else if lower.contains("cpuexecutionprovider") {
        Some("cpu".into())
    } else {
        None
    }
}

/// Scan `path` (newest-first across the rotated app logs: rCURRENT then
/// timestamped .log.gz siblings would need gunzip — current logs suffice,
/// a provider verdict older than the current file is stale anyway) for the
/// LAST `providers:` line, returning its CPU/CUDA verdict.
fn latest_provider_from_logs(dir: &std::path::Path) -> Option<String> {
    let current = dir.join("app_rCURRENT.log");
    let content = std::fs::read_to_string(&current).ok()?;
    content
        .lines()
        .filter(|l| l.contains("providers:") && l.contains("[local-daemon]"))
        .filter_map(provider_verdict)
        .next_back()
}

/// Engines-page runtime status for one local engine. Cheap: one
/// `engine_status`-style disk probe + one log-file read.
#[tauri::command]
pub fn engine_runtime_status(engine: String) -> Result<EngineRuntimeStatus, String> {
    let name = canonical_engine(&engine).ok_or_else(|| format!("unknown engine: {engine}"))?;

    let installed = engine_status(engine.clone())?.installed;
    // ONNX engines report their provider via the daemon stderr `providers:`
    // line. Torch-based engines (qwen, pocket) have no such line — the
    // verdict stays None there (CUDA proof for them is the allocator log).
    let provider = if installed && matches!(name, "kokoro" | "kitten") {
        latest_provider_from_logs(&crate::logging::logs_dir())
    } else {
        None
    };

    Ok(EngineRuntimeStatus {
        installed,
        provider,
    })
}

/// Probe whether a local engine is actually installed.
///
/// Each engine is checked the way it is *used*, not the way it was installed —
/// a manifest alone is not proof, so kokoro also requires its model files and
/// the uv-tool engines require their binary on PATH.
#[tauri::command]
pub fn engine_status(engine: String) -> Result<EngineStatus, String> {
    let name = canonical_engine(&engine).ok_or_else(|| format!("unknown engine: {engine}"))?;

    let installed = match name {
        "uv" => on_path("uv"),
        // uv projects: the installer writes the manifest last, so its presence
        // means the package install and wrapper copy both succeeded.
        "kitten" | "qwen" | "piper" | "pocket" => engine_dir(name)
            .map(|d| d.join("manifest.json").exists() && d.join("pyproject.toml").exists())
            .unwrap_or(false),
        // Kokoro refuses to synthesize without these two model files, which the
        // installer prepares separately from the package. The old audio-only
        // ONNX does not satisfy the native-caption wrapper's requirements.
        "kokoro" => engine_dir("kokoro")
            .map(|d| {
                d.join("manifest.json").exists()
                    && d.join("models").join("kokoro-v1.0-duration.onnx").exists()
                    && d.join("models").join("voices-v1.0.bin").exists()
            })
            .unwrap_or(false),
        "edge" => on_path("edge-tts"),
        _ => false,
    };

    Ok(EngineStatus {
        installed,
        voices: if installed {
            manifest_voices(name)
        } else {
            Vec::new()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{installer_args, uninstaller_script_for};
    use regex::Regex;
    use std::path::Path;

    /// The bash installers take long flags with space-separated voice ids;
    /// the PowerShell installers take one comma-joined `-Voices` string.
    #[test]
    fn installer_voices_match_platform_cli() {
        let voices = vec!["af_heart".to_string(), "bf_emma".to_string()];

        #[cfg(target_os = "windows")]
        assert_eq!(
            installer_args(Some(&voices), true),
            ["-Voices", "af_heart,bf_emma", "-Cuda"]
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            installer_args(Some(&voices), true),
            ["--voices", "af_heart", "bf_emma", "--cuda"]
        );
    }

    /// A packaged build only ships what `bundle.resources` lists; an installer
    /// whose wrapper directory is missing there fails at the wrapper copy in
    /// every release while dev runs (repo-relative) never notice. Bundling is
    /// platform-independent, so this pins the Windows `.ps1` names regardless
    /// of which platform's installer the app would actually launch.
    #[test]
    fn every_installer_wrapper_dir_is_bundled() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let conf: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest.join("tauri.conf.json")).unwrap(),
        )
        .unwrap();
        let resources = conf["bundle"]["resources"].as_object().unwrap();
        let wrapper = Regex::new(r#"\$PSScriptRoot "(\w+)/[^"]+\.py""#).unwrap();

        let scripts: &[(&str, &str)] = &[
            ("kitten", "install-kittentts.ps1"),
            ("qwen", "install-qwen.ps1"),
            ("piper", "install-piper.ps1"),
            ("kokoro", "install-kokoro.ps1"),
            ("pocket", "install-pocket.ps1"),
        ];

        for (engine, script) in scripts {
            let src = std::fs::read_to_string(manifest.join("../scripts").join(script)).unwrap();
            let dirs: Vec<String> = wrapper
                .captures_iter(&src)
                .map(|c| c[1].to_string())
                .collect();
            assert!(!dirs.is_empty(), "{script}: no wrapper copy found");
            for dir in dirs {
                let glob = format!("../scripts/{dir}/*.py");
                assert!(
                    resources.contains_key(&glob),
                    "{script} copies from scripts/{dir}/ but tauri.conf.json does not bundle {glob}"
                );
            }
            let _ = engine;
        }
    }

    /// The uninstall command resolves this file from the same resource set as
    /// the installers; it must exist in the repo next to them on this platform.
    #[test]
    fn uninstaller_twin_exists_on_this_platform() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            manifest
                .join("../scripts")
                .join(uninstaller_script_for())
                .exists(),
            "{} missing from scripts/",
            uninstaller_script_for()
        );
    }
}
