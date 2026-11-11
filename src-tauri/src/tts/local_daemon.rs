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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use super::stream::{AudioFormatMeta, ChunkItem, ChunkStream};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Engines that can run a resident wrapper.
pub const DAEMON_ENGINES: [&str; 5] = ["piper", "kitten", "qwen", "kokoro", "pocket"];

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
    fn next_chunk(&mut self) -> Result<Option<ChunkItem>, String> {
        read_item(&mut self.stdout)
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
fn read_item(reader: &mut impl BufRead) -> Result<Option<ChunkItem>, String> {
    let frame = read_json_line(reader)?;
    // Optional metadata must never mask an explicit engine failure.
    if frame["ok"].as_bool() == Some(false) {
        return Err(frame["error"]
            .as_str()
            .unwrap_or("unknown daemon error")
            .to_string());
    }
    if let Some(captions) = frame.get("captions") {
        // Only a caption-only frame can degrade safely. A mixed frame could
        // own binary bytes; ignoring it would desynchronize the pipe.
        if frame.as_object().is_none_or(|object| object.len() != 1) {
            return Err("unexpected mixed caption frame".into());
        }
        let captions =
            serde_json::from_value::<super::captions::CaptionAlignment>(captions.clone())
                .map_err(|e| e.to_string())
                .and_then(|captions| {
                    captions.validate()?;
                    Ok(captions)
                });
        return Ok(Some(match captions {
            Ok(captions) => ChunkItem::Captions(captions),
            Err(error) => {
                log::warn!("Clearing invalid daemon captions: {error}");
                ChunkItem::ClearCaptions
            }
        }));
    }
    if frame["end"].as_bool() == Some(true) {
        return Ok(None);
    }
    let Some(len) = frame["chunk"].as_u64() else {
        return Err(format!("unexpected frame: {frame}"));
    };
    let mut buf = vec![0u8; len as usize];
    reader
        .read_exact(&mut buf)
        .map_err(|e| format!("short read of {len}-byte chunk: {e}"))?;
    Ok(Some(ChunkItem::Pcm(buf)))
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

/// Read the v2 handshake, tolerating stray lines the wrapper leaked to stdout
/// before `READY 2` (third-party print/banner code that ignores the
/// "stdout is a protocol pipe" contract). Blank lines are skipped too — a
/// wrapper that dies before handshaking still surfaces as EOF (`Ok(0)`), which
/// is the case this loop must not mask. Without the skip, one stray banner
/// line ahead of READY disabled the daemon for the whole session.
fn read_handshake(stdout: &mut BufReader<ChildStdout>) -> Result<(), String> {
    let mut line = String::new();
    loop {
        match stdout.read_line(&mut line) {
            // EOF before READY: the wrapper genuinely exited (or was OOM-killed).
            Ok(0) => return check_handshake(""),
            Ok(_) if line.trim().is_empty() || !line.starts_with("READY") => {
                log::debug!(
                    "[LocalDaemon] skipping pre-handshake stdout line: {:?}",
                    line.trim()
                );
                line.clear();
            }
            Ok(_) => return check_handshake(&line),
            Err(e) => return Err(e.to_string()),
        }
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
    /// Key whose start thread is in flight (`busy` above). Lets a request for
    /// the SAME config wait for the resident model instead of forking a
    /// one-shot — on a 4GB GPU the overlap runs two model copies and OOMs.
    pending: Option<String>,
    /// Key whose wrapper could not serve, so we stop paying the startup cost.
    unsupported: Option<String>,
    /// When the daemon last entered the pool (placed or returned). `None`
    /// while checked out (streaming) or while a start is pending, so the
    /// reaper only ever ages daemons that are idle IN the pool.
    idle_since: Option<Instant>,
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
                // Release builds log at info: the resolved ONNX `providers:`
                // line is the engines-page CPU/GPU verdict source, so it must
                // survive the level filter. Everything else stays debug.
                if line.contains("providers:") {
                    log::info!("[local-daemon] {}", line);
                } else {
                    log::debug!("[local-daemon] {}", line);
                }
            }
        });
    }

    // A wedged interpreter never prints READY, so bound the handshake by the
    // model-load budget: the start must fail (and free the slot) instead of
    // leaking this thread and hanging every same-config waiter forever. The
    // reader thread owns the pipe meanwhile; killing the child unblocks its
    // read, and it exits when the receiver is dropped on the timeout arm.
    let budget = model_load_budget();
    let (handshake_tx, handshake_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let handshake = read_handshake(&mut stdout);
        let _ = handshake_tx.send((stdout, handshake));
    });
    let (handshake_stdout, handshake) = match handshake_rx.recv_timeout(budget) {
        Ok(loaded) => loaded,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "model load exceeded its {} s budget without READY 2",
                budget.as_secs()
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("handshake reader stopped before READY".to_string());
        }
    };
    if let Err(e) = handshake {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    let stdout = handshake_stdout;

    Ok(Daemon {
        key,
        child,
        stdin,
        stdout,
    })
}

