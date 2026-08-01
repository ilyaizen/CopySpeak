// Persistent Piper daemon.
//
// The one-shot CLI path pays `PiperVoice.load()` plus `uv run` startup on every
// utterance. This keeps a single wrapper process alive in `--serve` mode so the
// voice model stays resident in RAM and synthesis is a stdin round-trip.
//
// Every entry point degrades gracefully: `try_synthesize` returns `None` and the
// caller runs the one-shot command, so an engine dir with a pre-daemon wrapper
// keeps working until the user reruns install-piper.ps1.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

struct Daemon {
    /// Command + args the daemon was started with. A mismatch means the user
    /// switched voice or profile, so this daemon is stale.
    key: String,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Daemon {
    fn request(&mut self, text: &str, output: &str) -> Result<(), String> {
        let request = serde_json::json!({ "text": text, "output": output });
        writeln!(self.stdin, "{}", request).map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;

        let mut line = String::new();
        match self.stdout.read_line(&mut line) {
            Ok(0) => return Err("daemon closed its output".into()),
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }

        let reply: serde_json::Value = serde_json::from_str(line.trim())
            .map_err(|e| format!("unparseable reply {:?}: {e}", line.trim()))?;
        if reply["ok"].as_bool() == Some(true) {
            Ok(())
        } else {
            Err(reply["error"]
                .as_str()
                .unwrap_or("unknown daemon error")
                .to_string())
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static DAEMON: OnceLock<Mutex<Option<Daemon>>> = OnceLock::new();
static STARTING: AtomicBool = AtomicBool::new(false);
/// Key whose wrapper could not serve, so we stop paying the startup cost for it.
static UNSUPPORTED: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Daemon>> {
    DAEMON.get_or_init(|| Mutex::new(None))
}

fn unsupported() -> &'static Mutex<Option<String>> {
    UNSUPPORTED.get_or_init(|| Mutex::new(None))
}

fn key_of(command: &str, serve_args: &[String]) -> String {
    format!("{command}\u{1}{}", serve_args.join("\u{1}"))
}

fn output_path() -> String {
    std::env::temp_dir()
        .join("copyspeak_piper_out.wav")
        .to_string_lossy()
        .into_owned()
}

fn start(key: String, command: &str, serve_args: &[String]) -> Result<Daemon, String> {
    log::info!("[Piper] Starting daemon: {} {:?}", command, serve_args);

    #[allow(unused_mut)]
    let mut cmd = Command::new(command);
    cmd.args(serve_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.env("PATH", super::cli::get_expanded_path());
    }

    let mut child = cmd.spawn().map_err(|e| format!("{command}: {e}"))?;
    let stdin = child.stdin.take().ok_or("no stdin pipe")?;
    let mut stdout = BufReader::new(child.stdout.take().ok_or("no stdout pipe")?);

    // Drain stderr so the wrapper never blocks on a full pipe, and so model load
    // failures reach the log.
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                log::debug!("[piper-daemon] {}", line);
            }
        });
    }

    // ponytail: no read timeout — a pre-daemon wrapper rejects `--serve` and
    // exits, which surfaces as EOF. A wedged interpreter would leak this thread
    // and permanently disable the daemon; synthesis still works one-shot.
    let handshake = {
        let mut line = String::new();
        match stdout.read_line(&mut line) {
            Ok(0) => {
                Err("wrapper exited without READY (rerun install-piper.ps1 -Force)".to_string())
            }
            Ok(_) if line.trim() == "READY" => Ok(()),
            Ok(_) => Err(format!("unexpected handshake: {:?}", line.trim())),
            Err(e) => Err(e.to_string()),
        }
    };
    if let Err(e) = handshake {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }

    Ok(Daemon {
        key,
        child,
        stdin,
        stdout,
    })
}

/// Start the daemon in the background unless one is already running (or starting)
/// for this configuration. Safe to call repeatedly.
pub fn prewarm(command: String, serve_args: Vec<String>) {
    let key = key_of(&command, &serve_args);

    let known_bad = unsupported()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_deref()
        == Some(key.as_str());
    if known_bad {
        return;
    }
    let already_running = slot()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .is_some_and(|d| d.key == key);
    if already_running {
        return;
    }
    if STARTING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    std::thread::spawn(move || {
        match start(key.clone(), &command, &serve_args) {
            Ok(daemon) => {
                // Assigning drops (and kills) any daemon left from a prior config.
                *slot().lock().unwrap_or_else(|p| p.into_inner()) = Some(daemon);
                log::info!("[Piper] Daemon ready — voice model resident in RAM");
            }
            Err(e) => {
                log::info!("[Piper] Daemon unavailable ({e}); using one-shot synthesis");
                *unsupported().lock().unwrap_or_else(|p| p.into_inner()) = Some(key);
            }
        }
        STARTING.store(false, Ordering::SeqCst);
    });
}

/// Synthesize through the resident daemon. Returns `None` when the daemon cannot
/// serve this configuration — the caller then runs the one-shot command.
pub fn try_synthesize(command: &str, serve_args: &[String], text: &str) -> Option<Vec<u8>> {
    let key = key_of(command, serve_args);
    let mut guard = slot().lock().unwrap_or_else(|p| p.into_inner());

    if !guard.as_ref().is_some_and(|d| d.key == key) {
        // Nothing running, or it belongs to another voice/profile. Serve this
        // utterance one-shot and warm the right daemon for the next one.
        drop(guard);
        prewarm(command.to_string(), serve_args.to_vec());
        return None;
    }

    let output = output_path();
    let _ = std::fs::remove_file(&output);

    let result = match guard.as_mut() {
        Some(daemon) => daemon.request(text, &output),
        None => return None,
    };
    if let Err(e) = result {
        log::warn!("[Piper] Daemon request failed ({e}); falling back to one-shot");
        *guard = None; // Drop kills the process; the next call re-warms.
        return None;
    }
    drop(guard);

    match std::fs::read(&output) {
        Ok(bytes) if !bytes.is_empty() => {
            let _ = std::fs::remove_file(&output);
            Some(bytes)
        }
        _ => {
            log::warn!("[Piper] Daemon wrote no audio; falling back to one-shot");
            None
        }
    }
}

/// Kill the daemon on app exit. No-op when nothing is running.
pub fn shutdown() {
    *slot().lock().unwrap_or_else(|p| p.into_inner()) = None;
}
