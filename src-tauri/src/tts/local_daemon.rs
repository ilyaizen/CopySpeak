// Persistent daemons for the local (Python-wrapper) TTS engines.
//
// The one-shot CLI path pays model load plus `uv run` startup on every utterance.
// This keeps one wrapper process per engine alive in `--serve` mode so the model
// stays resident in RAM and synthesis is a pipe round-trip.
//
// Every entry point degrades gracefully: `try_stream` returns `None` and the
// caller runs the one-shot command, so an engine dir with a pre-daemon wrapper
// keeps working until the user reruns its installer.
//
// ## Wire protocol (v2)
//
// Handshake: the wrapper prints `READY 2` once its model is loaded. A bare
// `READY` is a v1 (temp-file) wrapper — recorded as unsupported so we stop
// paying its startup cost and stay on the one-shot path.
//
// Request — one JSON line on stdin:
//     {"text": "..."}
//
// Reply — JSON header lines interleaved with raw binary on stdout. Every byte
// count is declared, so text and binary share the pipe safely:
//     {"ok":true,"sample_rate":22050,"channels":1,"bits_per_sample":16}
//     {"chunk":8820}  followed by 8820 bytes of 16-bit LE PCM
//     {"chunk":9600}  followed by 9600 bytes
//     {"end":true}
//
// A failure at any point is a single {"ok":false,"error":"..."} line.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::stream::{AudioFormatMeta, ChunkItem, ChunkStream};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Engines that can run a resident wrapper.
pub const DAEMON_ENGINES: [&str; 4] = ["piper", "kitten", "kokoro", "pocket"];

struct Daemon {
    /// Command + args the daemon was started with. A mismatch means the user
    /// switched voice, device, or profile, so this daemon is stale.
    key: String,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Daemon {
    /// Send one synthesis request and read the format header.
    fn begin(&mut self, text: &str) -> Result<AudioFormatMeta, String> {
        let request = serde_json::json!({ "text": text });
        writeln!(self.stdin, "{}", request).map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        read_header(&mut self.stdout)
    }

    /// Read the next PCM chunk. `Ok(None)` is end-of-stream.
    fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        read_chunk(&mut self.stdout)
    }
}

// The wire protocol is parsed through `BufRead` rather than off the `Daemon`
// itself so it can be exercised against a byte slice — see the tests below.

fn read_json_line(reader: &mut impl BufRead) -> Result<serde_json::Value, String> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => return Err("daemon closed its output".into()),
        Ok(_) => {}
        Err(e) => return Err(e.to_string()),
    }
    serde_json::from_str(line.trim())
        .map_err(|e| format!("unparseable reply {:?}: {e}", line.trim()))
}

/// Read the `{"ok":true,...}` format header that opens every reply.
fn read_header(reader: &mut impl BufRead) -> Result<AudioFormatMeta, String> {
    let header = read_json_line(reader)?;
    if header["ok"].as_bool() != Some(true) {
        return Err(header["error"]
            .as_str()
            .unwrap_or("unknown daemon error")
            .to_string());
    }
    Ok(AudioFormatMeta {
        sample_rate: header["sample_rate"].as_u64().unwrap_or(22050) as u32,
        channels: header["channels"].as_u64().unwrap_or(1) as u16,
        bits_per_sample: header["bits_per_sample"].as_u64().unwrap_or(16) as u16,
    })
}

/// Read one `{"chunk":N}` frame plus its N payload bytes. `Ok(None)` is the
/// `{"end":true}` marker.
fn read_chunk(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let frame = read_json_line(reader)?;
    if frame["end"].as_bool() == Some(true) {
        return Ok(None);
    }
    if frame["ok"].as_bool() == Some(false) {
        return Err(frame["error"]
            .as_str()
            .unwrap_or("unknown daemon error")
            .to_string());
    }
    let Some(len) = frame["chunk"].as_u64() else {
        return Err(format!("unexpected frame: {frame}"));
    };
    let mut buf = vec![0u8; len as usize];
    reader
        .read_exact(&mut buf)
        .map_err(|e| format!("short read of {len}-byte chunk: {e}"))?;
    Ok(Some(buf))
}