/// Idle-shutdown timeout for pooled daemons, in seconds. 0 = never reap.
/// Global by design (one knob for every engine); read live from this static
/// so a settings change applies on the next reaper cycle without a restart.
/// `prewarm` keeps it in sync with the loaded/saved config.
static IDLE_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(DEFAULT_IDLE_TIMEOUT_SECS);

/// Default: 10 minutes. Frees the 1-4 GB of VRAM/RAM a resident model holds
/// between reads without cold-starting on every casual pause.
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 600;

/// How often the reaper wakes to check for expired daemons.
const REAP_INTERVAL: Duration = Duration::from_secs(30);

/// Pure expiry decision, kept free of clocks and locks so tests can drive it
/// with fixed instants: a daemon is reaped when it has been idle in the pool
/// for longer than the timeout, and never when the timeout is disabled.
fn idle_expired(idle_since: Option<Instant>, now: Instant, timeout_secs: u64) -> bool {
    match idle_since {
        Some(idle_since) => {
            timeout_secs > 0 && now.duration_since(idle_since) >= Duration::from_secs(timeout_secs)
        }
        None => false,
    }
}

/// Lazily spawn the one reaper thread. Called by `prewarm`, so nothing runs
/// until a local engine actually asks for a daemon; the OnceLock makes every
/// later call a no-op.
fn ensure_reaper() {
    static SPAWNED: OnceLock<()> = OnceLock::new();
    SPAWNED.get_or_init(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(REAP_INTERVAL);
            reap_expired();
        });
    });
}

/// Take every daemon whose slot has idled past the timeout, then kill the
/// processes AFTER releasing the lock — `Daemon::drop` waits on the child, and
/// holding the mutex across that would stall every synthesis path.
fn reap_expired() {
    let timeout_secs = IDLE_TIMEOUT_SECS.load(Ordering::Relaxed);
    if timeout_secs == 0 {
        return;
    }
    let now = Instant::now();
    let expired: Vec<(String, Daemon)> = {
        let mut guard = slots();
        let mut expired = Vec::new();
        for (engine, slot) in guard.iter_mut() {
            if slot.daemon.is_some()
                && !slot.busy
                && idle_expired(slot.idle_since, now, timeout_secs)
            {
                if let Some(daemon) = slot.daemon.take() {
                    slot.idle_since = None;
                    expired.push((engine.clone(), daemon));
                }
            }
        }
        expired
    };
    for (engine, daemon) in expired {
        log::info!(
            "[LocalDaemon] {engine} idle {}s — unloading model to free memory",
            timeout_secs
        );
        drop(daemon);
    }
}

/// Publish the idle timeout from config. Called at startup (config load) and
/// from the set_config path so a settings change applies without a restart.
pub fn set_idle_timeout(secs: u64) {
    IDLE_TIMEOUT_SECS.store(secs, Ordering::Relaxed);
}

/// Budget for one wrapper model load, in seconds. `READY 2` only prints after
/// the load, so the handshake read and the same-config pending wait share it:
/// a wedged interpreter must fail the start instead of leaking a busy slot
/// and hanging every waiter forever. The slowest measured load is ~103 s
/// (qwen 0.6B, CPU, bench 2026-11-11). Atomic so tests can shorten it.
static MODEL_LOAD_BUDGET_SECS: AtomicU64 = AtomicU64::new(DEFAULT_MODEL_LOAD_BUDGET_SECS);

const DEFAULT_MODEL_LOAD_BUDGET_SECS: u64 = 180;

fn model_load_budget() -> Duration {
    Duration::from_secs(MODEL_LOAD_BUDGET_SECS.load(Ordering::Relaxed))
}

