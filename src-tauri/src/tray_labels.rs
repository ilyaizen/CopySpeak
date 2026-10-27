// Tray menu labels. The tray is built in Rust, so it reads the same locale
// catalogs the webview bundles, using the locale saved in config.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Wry};

use crate::config::AppConfig;

// Same files the webview registers, so Crowdin keeps one source for both.
const CATALOGS: &[(&str, &str)] = &[
    ("en", include_str!("../../src/lib/locales/en.json")),
    ("ar", include_str!("../../src/lib/locales/ar.json")),
    ("de", include_str!("../../src/lib/locales/de.json")),
    ("es", include_str!("../../src/lib/locales/es.json")),
    ("fr", include_str!("../../src/lib/locales/fr.json")),
    ("he", include_str!("../../src/lib/locales/he.json")),
    ("hi", include_str!("../../src/lib/locales/hi.json")),
    ("ru", include_str!("../../src/lib/locales/ru.json")),
    ("zh", include_str!("../../src/lib/locales/zh.json")),
];

/// Tray items whose text follows the locale and the listening state.
pub struct TrayItems {
    pub open: MenuItem<Wry>,
    pub stop: MenuItem<Wry>,
    pub toggle: MenuItem<Wry>,
    pub speak: MenuItem<Wry>,
    pub settings: MenuItem<Wry>,
    pub quit: MenuItem<Wry>,
}

pub struct TrayLabels {
    pub open: String,
    pub stop: String,
    pub listening: String,
    pub paused: String,
    pub speak_clipboard: String,
    pub settings: String,
    pub quit: String,
}

fn catalog(code: &str) -> Option<Value> {
    CATALOGS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, src)| serde_json::from_str(src).expect("bundled locale catalog is valid JSON"))
}

fn lookup(catalog: &Value, key: &str) -> Option<String> {
    catalog
        .pointer(&format!("/tray/{key}"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Labels for `locale`. An unsupported locale, or a key that locale lacks,
/// falls back to English, as the webview does.
pub fn labels_for(locale: &str) -> TrayLabels {
    let english = catalog("en").expect("en catalog is bundled");
    let localized = catalog(locale);
    let pick = |key: &str| {
        localized
            .as_ref()
            .and_then(|c| lookup(c, key))
            .or_else(|| lookup(&english, key))
            .unwrap_or_else(|| panic!("en.json has no tray.{key}"))
    };
    TrayLabels {
        open: pick("open"),
        stop: pick("stop"),
        listening: pick("listening"),
        paused: pick("paused"),
        speak_clipboard: pick("speakClipboard"),
        settings: pick("settings"),
        quit: pick("quit"),
    }
}

/// Re-apply every tray label from the saved locale and the listening state.
pub fn apply(app: &AppHandle) -> tauri::Result<()> {
    let locale = app
        .state::<Mutex<AppConfig>>()
        .lock()
        .unwrap()
        .general
        .locale
        .clone();
    let listening = app.state::<Arc<AtomicBool>>().load(Ordering::Relaxed);
    let labels = labels_for(&locale);
    let toggle_text = if listening {
        labels.listening
    } else {
        labels.paused
    };
    let items = app.state::<TrayItems>();
    items.open.set_text(labels.open)?;
    items.stop.set_text(labels.stop)?;
    items.toggle.set_text(toggle_text)?;
    items.speak.set_text(labels.speak_clipboard)?;
    items.settings.set_text(labels.settings)?;
    items.quit.set_text(labels.quit)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [&str; 7] = [
        "open",
        "stop",
        "listening",
        "paused",
        "speakClipboard",
        "settings",
        "quit",
    ];

    #[test]
    fn every_bundled_catalog_has_every_tray_label() {
        for (code, src) in CATALOGS {
            let value: Value = serde_json::from_str(src).expect("valid JSON");
            for key in KEYS {
                assert!(lookup(&value, key).is_some(), "{code} has no tray.{key}");
            }
        }
    }

    #[test]
    fn unknown_locale_falls_back_to_english() {
        let fallback = labels_for("xx");
        let english = labels_for("en");
        assert_eq!(fallback.listening, english.listening);
        assert_eq!(fallback.quit, english.quit);
        assert_eq!(english.quit, "Quit");
    }

    #[test]
    fn supported_locale_uses_its_own_catalog() {
        let de = catalog("de").expect("de catalog is bundled");
        assert_eq!(labels_for("de").quit, lookup(&de, "quit").unwrap());
    }
}
