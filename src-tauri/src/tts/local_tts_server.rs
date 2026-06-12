use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn set_local_tts_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

fn emit_status(engine: &str, phase: &str, error: Option<&str>) {
    if let Some(app) = APP_HANDLE.get() {
        let payload = serde_json::json!({
            "engine": engine,
            "phase": phase,
            "error": error,
        });
        let _ = app.emit("local-tts-status-changed", payload);
    }
}

#[derive(Clone)]
pub struct ServerHandle {
    pub port: u16,
    pub client: reqwest::blocking::Client,
}

struct ActiveServer {
    child: Mutex<std::process::Child>,
    port: u16,
    engine_name: String,
    client: reqwest::blocking::Client,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartingConfig {
    pub command: String,
    pub engine: String,
    pub script_args: Vec<String>,
}

enum ServerState {
    Stopped,
    Starting {
        _generation: u64,
        config: StartingConfig,
        stderr_tail: Arc<Mutex<Vec<String>>>,
    },
    Ready(Arc<ActiveServer>),
}

struct EngineSlot {
    generation: AtomicU64,
    state: OnceLock<Mutex<ServerState>>,
}

impl EngineSlot {
    const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            state: OnceLock::new(),
        }
    }

    fn generation(&self) -> &AtomicU64 {
        &self.generation
    }

    fn state(&self) -> &Mutex<ServerState> {
        self.state
            .get_or_init(|| Mutex::new(ServerState::Stopped))
    }
}

static KOKORO_SLOT: EngineSlot = EngineSlot::new();
static KITTEN_SLOT: EngineSlot = EngineSlot::new();
static POCKET_SLOT: EngineSlot = EngineSlot::new();

fn slot_for(engine: &str) -> Option<&'static EngineSlot> {
    match engine {
        "kokoro" => Some(&KOKORO_SLOT),
        "kitten" => Some(&KITTEN_SLOT),
        "pocket" => Some(&POCKET_SLOT),
        _ => None,
    }
}

fn get_synth_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .tcp_nodelay(true)
            .connect_timeout(std::time::Duration::from_secs(2))
            .pool_max_idle_per_host(2)
            .build()
            .expect("Failed to build synthesis HTTP client")
    })
}

fn get_free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .ok()
}

fn resolve_python_command(user_command: &str) -> String {
    // If the user's command is already a Python interpreter, use it directly
    let cmd_lower = user_command.to_lowercase();
    if cmd_lower == "py" || cmd_lower.starts_with("python") {
        // But also try to resolve "py" without version if it's just "py"
        return user_command.to_string();
    }

    // For non-Python commands (kokoro-tts, pocket-tts), find a Python interpreter
    for candidate in &["python", "python3", "py"] {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("--version")
            .output()
        {
            if output.status.success() {
                return candidate.to_string();
            }
        }
    }

    // Try py -3 as last resort on Windows
    #[cfg(windows)]
    {
        if let Ok(output) = std::process::Command::new("py")
            .args(["-3", "--version"])
            .output()
        {
            if output.status.success() {
                return "py".to_string();
            }
        }
    }

    // Fallback to the user's original command
    user_command.to_string()
}

