//! Per-app ducking of other programs' audio while CopySpeak speaks.
//!
//! Handy mutes the system master output while recording; here that would
//! silence CopySpeak itself, so only OTHER apps' WASAPI sessions are lowered.
//! Playback lives in the webview, whose audio session belongs to a
//! `msedgewebview2` child process, so the skip set is our whole process tree,
//! not just our PID.
//!
//! The frontend's `playback-started` / `playback-finished` events drive
//! [`duck`] / [`restore`]. All COM work happens on one worker thread. The
//! originals are also written to `duck-state.json`: per-app volumes persist in
//! the Windows mixer, so a crash mid-read would otherwise leave apps quiet
//! until the next launch restores them ([`restore_leftover`]).

#![cfg_attr(not(windows), allow(dead_code))]

use crate::config::{DuckConfig, DuckMode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A session we lowered: its volume before (`original`) and the value we set
/// (`applied`). Restore only when the live volume still equals `applied`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Saved {
    pub id: String,
    pub original: f32,
    pub applied: f32,
}

/// Volume to set for a session whose own level is `original`.
pub fn target_volume(original: f32, cfg: &DuckConfig) -> f32 {
    match cfg.mode {
        DuckMode::Mute => 0.0,
        DuckMode::Lower => {
            (original * f32::from(cfg.level_percent.min(100)) / 100.0).clamp(0.0, 1.0)
        }
    }
}

/// True when something other than us changed the volume since we ducked it.
pub fn user_touched(current: f32, applied: f32) -> bool {
    (current - applied).abs() > 0.02
}

/// `root` plus every process transitively spawned by it, from `(pid, parent)` pairs.
pub fn descendants(root: u32, pairs: &[(u32, u32)]) -> HashSet<u32> {
    let mut set = HashSet::from([root]);
    loop {
        let before = set.len();
        for &(pid, parent) in pairs {
            if set.contains(&parent) {
                set.insert(pid);
            }
        }
        if set.len() == before {
            return set;
        }
    }
}

fn state_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("CopySpeak")
        .join("duck-state.json")
}

fn write_state(saved: &[Saved]) {
    if saved.is_empty() {
        let _ = std::fs::remove_file(state_path());
        return;
    }
    match serde_json::to_string(saved) {
        Ok(json) => {
            if let Err(e) = std::fs::write(state_path(), json) {
                log::warn!("[duck] could not persist duck state: {e}");
            }
        }
        Err(e) => log::warn!("[duck] could not serialize duck state: {e}"),
    }
}

fn read_state() -> Vec<Saved> {
    std::fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(not(windows))]
pub use stub::{duck, restore, restore_blocking, restore_leftover};
#[cfg(windows)]
pub use win::{duck, restore, restore_blocking, restore_leftover};

#[cfg(not(windows))]
mod stub {
    use crate::config::DuckConfig;

    pub fn duck(_cfg: &DuckConfig) {}
    pub fn restore() {}
    pub fn restore_blocking() {}
    pub fn restore_leftover() {}
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use windows::core::Interface;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
        ISimpleAudioVolume, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    /// Rescan cadence while ducked, to catch apps that start audio mid-read.
    const POLL: Duration = Duration::from_secs(1);
    /// Restore even without a `playback-finished` after this long.
    const MAX_DUCK: Duration = Duration::from_secs(10 * 60);

    enum Cmd {
        Duck(DuckConfig),
        Restore(Option<Sender<()>>),
    }

    static TX: OnceLock<Mutex<Sender<Cmd>>> = OnceLock::new();

