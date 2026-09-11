// Native messaging host binary: stdin/stdout framing to Windows named pipe bridge.
// Chrome/Edge launches this process; it forwards protocol messages to the running CopySpeak app.
//
// Both legs run concurrently: the desktop replies to `start` only (hello/state are
// log-only or unsolicited), so lockstep request/response forwarding deadlocks —
// the first `start` would sit unread in stdin while the host waits for a hello
// reply that never comes.
//
// The pipe handle is opened FILE_FLAG_OVERLAPPED: a sync handle allows only ONE
// outstanding I/O per kernel file object, so a parked read made every concurrent
// write block forever (2026-09-09 deadlock, found with tests/harness_deadlock.py).

use copyspeak_browser_host::{read_frame, write_frame, ClientMessage, MAX_FRAME};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;
use windows::Win32::Foundation::{
    ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING,
};
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::INFINITE;

enum HostEvent {
    /// A complete message arrived from the desktop pipe.
    Desktop(Value),
    /// The desktop pipe closed or errored.
    DesktopClosed(String),
    /// Browser closed stdin; exit cleanly.
    BrowserClosed,
    /// Browser-side failure; `message` goes to the worker as an error frame.
    BrowserError(&'static str),
}

#[cfg(windows)]
fn main() {
    // Windows native messaging stdin/stdout are already binary mode.
    run();
}

#[cfg(not(windows))]
fn main() {
    run();
}

fn write_error_frame(writer: &mut impl io::Write, message: &str) {
    let _ = write_frame(writer, &json!({"v":1,"type":"error","message":message}));
}

/// True when a failed I/O call means "started, still pending" (ERROR_IO_PENDING).
fn is_pending(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(997)
}

/// One blocking overlapped READ to completion. Ok(0) = peer disconnected.
unsafe fn overlapped_read(handle: HANDLE) -> std::io::Result<Vec<u8>> {
    let event = windows::Win32::System::Threading::CreateEventW(None, true, false, None)
        .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = event;

    let mut chunk = vec![0u8; MAX_FRAME];
    let mut got: u32 = 0;
    let result = windows::Win32::Storage::FileSystem::ReadFile(
        handle,
        Some(chunk.as_mut_slice()),
        Some(&mut got),
        Some(&mut overlapped),
    );
    if result.is_err() {
        let err = std::io::Error::last_os_error();
        if !is_pending(&err) {
            return Err(err);
        }
    }
    if windows::Win32::System::Threading::WaitForSingleObject(event, INFINITE)
        != WAIT_OBJECT_0
    {
        return Err(std::io::Error::last_os_error());
    }
    let mut moved: u32 = 0;
    if GetOverlappedResult(handle, &overlapped, &mut moved, false).is_err() {
        return Err(std::io::Error::last_os_error());
    }
    chunk.truncate(moved as usize);
    Ok(chunk)
}

/// One blocking overlapped WRITE to completion.
unsafe fn overlapped_write(handle: HANDLE, buf: &[u8]) -> std::io::Result<()> {
    let event = windows::Win32::System::Threading::CreateEventW(None, true, false, None)
        .map_err(|e| std::io::Error::from_raw_os_error(e.code().0))?;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = event;

    let mut written: u32 = 0;
    let result = windows::Win32::Storage::FileSystem::WriteFile(
        handle,
        Some(buf),
        Some(&mut written),
        Some(&mut overlapped),
    );
    if result.is_err() {
        let err = std::io::Error::last_os_error();
        if !is_pending(&err) {
            return Err(err);
        }
    }
    if windows::Win32::System::Threading::WaitForSingleObject(event, INFINITE)
        != WAIT_OBJECT_0
    {
        return Err(std::io::Error::last_os_error());
    }
    let mut moved: u32 = 0;
    if GetOverlappedResult(handle, &overlapped, &mut moved, false).is_err() {
        return Err(std::io::Error::last_os_error());
    }
    if moved as usize != buf.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::WriteZero,
            "Short overlapped write",
        ));
    }
    Ok(())
}

/// Duplex named-pipe handle opened in overlapped mode; safe to share across
/// the two relay threads (one parked read + one write at a time).
struct PipeChannel {
    handle: HANDLE,
    /// Leftover bytes from the last overlapped read. read_frame() requests
    /// 1, then 3, then N bytes; without this buffer each read() would
    /// swallow a whole pipe chunk and silently drop its tail.
    buf: std::sync::Mutex<Vec<u8>>,
}
unsafe impl Send for PipeChannel {}
unsafe impl Sync for PipeChannel {}