/// Start the daemon in the background unless one is already running (or starting,
/// or streaming) for this configuration. Safe to call repeatedly.
///
/// A replacement load (the key changed) retires the old-config daemon first,
/// and takes every OTHER engine's idle daemon with it: a profile switch must
/// not run two model copies at once, and must not keep the previous engine's
/// model resident until the idle reaper fires. Busy slots (mid-stream or
/// mid-load) are left alone — killing their pipe would yank live audio.
pub fn prewarm(engine: &str, command: String, serve_args: Vec<String>) {
    ensure_reaper();
    let key = key_of(&command, &serve_args);
    let engine = engine.to_string();

    // Daemons to kill. Dropped AFTER the lock: `Daemon::drop` waits on the
    // child, and holding the mutex across that would stall every synthesis
    // path (same discipline as `reap_expired`).
    let mut stale: Vec<Daemon> = Vec::new();
    {
        let mut guard = slots();
        {
            let slot = guard.entry(engine.clone()).or_default();
            if slot.unsupported.as_deref() == Some(key.as_str()) {
                return;
            }
            if slot.busy || slot.daemon.as_ref().is_some_and(|d| d.key == key) {
                return;
            }
            // Replacement load: retire the old-config daemon BEFORE the new
            // wrapper spawns. Keeping it until the replacement succeeded ran
            // two model copies at once — a CUDA OOM on a 4 GB GPU once a
            // one-shot for a third config joined during the load.
            if let Some(old) = slot.daemon.take() {
                slot.idle_since = None;
                stale.push(old);
            }
            slot.pending = Some(key.clone());
            slot.busy = true;
        }
        for (other, slot) in guard.iter_mut() {
            if other != &engine && !slot.busy && slot.daemon.is_some() {
                if let Some(old) = slot.daemon.take() {
                    slot.idle_since = None;
                    stale.push(old);
                }
            }
        }
    }
    drop(stale);

    std::thread::spawn(move || {
        let started = start(key.clone(), &command, &serve_args);
        let mut guard = slots();
        let slot = guard.entry(engine.clone()).or_default();
        slot.busy = false;
        slot.pending = None;
        match started {
            Ok(daemon) => {
                // The old-config daemon was retired at prewarm entry, so this
                // assignment never overwrites a live daemon.
                slot.daemon = Some(daemon);
                // Idle clock starts when the model enters the pool.
                slot.idle_since = Some(Instant::now());
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
    slots()
        .get(engine)
        .is_some_and(|s| !s.busy && s.daemon.is_some())
}

/// Block until any in-flight start for this engine settles (daemon ready,
/// start failed, or a different config took over), bounded by the model-load
/// budget: the start thread owns the whole outcome, but a wedged interpreter
/// must degrade this caller to its one-shot instead of hanging it forever.
/// Config changes during the wait flip `pending` to the new key, which no
/// longer matches and falls through to the normal take-or-one-shot path.
fn wait_for_start(engine: &str, key: &str) {
    let budget = model_load_budget();
    let deadline = Instant::now() + budget;
    loop {
        {
            let guard = slots();
            let slot = guard.get(engine);
            let settled = match slot {
                Some(slot) => {
                    slot.pending.as_deref() != Some(key)
                        || slot.daemon.as_ref().is_some_and(|d| d.key == key)
                }
                None => true,
            };
            if settled {
                return;
            }
        }
        if Instant::now() >= deadline {
            log::warn!(
                "[LocalDaemon] {engine} start still pending after {} s; using one-shot",
                budget.as_secs()
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
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
        // A start thread for this exact config is still loading the model.
        // Forking a one-shot here runs two model copies at once — a guaranteed
        // CUDA OOM on a 4GB GPU — so wait for the load to settle, then take the
        // freshly-ready daemon (or degrade below if the start failed).
        if slot.pending.as_deref() == Some(key.as_str()) {
            log::info!(
                "[LocalDaemon] {engine} daemon is loading for this config; waiting instead of one-shot"
            );
            drop(guard);
            wait_for_start(engine, &key);
            guard = slots();
        }
        let slot = guard.entry(engine.to_string()).or_default();
        let usable = !slot.busy && slot.daemon.as_ref().is_some_and(|d| d.key == key);
        // Take first, mark busy second: an early return must never leave the
        // engine flagged busy with no stream to clear it.
        match usable.then(|| slot.daemon.take()).flatten() {
            Some(daemon) => {
                slot.busy = true;
                // Checked out for the whole stream — not idle in the pool.
                slot.idle_since = None;
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
                Ok(Some(item)) => {
                    if tx.send(item).is_err() {
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
            slot.idle_since = Some(Instant::now()); // idle clock restarts
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

        assert_eq!(
            read_item(&mut reader),
            Ok(Some(ChunkItem::Pcm(vec![1, 2, 3, 4])))
        );
        assert_eq!(
            read_item(&mut reader),
            Ok(Some(ChunkItem::Pcm(vec![b'\n', 9])))
        );
        assert_eq!(
            read_item(&mut reader),
            Ok(None),
            "end frame ends the stream"
        );
    }

    #[test]
    fn caption_snapshot_precedes_its_pcm_without_ending_the_fragment() {
        let wire = b"{\"captions\":{\"text\":\"Hi\",\"words\":[{\"text_start\":0,\"text_end\":2,\"start_ms\":120,\"end_ms\":350}]}}\n{\"chunk\":2}\n\x01\x02{\"end\":true}\n";
        let mut reader = std::io::BufReader::new(&wire[..]);
        let Some(ChunkItem::Captions(captions)) = read_item(&mut reader).unwrap() else {
            panic!("expected native captions")
        };
        assert_eq!(captions.words[0].start_ms, 120.0);
        assert_eq!(read_item(&mut reader), Ok(Some(ChunkItem::Pcm(vec![1, 2]))));
        assert_eq!(read_item(&mut reader), Ok(None));
    }

    #[test]
    fn invalid_caption_frames_clear_metadata_without_consuming_audio_or_next_reply() {
        let valid = b"{\"captions\":{\"text\":\"Hi\",\"words\":[{\"text_start\":0,\"text_end\":2,\"start_ms\":0,\"end_ms\":1}]}}\n";
        let mut wire = valid.to_vec();
        wire.extend_from_slice(b"{\"chunk\":2}\n\x01\x02");
        // A long run must be consumed iteratively, not recursively.
        for _ in 0..4096 {
            wire.extend_from_slice(b"{\"captions\":{}}\n");
        }
        wire.extend_from_slice(b"{\"captions\":{\"text\":\"Hi\",\"words\":[{\"text_start\":0,\"text_end\":9,\"start_ms\":0,\"end_ms\":1}]}}\n");
        wire.extend_from_slice(b"{\"chunk\":2}\n\x03\x04{\"end\":true}\n");
        wire.extend_from_slice(valid);
        wire.extend_from_slice(b"{\"end\":true}\n");
        let mut reader = std::io::BufReader::new(&wire[..]);
        let first = read_item(&mut reader).unwrap();
        assert!(matches!(first, Some(ChunkItem::Captions(_))));
        assert_eq!(read_item(&mut reader), Ok(Some(ChunkItem::Pcm(vec![1, 2]))));
        for _ in 0..4096 {
            assert_eq!(read_item(&mut reader), Ok(Some(ChunkItem::ClearCaptions)));
        }
        assert_eq!(read_item(&mut reader), Ok(Some(ChunkItem::ClearCaptions)));
        assert_eq!(read_item(&mut reader), Ok(Some(ChunkItem::Pcm(vec![3, 4]))));
        assert_eq!(read_item(&mut reader), Ok(None));
        assert_eq!(read_item(&mut reader), Ok(first));
        assert_eq!(read_item(&mut reader), Ok(None));
    }

    #[test]
    fn caption_degradation_does_not_hide_protocol_errors() {
        for suffix in [
            "not json\n",
            "{\"unexpected\":true}\n",
            "{\"ok\":false,\"error\":\"engine failed\",\"captions\":{}}\n",
            "{\"chunk\":8}\nabc",
            "{\"captions\":{},\"chunk\":2}\nxx",
            "",
        ] {
            let wire = format!("{{\"captions\":{{}}}}\n{suffix}");
            let mut reader = std::io::BufReader::new(wire.as_bytes());
            assert_eq!(read_item(&mut reader), Ok(Some(ChunkItem::ClearCaptions)));
            assert!(read_item(&mut reader).is_err());
        }
        let wire = b"{\"captions\":null}\n{\"end\":true}\n";
        let mut reader = std::io::BufReader::new(&wire[..]);
        assert_eq!(read_item(&mut reader), Ok(Some(ChunkItem::ClearCaptions)));
        assert_eq!(read_item(&mut reader), Ok(None));
    }

    #[test]
    fn a_failure_frame_surfaces_its_reason_before_and_after_the_header() {
        let wire = b"{\"ok\":false,\"error\":\"model exploded\"}\n";
        assert_eq!(
            read_header(&mut std::io::BufReader::new(&wire[..])).unwrap_err(),
            "model exploded"
        );
        assert_eq!(
            read_item(&mut std::io::BufReader::new(&wire[..])),
            Err("model exploded".to_string())
        );
    }

    #[test]
    fn a_truncated_chunk_is_an_error_not_a_short_read() {
        // Declares 8 bytes, supplies 3.
        let wire = b"{\"chunk\":8}\n\x01\x02\x03";
        let err = read_item(&mut std::io::BufReader::new(&wire[..])).unwrap_err();
        assert!(err.contains("short read"), "unexpected error: {err}");
    }

    #[test]
    fn only_the_v2_handshake_is_accepted() {
        assert_eq!(check_handshake("READY 2\n"), Ok(()));
        // A v1 wrapper answers with the temp-file protocol, which would hang.
        assert!(check_handshake("READY").unwrap_err().contains("v1"));
        assert!(check_handshake("").unwrap_err().contains("without READY"));
        assert!(check_handshake("Traceback...")
            .unwrap_err()
            .contains("unexpected"));
    }

    #[test]
    fn unknown_engine_is_not_ready_and_shutdown_is_a_noop() {
        // Hold the slots lock: cargo test runs suites as threads of one
        // process, and `shutdown()` clears every engine's slot.
        let _lock = SLOTS_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        assert!(!is_ready("nosuch"));
        shutdown();
        assert!(!is_ready("nosuch"));
    }

    #[test]
    fn idle_beyond_timeout_is_expired() {
        let placed = Instant::now() - Duration::from_secs(700);
        assert!(idle_expired(Some(placed), Instant::now(), 600));
        // Exactly at the boundary counts as expired: at a 30 s scan interval
        // the daemon has then been holding VRAM for the full window already.
        let boundary = Instant::now() - Duration::from_secs(600);
        assert!(idle_expired(Some(boundary), Instant::now(), 600));
    }

    #[test]
    fn idle_under_timeout_survives() {
        let placed = Instant::now() - Duration::from_secs(599);
        assert!(!idle_expired(Some(placed), Instant::now(), 600));
        assert!(!idle_expired(Some(Instant::now()), Instant::now(), 600));
    }

    #[test]
    fn zero_timeout_never_expires() {
        let placed = Instant::now() - Duration::from_secs(86_400);
        assert!(!idle_expired(Some(placed), Instant::now(), 0));
    }

    #[test]
    fn checked_out_slot_has_no_idle_clock_and_never_expires() {
        let placed = Instant::now() - Duration::from_secs(86_400);
        assert!(!idle_expired(None, Instant::now(), 600));
        // Document the pairing the reaper relies on: a checked-out slot is
        // (daemon: Some, busy: true, idle_since: None) — busy alone would also
        // shield it, but the None makes the invariant explicit.
        assert!(!idle_expired(None, placed + Duration::from_secs(1), 600));
    }

    // --- daemon lifecycle (real child processes, fake wrappers) ---
    //
    // These tests drive `prewarm`/`try_stream`/`start` against a fake wrapper:
    // powershell prints `READY 2` after a set delay, then sleeps. The sleep
    // models a wrapper whose model load or resident phase takes time. They
    // share the global SLOTS map with every other test in this binary, so
    // each one holds SLOTS_TEST_LOCK.

    static SLOTS_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Temporarily shortens the model-load budget. Restores the old value on
    /// drop, so a panicking assert cannot leave later tests a 1 s budget.
    struct BudgetGuard(u64);

    impl BudgetGuard {
        fn set(secs: u64) -> Self {
            BudgetGuard(MODEL_LOAD_BUDGET_SECS.swap(secs, Ordering::Relaxed))
        }
    }

    impl Drop for BudgetGuard {
        fn drop(&mut self) {
            MODEL_LOAD_BUDGET_SECS.store(self.0, Ordering::Relaxed);
        }
    }

    /// A fake v2 wrapper on Windows: prints `READY 2` after `ready_delay_ms`,
    /// then holds the pipes open for `hold_secs` (the resident-model phase).
    #[cfg(windows)]
    fn wrapper_args(ready_delay_ms: u64, hold_secs: u64) -> Vec<String> {
        vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            format!(
                "Start-Sleep -Milliseconds {ready_delay_ms}; 'READY 2'; Start-Sleep -Seconds {hold_secs}"
            ),
        ]
    }

    /// Poll `pred` until it holds, or panic naming `what` we waited for.
    #[cfg(windows)]
    fn wait_until(pred: impl Fn() -> bool, timeout: Duration, what: &str) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if pred() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("timed out waiting for {what}");
    }

    /// The two-model-copies overlap, reproduced. While a replacement load for a
    /// new config runs, the old resident daemon must already be gone — not
    /// parked in the slot until the replacement succeeds. Stage 1 also kills
    /// another engine's idle daemon at that moment: a profile switch must not
    /// keep the previous engine's model until the 600 s reaper fires.
    #[cfg(windows)]
    #[test]
    fn replacement_load_must_not_keep_the_old_model_or_other_engines_resident() {
        let _lock = SLOTS_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let engine = "tts-red-lifecycle";
        let other = "tts-red-lifecycle-other";
        slots().remove(engine);
        slots().remove(other);
        // Resident: a second engine's daemon, idle in its pool.
        prewarm(other, "powershell".into(), wrapper_args(150, 30));
        // Resident: this engine's daemon for config K1 (instant READY).
        prewarm(engine, "powershell".into(), wrapper_args(150, 30));
        wait_until(
            || {
                slots()
                    .get(other)
                    .is_some_and(|s| s.daemon.is_some() && !s.busy)
            },
            Duration::from_secs(15),
            "other engine's daemon ready",
        );
        wait_until(
            || {
                slots()
                    .get(engine)
                    .is_some_and(|s| s.daemon.is_some() && !s.busy)
            },
            Duration::from_secs(15),
            "first daemon ready",
        );

        // Replacement load for config K2: READY delayed 6 s, so the load
        // window is wide enough to sample mid-flight.
        let k2_args = wrapper_args(6000, 30);
        let k2_key = key_of("powershell", &k2_args);
        prewarm(engine, "powershell".into(), k2_args);
        std::thread::sleep(Duration::from_millis(300));

        // A third config asking during the load must degrade to the caller's
        // one-shot WITHOUT hijacking the pending load.
        let third = try_stream(engine, "powershell", &wrapper_args(150, 30), "x");
        assert!(third.is_none(), "a busy load must degrade to one-shot");

        // One lock scope for all three asserts: `slots()` is not reentrant,
        // and a second acquire while `guard` is alive deadlocks the test
        // thread against itself (the wedge the full-suite run died in).
        let guard = slots();
        let slot = guard.get(engine).expect("engine slot exists");
        assert_eq!(
            slot.pending.as_deref(),
            Some(k2_key.as_str()),
            "the third-config request must not hijack the pending load"
        );
        assert!(
            slot.daemon.is_none(),
            "old daemon still resident while its replacement loads — two model copies at once (the 4 GB CUDA OOM overlap)"
        );
        assert!(
            guard
                .get(other)
                .is_some_and(|s| s.daemon.is_none() && !s.busy),
            "another engine's idle daemon must die with the switch, not age out over 600 s"
        );
        drop(guard);

        // The delayed replacement must still land normally after all that.
        wait_until(
            || {
                slots()
                    .get(engine)
                    .is_some_and(|s| !s.busy && s.daemon.as_ref().is_some_and(|d| d.key == k2_key))
            },
            Duration::from_secs(25),
            "replacement daemon ready",
        );

        slots().remove(engine);
        slots().remove(other);
    }

    /// A wedged interpreter never prints READY. The start must fail within the
    /// budget instead of leaving the slot busy and every same-config waiter
    /// hung; the guard keeps this test near one second.
    #[cfg(windows)]
    #[test]
    fn a_wedged_wrapper_fails_the_start_within_the_model_load_budget() {
        // The budget is global: hold the lock so the delayed-READY lifecycle
        // test cannot run under this shortened budget.
        let _lock = SLOTS_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let _budget = BudgetGuard::set(1);
        let started = Instant::now();
        let result = start(
            "tts-budget-wedged".into(),
            "powershell",
            &wrapper_args(30_000, 1),
        );
        let error = result.err().expect("a load past the budget must fail");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the budget must fail the start fast, took {:?}",
            started.elapsed()
        );
        assert!(error.contains("budget"), "error names the budget: {error}");
    }

    /// A caller waiting on an in-flight start must not wait past the budget.
    #[test]
    fn a_same_config_wait_is_bounded_by_the_model_load_budget() {
        let _lock = SLOTS_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let _budget = BudgetGuard::set(1);
        let engine = "tts-budget-wait";
        {
            let mut guard = slots();
            guard.insert(
                engine.to_string(),
                Slot {
                    pending: Some("tts-budget-key".into()),
                    busy: true,
                    ..Slot::default()
                },
            );
        }
        let started = Instant::now();
        wait_for_start(engine, "tts-budget-key");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "a wedged pending start must not hang the caller, took {:?}",
            started.elapsed()
        );
        slots().remove(engine);
    }
}