    fn send(cmd: Cmd) {
        let tx = TX.get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("audio-duck".into())
                .spawn(move || worker(rx))
                .expect("spawn audio-duck thread");
            Mutex::new(tx)
        });
        let _ = tx.lock().unwrap().send(cmd);
    }

    /// Duck other apps per `cfg`. No-op when disabled or already ducked.
    pub fn duck(cfg: &DuckConfig) {
        if cfg.enabled {
            send(Cmd::Duck(cfg.clone()));
        }
    }

    /// Restore ducked apps. Safe to call when nothing is ducked.
    pub fn restore() {
        send(Cmd::Restore(None));
    }

    /// Restore and wait (bounded) for it to finish; for app exit.
    pub fn restore_blocking() {
        let (ack_tx, ack_rx) = mpsc::channel();
        send(Cmd::Restore(Some(ack_tx)));
        let _ = ack_rx.recv_timeout(Duration::from_secs(2));
    }

    /// Undo a duck left behind by a crashed run.
    pub fn restore_leftover() {
        if !state_path().exists() {
            return;
        }
        restore();
    }

    struct Session {
        id: String,
        pid: u32,
        vol: ISimpleAudioVolume,
    }

    fn own_tree() -> HashSet<u32> {
        let mut pairs = Vec::new();
        unsafe {
            if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W {
                    dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                    ..Default::default()
                };
                let mut ok = Process32FirstW(snap, &mut entry).is_ok();
                while ok {
                    pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
                    ok = Process32NextW(snap, &mut entry).is_ok();
                }
                let _ = windows::Win32::Foundation::CloseHandle(snap);
            }
        }
        descendants(std::process::id(), &pairs)
    }

    fn sessions() -> windows::core::Result<Vec<Session>> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let list = manager.GetSessionEnumerator()?;
            let mut out = Vec::new();
            for i in 0..list.GetCount()? {
                let ctl = list.GetSession(i)?;
                let ctl2: IAudioSessionControl2 = ctl.cast()?;
                // S_OK (0) means system sounds; leave the OS's own chimes alone.
                if ctl2.IsSystemSoundsSession().0 == 0 {
                    continue;
                }
                let id_ptr = ctl2.GetSessionInstanceIdentifier()?;
                let id = id_ptr.to_string().unwrap_or_default();
                CoTaskMemFree(Some(id_ptr.0 as *const _));
                out.push(Session {
                    id,
                    pid: ctl2.GetProcessId().unwrap_or(0),
                    vol: ctl.cast()?,
                });
            }
            Ok(out)
        }
    }

    fn get(vol: &ISimpleAudioVolume) -> Option<f32> {
        unsafe { vol.GetMasterVolume().ok() }
    }

    fn set(vol: &ISimpleAudioVolume, v: f32) {
        unsafe {
            let _ = vol.SetMasterVolume(v.clamp(0.0, 1.0), std::ptr::null());
        }
    }

    /// Ramp `vol` from `from` to `to` over `fade_ms`; the end value is always set.
    fn fade(vol: &ISimpleAudioVolume, from: f32, to: f32, fade_ms: u32) {
        const STEP_MS: u32 = 15;
        let steps = fade_ms / STEP_MS;
        for n in 1..=steps {
            set(vol, from + (to - from) * n as f32 / steps as f32);
            std::thread::sleep(Duration::from_millis(u64::from(STEP_MS)));
        }
        set(vol, to);
    }

    fn duck_new(cfg: &DuckConfig, saved: &mut Vec<Saved>) {
        let skip = own_tree();
        let found = match sessions() {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[duck] session enumeration failed: {e}");
                return;
            }
        };
        let mut changed = false;
        for s in found {
            if skip.contains(&s.pid) || saved.iter().any(|x| x.id == s.id) {
                continue;
            }
            let Some(original) = get(&s.vol) else {
                continue;
            };
            let applied = target_volume(original, cfg);
            fade(&s.vol, original, applied, cfg.fade_ms);
            saved.push(Saved {
                id: s.id,
                original,
                applied,
            });
            changed = true;
        }
        if changed {
            write_state(saved);
        }
    }

    fn restore_all(saved: &mut Vec<Saved>, fade_ms: u32) {
        if saved.is_empty() {
            return;
        }
        let live = sessions().unwrap_or_default();
        for entry in saved.drain(..) {
            let Some(s) = live.iter().find(|s| s.id == entry.id) else {
                continue;
            };
            let Some(current) = get(&s.vol) else { continue };
            // The user (or the app) changed it since we ducked: not ours to undo.
            if user_touched(current, entry.applied) {
                continue;
            }
            fade(&s.vol, current, entry.original, fade_ms);
        }
        write_state(&[]);
    }

    fn worker(rx: Receiver<Cmd>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        // A previous run may have died mid-duck.
        let mut saved = read_state();
        let mut cfg = DuckConfig::default();
        let mut since = Instant::now();
        let mut ducked = false;
        if !saved.is_empty() {
            log::info!(
                "[duck] restoring {} session(s) left by a previous run",
                saved.len()
            );
            restore_all(&mut saved, 0);
        }
        loop {
            let msg = if ducked {
                match rx.recv_timeout(POLL) {
                    Ok(m) => Some(m),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => return,
                }
            };
            match msg {
                Some(Cmd::Duck(c)) => {
                    if !ducked {
                        since = Instant::now();
                    }
                    cfg = c;
                    ducked = true;
                    duck_new(&cfg, &mut saved);
                }
                Some(Cmd::Restore(ack)) => {
                    restore_all(&mut saved, cfg.fade_ms);
                    ducked = false;
                    if let Some(ack) = ack {
                        let _ = ack.send(());
                    }
                }
                None => {
                    if since.elapsed() > MAX_DUCK {
                        log::warn!("[duck] no playback-finished after {MAX_DUCK:?}; restoring");
                        restore_all(&mut saved, cfg.fade_ms);
                        ducked = false;
                    } else {
                        duck_new(&cfg, &mut saved);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(mode: DuckMode, level: u8) -> DuckConfig {
        DuckConfig {
            enabled: true,
            mode,
            level_percent: level,
            fade_ms: 0,
        }
    }

    #[test]
    fn lower_scales_the_apps_own_level_and_mute_zeroes_it() {
        assert!((target_volume(0.8, &cfg(DuckMode::Lower, 25)) - 0.2).abs() < 1e-6);
        assert_eq!(target_volume(0.8, &cfg(DuckMode::Mute, 25)), 0.0);
        assert_eq!(target_volume(0.5, &cfg(DuckMode::Lower, 250)), 0.5);
    }

    #[test]
    fn restore_guard_only_yields_to_real_user_changes() {
        assert!(!user_touched(0.2, 0.2));
        assert!(!user_touched(0.21, 0.2));
        assert!(user_touched(0.6, 0.2));
    }

    #[test]
    fn descendants_follow_the_whole_chain() {
        // ours(1) -> webview browser(2) -> audio utility(3); 9 is unrelated.
        let set = descendants(1, &[(3, 2), (2, 1), (9, 8)]);
        assert_eq!(set, HashSet::from([1, 2, 3]));
    }

    #[test]
    fn saved_state_round_trips() {
        let s = vec![Saved {
            id: "a".into(),
            original: 0.7,
            applied: 0.14,
        }];
        let back: Vec<Saved> = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