impl PipeChannel {
    /// Second owner of the SAME kernel handle (no DuplicateHandle needed:
    /// the I/O legs use independent OVERLAPPED structures, which is the whole
    /// point of the overlapped mode this handle was opened with).
    fn same_handle(&self) -> PipeChannel {
        PipeChannel {
            handle: self.handle,
            buf: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl io::Read for PipeChannel {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut staged = self.buf.lock().unwrap();
        if staged.is_empty() {
            *staged = unsafe { overlapped_read(self.handle) }?;
            if staged.is_empty() {
                return Ok(0);
            }
        }
        let n = staged.len().min(buf.len());
        buf[..n].copy_from_slice(&staged[..n]);
        staged.drain(..n);
        Ok(n)
    }
}

impl io::Write for PipeChannel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe { overlapped_write(self.handle, buf) }?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn open_pipe(pipe_name: &str) -> std::io::Result<PipeChannel> {
    let wide: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
    let name = windows::core::PCWSTR(wide.as_ptr());
    // The desktop serves one client at a time. A new reading started while the
    // previous host is still exiting finds the instance busy; wait for the
    // server to re-listen instead of failing the reading.
    let mut attempts = 0;
    let handle = loop {
        let opened = unsafe {
            CreateFileW(
                name,
                (GENERIC_READ.0 | GENERIC_WRITE.0) as u32,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                None,
            )
        };
        match opened {
            Ok(handle) => break handle,
            Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() && attempts < 5 => {
                attempts += 1;
                // Either outcome retries: a timeout re-checks, a free instance connects.
                let _ = unsafe { WaitNamedPipeW(name, 1000) };
            }
            Err(e) => return Err(std::io::Error::from_raw_os_error(e.code().0)),
        }
    };
    Ok(PipeChannel {
        handle,
        buf: std::sync::Mutex::new(Vec::new()),
    })
}

fn run() {
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    // Pipe name is configured; default matches CopySpeak desktop.
    let pipe_name = std::env::var("COPYSPEAK_BROWSER_PIPE")
        .unwrap_or_else(|_| r"\\.\pipe\copyspeak-browser".to_string());

    // Connect to desktop app. If unavailable, report via stdout and exit.
    let mut pipe = match open_pipe(&pipe_name) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[native-host] Cannot open CopySpeak pipe {}: {}",
                pipe_name, e
            );
            write_error_frame(&mut writer, "Open CopySpeak");
            return;
        }
    };

    let (tx, rx) = mpsc::channel::<HostEvent>();

    // Second owner of the same handle for the read leg. Must be taken BEFORE
    // the stdin thread moves `pipe`.
    let pipe_reads = pipe.same_handle();

    // Browser stdin -> desktop pipe. Forwards immediately; never waits on replies.
    let stdin_tx = tx.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut pipe = pipe;
        loop {
            match read_frame(&mut reader) {
                Ok(Some(message)) => {
                    // Validate protocol before forwarding.
                    if ClientMessage::parse(message.clone()).is_err() {
                        eprintln!("[native-host] Invalid message");
                        let _ = stdin_tx.send(HostEvent::BrowserError("Invalid protocol"));
                        return;
                    }
                    if let Err(e) = forward_to_pipe(&mut pipe, &message) {
                        eprintln!("[native-host] Pipe write failed: {}", e);
                        let _ = stdin_tx.send(HostEvent::BrowserError("CopySpeak pipe error"));
                        return;
                    }
                }
                Ok(None) => {
                    // Browser closed stdin.
                    let _ = stdin_tx.send(HostEvent::BrowserClosed);
                    return;
                }
                Err(e) => {
                    eprintln!("[native-host] Read error: {}", e);
                    let _ = stdin_tx.send(HostEvent::BrowserClosed);
                    return;
                }
            }
        }
    });

    // Desktop pipe -> events. Relays accepted/rejected/state frames the moment
    // they arrive; the desktop also pushes unsolicited state updates.
    let pipe_tx = tx.clone();
    thread::spawn(move || {
        let mut pipe = pipe_reads;
        loop {
            match read_frame(&mut pipe) {
                Ok(Some(response)) => {
                    if pipe_tx.send(HostEvent::Desktop(response)).is_err() {
                        return;
                    }
                }
                Ok(None) => {
                    let _ = pipe_tx.send(HostEvent::DesktopClosed("CopySpeak disconnected".into()));
                    return;
                }
                Err(e) => {
                    let _ = pipe_tx.send(HostEvent::DesktopClosed(format!("Pipe read error: {}", e)));
                    return;
                }
            }
        }
    });
    drop(tx);

    // Desktop pipe -> browser stdout, plus terminal events from both legs.
    // Blocking recv: responses are relayed the moment they arrive.
    while let Ok(event) = rx.recv() {
        match event {
            HostEvent::Desktop(response) => {
                if let Err(e) = write_frame(&mut writer, &response) {
                    eprintln!("[native-host] Stdout write failed: {}", e);
                    return;
                }
            }
            HostEvent::DesktopClosed(reason) => {
                eprintln!("[native-host] {}", reason);
                write_error_frame(&mut writer, &reason);
                return;
            }
            HostEvent::BrowserError(message) => {
                write_error_frame(&mut writer, message);
                return;
            }
            HostEvent::BrowserClosed => return,
        }
    }
}

fn forward_to_pipe(pipe: &mut PipeChannel, message: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(message)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if bytes.is_empty() || bytes.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Frame size out of bounds",
        ));
    }
    // Single write: header+payload together so the desktop's stream parser
    // sees contiguous bytes even when writes coalesce.
    let mut frame = Vec::with_capacity(4 + bytes.len());
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(&bytes);
    pipe.write_all(&frame)
}