fn resolve_server_script(script_name: &str) -> Option<std::path::PathBuf> {
    // During dev: CARGO_MANIFEST_DIR points to src-tauri/
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir.join(script_name);
    if script_path.exists() {
        return Some(script_path);
    }

    // Fallback: relative to exe parent
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(script_name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

fn spawn_start_thread(
    generation: u64,
    slot: &'static EngineSlot,
    command: String,
    engine: String,
    script_args: Vec<String>,
    stderr_tail: Arc<Mutex<Vec<String>>>,
) {
    std::thread::spawn(move || {
        let script_name = match engine.as_str() {
            "kokoro" => "kokoro_server.py",
            "kitten" => "kitten_server.py",
            "pocket" => "pocket_server.py",
            _ => {
                log::warn!("[LocalServer] Unknown engine: {}", engine);
                return;
            }
        };

        let script_path = match resolve_server_script(script_name) {
            Some(p) => p,
            None => {
                log::warn!(
                    "[LocalServer] Server script not found: {} (manifest_dir: {})",
                    script_name,
                    env!("CARGO_MANIFEST_DIR")
                );
                emit_status(&engine, "error", Some("Server script not found"));
                return;
            }
        };

        let port = match get_free_port() {
            Some(p) => p,
            None => {
                log::warn!("[LocalServer] No free port for {}", engine);
                emit_status(&engine, "error", Some("No free port available"));
                return;
            }
        };

        log::info!(
            "[LocalServer] Starting {} server on port {} — script: {}",
            engine,
            port,
            script_path.display()
        );

        let python_cmd = resolve_python_command(&command);
        let mut cmd = Command::new(&python_cmd);
        let mut args = vec![
            script_path.to_string_lossy().to_string(),
            "--port".to_string(),
            port.to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
        ];
        args.extend(script_args);

        cmd.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        emit_status(&engine, "loading", None);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[LocalServer] {} spawn error: {}", engine, e);
                emit_status(&engine, "error", Some(&format!("Spawn error: {}", e)));
                return;
            }
        };

        // Drain stdout in background
        if let Some(stdout) = child.stdout.take() {
            let engine_name = engine.clone();
            std::thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(line) => log::debug!("[{}_server] {}", engine_name, line),
                        Err(_) => break,
                    }
                }
            });
        }

        // Drain stderr to tail buffer
        let stderr_tail_clone = stderr_tail.clone();
        if let Some(stderr) = child.stderr.take() {
            let engine_name = engine.clone();
            std::thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        log::debug!("[{}_server] {}", engine_name, line);
                        let mut buffer =
                            stderr_tail_clone.lock().unwrap_or_else(|p| p.into_inner());
                        buffer.push(line);
                        if buffer.len() > 30 {
                            buffer.remove(0);
                        }
                    } else {
                        break;
                    }
                }
            });
        }

        // Health check poll
        let health_client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(1000))
            .connect_timeout(std::time::Duration::from_millis(500))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                let _ = child.kill();
                log::warn!("[LocalServer] {} health client build failed", engine);
                emit_status(&engine, "error", Some("Health client build failed"));
                return;
            }
        };

        let url = format!("http://127.0.0.1:{}/voices", port);
        let poll_start = std::time::Instant::now();
        let max_secs = 30;
        let mut ready = false;
        let mut poll_delay_ms = 100u64;
        let max_poll_delay_ms = 1600u64;

        while poll_start.elapsed() < std::time::Duration::from_secs(max_secs) {
            if slot.generation().load(Ordering::SeqCst) != generation {
                log::info!(
                    "[LocalServer] {} generation {} superseded. Killing child.",
                    engine,
                    generation
                );
                let _ = child.kill();
                return;
            }

            if let Ok(Some(status)) = child.try_wait() {
                let err_tail = {
                    let buffer = stderr_tail.lock().unwrap_or_else(|p| p.into_inner());
                    buffer.join("\n")
                };
                log::warn!(
                    "[LocalServer] {} server exited prematurely with code {:?}. Stderr tail:\n{}",
                    engine,
                    status.code(),
                    err_tail
                );
                emit_status(&engine, "error", Some("Server exited prematurely"));
                return;
            }

            if let Ok(resp) = health_client.get(&url).send() {
                if resp.status().is_success() {
                    ready = true;
                    break;
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(poll_delay_ms));
            poll_delay_ms = (poll_delay_ms * 2).min(max_poll_delay_ms);
        }

        if !ready {
            let _ = child.kill();
            let err_tail = {
                let buffer = stderr_tail.lock().unwrap_or_else(|p| p.into_inner());
                buffer.join("\n")
            };
            log::warn!(
                "[LocalServer] {} start timed out after {}s. Stderr tail:\n{}",
                engine,
                max_secs,
                err_tail
            );
            emit_status(&engine, "error", Some("Start timed out"));
            return;
        }

        log::info!(
            "[LocalServer] {} server ready on port {} (generation {})",
            engine,
            port,
            generation
        );

        // Warmup synthesis
        let warmup_client = get_synth_client();
        let warmup_url = format!("http://127.0.0.1:{}/", port);
        let warmup_body = serde_json::json!({"text": "Hello", "length_scale": 1.0});
        match warmup_client.post(&warmup_url).json(&warmup_body).send() {
            Ok(_) => {
                log::info!("[LocalServer] {} warmup complete", engine);
            }
            Err(e) => {
                log::warn!(
                    "[LocalServer] {} warmup failed: {}. First synthesis may be slower.",
                    engine,
                    e
                );
            }
        }

        // Transition to Ready
        let mut state = slot.state().lock().unwrap_or_else(|p| p.into_inner());
        if slot.generation().load(Ordering::SeqCst) == generation {
            *state = ServerState::Ready(Arc::new(ActiveServer {
                child: Mutex::new(child),
                port,
                engine_name: engine.clone(),
                client: get_synth_client().clone(),
            }));
            emit_status(&engine, "ready", None);
        } else {
            log::info!(
                "[LocalServer] {} on port {} was superseded during warmup. Killing.",
                engine,
                port
            );
            let _ = child.kill();
        }
    });
}

