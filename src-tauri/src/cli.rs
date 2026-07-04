//! Thin CLI client that drives the running control server.
//!
//! Invoked when the .exe is started with a subcommand argument. Connects to
//! the local control server (default 127.0.0.1:43117); if no server is
//! reachable, auto-launches a GUI instance detached, waits for /health, then
//! runs the requested command.

use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::process::Command;
use std::time::{Duration, Instant};

const DEFAULT_ADDR: &str = "127.0.0.1:43117";
const STARTUP_WAIT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Entry point for CLI mode. Returns the process exit code.
pub fn run(args: &[String]) -> i32 {
    let client = match Client::builder().timeout(REQUEST_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to build HTTP client: {}", e);
            return 1;
        }
    };
    let base = format!(
        "http://{}",
        std::env::var("COPYSPEAK_CONTROL_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string())
    );

    if let Err(e) = ensure_running(&client, &base) {
        eprintln!("{}", e);
        return 1;
    }

    let Some((cmd, rest)) = args.split_first() else {
        eprintln!("{}", usage());
        return 2;
    };

    match dispatch(&client, &base, cmd, rest) {
        Ok(out) => {
            println!("{}", out);
            0
        }
        Err((code, msg)) => {
            eprintln!("{}", msg);
            code
        }
    }
}

/// Make sure the control server answers /health; if not, launch GUI and wait.
fn ensure_running(client: &Client, base: &str) -> Result<(), String> {
    if health_ok(client, base) {
        return Ok(());
    }
    eprintln!("CopySpeak not running — starting it...");
    launch_gui()?;
    let deadline = Instant::now() + STARTUP_WAIT;
    while Instant::now() < deadline {
        if health_ok(client, base) {
            return Ok(());
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err("timed out waiting for CopySpeak to start".into())
}

fn health_ok(client: &Client, base: &str) -> bool {
    client
        .get(format!("{}/health", base))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

fn launch_gui() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: don't tie the GUI's
        // lifetime/console to this short-lived CLI process.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        Command::new(exe)
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|e| format!("failed to launch GUI: {}", e))?;
    }
    #[cfg(not(windows))]
    {
        // ponytail: plain spawn — new session/daemonize only if it ever matters.
        Command::new(exe)
            .spawn()
            .map_err(|e| format!("failed to launch GUI: {}", e))?;
    }
    Ok(())
}

/// Dispatch returns (exit_code, message) on error so callers can surface both.
fn dispatch(
    client: &Client,
    base: &str,
    cmd: &str,
    args: &[String],
) -> Result<String, (i32, String)> {
    match cmd {
        "health" | "ping" => get(client, base, "/health"),
        "profiles" => get(client, base, "/profiles"),
        "profile" => profile_cmd(client, base, args),
        "engines" => get(client, base, "/engines"),
        "voices" => voices_cmd(client, base, args),
        "speak" => speak_cmd(client, base, args),
        "help" | "--help" | "-h" | "/?" => Ok(usage()),
        other => Err((2, format!("unknown command: {}\n\n{}", other, usage()))),
    }
}

fn profile_cmd(client: &Client, base: &str, args: &[String]) -> Result<String, (i32, String)> {
    if args.is_empty() {
        return Err((
            2,
            "usage: copyspeak profile <id> | copyspeak profile --set <id>".into(),
        ));
    }
    if args[0] == "--set" {
        let Some(id) = args.get(1) else {
            return Err((2, "usage: copyspeak profile --set <id>".into()));
        };
        let body = json!({ "profile": id }).to_string();
        post(client, base, "/profiles/active", body)
    } else {
        get(client, base, &format!("/profiles/{}", args[0]))
    }
}

fn voices_cmd(client: &Client, base: &str, args: &[String]) -> Result<String, (i32, String)> {
    let engine = args
        .first()
        .ok_or((2, "usage: copyspeak voices <engine>".to_string()))?;
    get(client, base, &format!("/engines/{}/voices", engine))
}

fn speak_cmd(client: &Client, base: &str, args: &[String]) -> Result<String, (i32, String)> {
    let mut text: Option<String> = None;
    let mut engine: Option<String> = None;
    let mut effect: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut persist = false;

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--engine" => {
                i += 1;
                engine = args.get(i).cloned();
            }
            "--effect" => {
                i += 1;
                effect = args.get(i).cloned();
            }
            "--profile" => {
                i += 1;
                profile = args.get(i).cloned();
            }
            "--persist" => persist = true,
            "--" => {
                // Everything after `--` is the text verbatim.
                let remainder = args[i + 1..].join(" ");
                if !remainder.is_empty() {
                    text = Some(remainder);
                }
                break;
            }
            other => {
                if other.starts_with("--") {
                    return Err((2, format!("unknown option: {}\n\n{}", other, usage())));
                }
                match &text {
                    None => text = Some(other.to_string()),
                    Some(existing) => text = Some(format!("{} {}", existing, other)),
                }
            }
        }
        i += 1;
    }

    let text = text.ok_or((
        2,
        "usage: copyspeak speak \"<text>\" [--profile X] [--engine X] [--effect X] [--persist]"
            .to_string(),
    ))?;

    if engine.is_none() && effect.is_none() && profile.is_none() {
        // ponytail: --persist alone is a no-op; warn rather than silently swallow.
        if persist {
            eprintln!("note: --persist has no effect without --profile/--engine/--effect");
        }
    }

    let body = json!({
        "text": text,
        "engine": engine,
        "effect": effect,
        "profile": profile,
        "persist_selection": persist,
    })
    .to_string();
    post(client, base, "/speak", body)
}

fn get(client: &Client, base: &str, path: &str) -> Result<String, (i32, String)> {
    let resp = client
        .get(format!("{}{}", base, path))
        .send()
        .map_err(|e| (1, format!("request failed: {}", e)))?;
    format_response(resp)
}

fn post(
    client: &Client,
    base: &str,
    path: &str,
    body: String,
) -> Result<String, (i32, String)> {
    let resp = client
        .post(format!("{}{}", base, path))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .map_err(|e| (1, format!("request failed: {}", e)))?;
    format_response(resp)
}

/// Pretty-print JSON bodies; fall back to raw text for non-JSON.
fn format_response(resp: reqwest::blocking::Response) -> Result<String, (i32, String)> {
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err((1, format!("error (HTTP {}): {}", status.as_u16(), body)));
    }
    if let Ok(value) = serde_json::from_str::<Value>(&body) {
        Ok(serde_json::to_string_pretty(&value).unwrap_or(body))
    } else {
        Ok(body)
    }
}

fn usage() -> String {
    "\
CopySpeak CLI — controls the running CopySpeak instance.

Usage: copyspeak <command> [options]

Commands:
  speak \"<text>\" [--profile X] [--engine X] [--effect X] [--persist]
      Speak the given text. Use --profile to pick a preset profile by id.
      --persist also saves engine/effect/profile as the active selection.
  profiles                 List all configured profiles.
  profile <id>             Show details for one profile.
  profile --set <id>       Set the active profile.
  engines                  List supported TTS engines.
  voices <engine>          List voices for an engine.
  health                   Ping the running instance.

If no CopySpeak instance is running, one is launched automatically."
        .to_string()
}