/// Whether a handshake line comes from a wrapper speaking protocol v2.
fn check_handshake(line: &str) -> Result<(), String> {
    match line.trim() {
        "READY 2" => Ok(()),
        "READY" => Err("wrapper speaks protocol v1 (rerun its installer -Force)".to_string()),
        "" => Err("wrapper exited without READY (rerun its installer -Force)".to_string()),
        other => Err(format!("unexpected handshake: {other:?}")),
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct Slot {
    daemon: Option<Daemon>,
    /// A start thread is in flight, or a stream currently owns the daemon.
    /// Either way, do not start a second process for this engine.
    busy: bool,
    /// Key whose wrapper could not serve, so we stop paying the startup cost.
    unsupported: Option<String>,
}

static SLOTS: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();

fn slots() -> MutexGuard<'static, HashMap<String, Slot>> {
    SLOTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn key_of(command: &str, serve_args: &[String]) -> String {
    format!("{command}\u{1}{}", serve_args.join("\u{1}"))
}

fn start(key: String, command: &str, serve_args: &[String]) -> Result<Daemon, String> {
    log::info!("[LocalDaemon] Starting: {} {:?}", command, serve_args);

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
    // failures (and the resolved ONNX providers) reach the log.
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                log::debug!("[local-daemon] {}", line);
            }
        });
    }

    // ponytail: no read timeout — a pre-daemon wrapper rejects `--serve` and
    // exits, which surfaces as EOF. A wedged interpreter would leak this thread
    // and permanently disable the daemon; synthesis still works one-shot.
    let handshake = {
        let mut line = String::new();
        match stdout.read_line(&mut line) {
            // EOF and a blank line both mean "no handshake arrived".
            Ok(0) => check_handshake(""),
            Ok(_) => check_handshake(&line),
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

/// Start the daemon in the background unless one is already running (or starting,
/// or streaming) for this configuration. Safe to call repeatedly.
pub fn prewarm(engine: &str, command: String, serve_args: Vec<String>) {
    let key = key_of(&command, &serve_args);
    let engine = engine.to_string();

    {
        let mut guard = slots();
        let slot = guard.entry(engine.clone()).or_default();
        if slot.unsupported.as_deref() == Some(key.as_str()) {
            return;
        }
        if slot.busy || slot.daemon.as_ref().is_some_and(|d| d.key == key) {
            return;
        }
        slot.busy = true;
    }

    std::thread::spawn(move || {
        let started = start(key.clone(), &command, &serve_args);
        let mut guard = slots();
        let slot = guard.entry(engine.clone()).or_default();
        slot.busy = false;
        match started {
            Ok(daemon) => {
                // Assigning drops (and kills) any daemon left from a prior config.
                slot.daemon = Some(daemon);
                log::info!("[LocalDaemon] {engine} ready — model resident in RAM");
            }
            Err(e) => {
                log::info!("[LocalDaemon] {engine} unavailable ({e}); using one-shot synthesis");
                slot.unsupported = Some(key);
            }
        }
    });
}

/// Whether this engine has an idle daemon. Deliberately not keyed on the exact
/// serve args: callers use it to decide whether to *offer* streaming, and a
/// stale key still degrades cleanly through `try_stream`.
pub fn is_ready(engine: &str) -> bool {
    slots().get(engine).is_some_and(|s| !s.busy && s.daemon.is_some())
}

/// Synthesize through the resident daemon, streaming PCM chunks as they arrive.
/// Returns `None` when the daemon cannot serve this configuration — the caller
/// then runs the one-shot command.
pub fn try_stream(
    engine: &str,
    command: &str,
    serve_args: &[String],
    text: &str,
) -> Option<ChunkStream> {
    let key = key_of(command, serve_args);

    // Check the daemon out of the registry for the duration of the stream. Owning
    // it outright (rather than holding the mutex) keeps the lock short and makes
    // two requests interleaving on one pipe impossible.
    let mut daemon = {
        let mut guard = slots();
        let slot = guard.entry(engine.to_string()).or_default();
        let usable = !slot.busy && slot.daemon.as_ref().is_some_and(|d| d.key == key);
        // Take first, mark busy second: an early return must never leave the
        // engine flagged busy with no stream to clear it.
        match usable.then(|| slot.daemon.take()).flatten() {
            Some(daemon) => {
                slot.busy = true;
                daemon
            }
            None => {
                drop(guard);
                // Nothing running, or it belongs to another voice/profile/device.
                // Serve this utterance one-shot and warm the right daemon for next.
                prewarm(engine, command.to_string(), serve_args.to_vec());
                return None;
            }
        }
    };

    let meta = match daemon.begin(text) {
        Ok(meta) => meta,
        Err(e) => {
            log::warn!("[LocalDaemon] {engine} request failed ({e}); falling back to one-shot");
            drop(daemon); // kills the process; the next call re-warms
            slots().entry(engine.to_string()).or_default().busy = false;
            return None;
        }
    };

    let (tx, rx) = mpsc::channel();
    let engine_name = engine.to_string();
    std::thread::spawn(move || {
        let mut healthy = true;
        loop {
            match daemon.next_chunk() {
                Ok(Some(pcm)) => {
                    if tx.send(ChunkItem::Pcm(pcm)).is_err() {
                        // Consumer dropped (abort/stop). The pipe still holds
                        // unread frames, so this daemon cannot be reused.
                        log::info!("[LocalDaemon] {engine_name} stream dropped by consumer");
                        healthy = false;
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    log::warn!("[LocalDaemon] {engine_name} stream failed: {e}");
                    let _ = tx.send(ChunkItem::Failed(e));
                    healthy = false;
                    break;
                }
            }
        }
        drop(tx); // channel close signals end-of-stream

        let mut guard = slots();
        let slot = guard.entry(engine_name).or_default();
        slot.busy = false;
        if healthy {
            slot.daemon = Some(daemon); // back in the pool, model still resident
        }
        // Otherwise `daemon` drops here and is killed; the next call re-warms.
    });

    Some(ChunkStream::new(meta, rx))
}

/// Kill every daemon on app exit. No-op when nothing is running.
pub fn shutdown() {
    slots().clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_distinguishes_command_and_args() {
        let a = key_of("uv", &["--voice".into(), "amy".into()]);
        let b = key_of("uv", &["--voice".into(), "joe".into()]);
        assert_ne!(a, b, "a voice switch must invalidate the daemon");
        assert_eq!(a, key_of("uv", &["--voice".into(), "amy".into()]));
    }

    #[test]
    fn key_is_unambiguous_across_arg_boundaries() {
        // A separator that cannot appear in a path or flag keeps ["a b"] and
        // ["a", "b"] distinct.
        assert_ne!(
            key_of("uv", &["a b".into()]),
            key_of("uv", &["a".into(), "b".into()])
        );
    }

    /// One full reply as a wrapper writes it: header line, two length-prefixed
    /// PCM frames, end marker. Binary payloads sit directly after their frame.
    fn sample_reply() -> Vec<u8> {
        let mut wire = Vec::new();
        wire.extend_from_slice(
            b"{\"ok\":true,\"sample_rate\":24000,\"channels\":1,\"bits_per_sample\":16}\n",
        );
        wire.extend_from_slice(b"{\"chunk\":4}\n");
        wire.extend_from_slice(&[1u8, 2, 3, 4]);
        wire.extend_from_slice(b"{\"chunk\":2}\n");
        // A newline inside the payload must not be mistaken for a frame break.
        wire.extend_from_slice(&[b'\n', 9]);
        wire.extend_from_slice(b"{\"end\":true}\n");
        wire
    }

    #[test]
    fn protocol_v2_round_trips_header_chunks_and_end() {
        let wire = sample_reply();
        let mut reader = std::io::BufReader::new(&wire[..]);

        let meta = read_header(&mut reader).expect("header");
        assert_eq!(meta.sample_rate, 24000);
        assert_eq!(meta.channels, 1);
        assert_eq!(meta.bits_per_sample, 16);

        assert_eq!(read_chunk(&mut reader), Ok(Some(vec![1, 2, 3, 4])));
        assert_eq!(read_chunk(&mut reader), Ok(Some(vec![b'\n', 9])));
        assert_eq!(read_chunk(&mut reader), Ok(None), "end frame ends the stream");
    }

    #[test]
    fn a_failure_frame_surfaces_its_reason_before_and_after_the_header() {
        let wire = b"{\"ok\":false,\"error\":\"model exploded\"}\n";
        assert_eq!(
            read_header(&mut std::io::BufReader::new(&wire[..])).unwrap_err(),
            "model exploded"
        );
        assert_eq!(
            read_chunk(&mut std::io::BufReader::new(&wire[..])),
            Err("model exploded".to_string())
        );
    }

    #[test]
    fn a_truncated_chunk_is_an_error_not_a_short_read() {
        // Declares 8 bytes, supplies 3.
        let wire = b"{\"chunk\":8}\n\x01\x02\x03";
        let err = read_chunk(&mut std::io::BufReader::new(&wire[..])).unwrap_err();
        assert!(err.contains("short read"), "unexpected error: {err}");
    }

    #[test]
    fn only_the_v2_handshake_is_accepted() {
        assert_eq!(check_handshake("READY 2\n"), Ok(()));
        // A v1 wrapper answers with the temp-file protocol, which would hang.
        assert!(check_handshake("READY").unwrap_err().contains("v1"));
        assert!(check_handshake("").unwrap_err().contains("without READY"));
        assert!(check_handshake("Traceback...").unwrap_err().contains("unexpected"));
    }

    #[test]
    fn unknown_engine_is_not_ready_and_shutdown_is_a_noop() {
        assert!(!is_ready("nosuch"));
        shutdown();
        assert!(!is_ready("nosuch"));
    }
}