pub fn ensure_running(
    engine: &str,
    command: String,
    script_args: Vec<String>,
) -> Result<ServerHandle, String> {
    let slot = slot_for(engine).ok_or_else(|| format!("Unknown engine: {}", engine))?;
    let start_wait = std::time::Instant::now();

    loop {
        let mut state = slot.state().lock().unwrap_or_else(|p| p.into_inner());
        match &mut *state {
            ServerState::Ready(server) => {
                let active = server.clone();
                drop(state);

                let is_alive = matches!(
                    active
                        .child
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .try_wait(),
                    Ok(None)
                );

                if is_alive {
                    return Ok(ServerHandle {
                        port: active.port,
                        client: active.client.clone(),
                    });
                } else {
                    let mut state = slot.state().lock().unwrap_or_else(|p| p.into_inner());
                    if let ServerState::Ready(curr) = &*state {
                        if Arc::ptr_eq(curr, &active) {
                            log::info!(
                                "[LocalServer] Killing dead {} server on port {}",
                                engine,
                                active.port
                            );
                            {
                                let mut child = active
                                    .child
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner());
                                let _ = child.kill();
                                let _ = child.wait();
                            }
                            slot.generation().fetch_add(1, Ordering::SeqCst);
                            *state = ServerState::Stopped;
                        }
                    }
                }
            }
            ServerState::Starting {
                _generation: _,
                config: starting_config,
                stderr_tail,
            } => {
                if starting_config.command == command
                    && starting_config.engine == engine
                    && starting_config.script_args == script_args
                {
                    if start_wait.elapsed() > std::time::Duration::from_secs(65) {
                        let err_msg = {
                            let buffer =
                                stderr_tail.lock().unwrap_or_else(|p| p.into_inner());
                            buffer.join("\n")
                        };
                        return Err(format!(
                            "Timeout waiting for {} server to start. Stderr tail:\n{}",
                            engine, err_msg
                        ));
                    }
                    drop(state);
                    std::thread::sleep(std::time::Duration::from_millis(200));
                } else {
                    let new_gen = slot.generation().fetch_add(1, Ordering::SeqCst) + 1;
                    let tail = Arc::new(Mutex::new(Vec::new()));
                    *state = ServerState::Starting {
                        _generation: new_gen,
                        config: StartingConfig {
                            command: command.clone(),
                            engine: engine.to_string(),
                            script_args: script_args.clone(),
                        },
                        stderr_tail: tail.clone(),
                    };
                    drop(state);
                    spawn_start_thread(
                        new_gen,
                        slot,
                        command.clone(),
                        engine.to_string(),
                        script_args.clone(),
                        tail,
                    );
                }
            }
            ServerState::Stopped => {
                let new_gen = slot.generation().fetch_add(1, Ordering::SeqCst) + 1;
                let tail = Arc::new(Mutex::new(Vec::new()));
                *state = ServerState::Starting {
                    _generation: new_gen,
                    config: StartingConfig {
                        command: command.clone(),
                        engine: engine.to_string(),
                        script_args: script_args.clone(),
                    },
                    stderr_tail: tail.clone(),
                };
                drop(state);
                spawn_start_thread(
                    new_gen,
                    slot,
                    command.clone(),
                    engine.to_string(),
                    script_args.clone(),
                    tail,
                );
            }
        }
    }
}

pub fn unload(engine: &str) -> bool {
    let slot = match slot_for(engine) {
        Some(s) => s,
        None => return false,
    };

    let mut state = slot.state().lock().unwrap_or_else(|p| p.into_inner());
    match &*state {
        ServerState::Ready(server) => {
            log::info!(
                "[LocalServer] Unloading {} model on port {}",
                engine,
                server.port
            );
            {
                let mut child = server.child.lock().unwrap_or_else(|p| p.into_inner());
                let _ = child.kill();
                let _ = child.wait();
            }
            *state = ServerState::Stopped;
            emit_status(engine, "stopped", None);
            true
        }
        ServerState::Starting { .. } => {
            log::info!(
                "[LocalServer] Cancelling in-flight {} start via generation bump",
                engine
            );
            slot.generation().fetch_add(1, Ordering::SeqCst);
            *state = ServerState::Stopped;
            emit_status(engine, "stopped", None);
            true
        }
        ServerState::Stopped => false,
    }
}

pub fn prewarm(engine: String, command: String, script_args: Vec<String>) {
    std::thread::spawn(move || {
        let _ = ensure_running(&engine, command, script_args);
    });
}

pub fn unload_all() {
    for engine in &["kokoro", "kitten", "pocket"] {
        unload(engine);
    }
}
