// Voice listing, with the provider voice cache.

use crate::config::AppConfig;
use crate::config::{TtsConfig, TtsEngine};
use crate::tts::catalog::VoiceCatalogEntry;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub fn list_tts_engines() -> Vec<crate::tts::catalog::EngineCatalogEntry> {
    crate::tts::catalog::list_engines()
}

/// Whether this (engine, model) pair interprets `[...]` emote tags instead
/// of speaking them. The Dramatize switch uses it to disable itself with an
/// explanation when the active profile cannot take tags.
#[tauri::command]
pub fn supports_emote_tags(engine: TtsEngine, model: String) -> bool {
    crate::tts::catalog::supports_emote_tags(&engine, &model)
}

/// Fetch an engine's voice list.
///
/// Provider-backed engines (catalog `supports_voice_refresh`) call the
/// provider's API off the main thread and store the result in the voice
/// cache. Provider errors propagate: the picker keeps its last good list and
/// shows the reason, so a failed refresh never looks like a successful one.
/// Every other engine returns its built-in catalog list.
#[tauri::command]
pub async fn list_tts_voices(
    engine: TtsEngine,
    config: State<'_, Mutex<AppConfig>>,
) -> Result<Vec<VoiceCatalogEntry>, String> {
    if !supports_voice_refresh(&engine) {
        return Ok(crate::tts::catalog::list_static_voices(&engine));
    }
    // Clone and release: the lock must not be held across a network call.
    let tts = config.lock().unwrap().tts.clone();
    let account = account_fingerprint(&engine, &tts);
    let fetch_engine = engine.clone();
    let voices =
        tauri::async_runtime::spawn_blocking(move || fetch_provider_voices(&fetch_engine, &tts))
            .await
            .map_err(|e| format!("Voice refresh task failed: {}", e))??;

    if let Err(e) = store_cached_voices(&voice_cache_path(), &engine, account, &voices) {
        log::warn!("[Voices] could not write voice cache: {}", e);
    }
    Ok(voices)
}

/// Last successful provider voice lists, keyed by engine id. Entries fetched
/// with a different API key (another account) are left out.
#[tauri::command]
pub fn get_cached_voices(config: State<'_, Mutex<AppConfig>>) -> HashMap<String, CachedVoiceList> {
    let tts = config.lock().unwrap().tts.clone();
    load_cached_voices(&voice_cache_path(), &tts)
}

#[derive(Debug, Clone, Serialize)]
pub struct CachedVoiceList {
    /// Unix epoch milliseconds of the successful fetch.
    pub fetched_at_ms: i64,
    pub voices: Vec<VoiceCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVoiceList {
    fetched_at_ms: i64,
    /// Fingerprint of the API key the list was fetched with.
    account: u64,
    voices: Vec<VoiceCatalogEntry>,
}

/// Serializes cache read-modify-write so two concurrent refreshes cannot
/// drop each other's entry.
static VOICE_CACHE_LOCK: Mutex<()> = Mutex::new(());

fn voice_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CopySpeak")
        .join("voice-cache.json")
}

fn supports_voice_refresh(engine: &TtsEngine) -> bool {
    crate::tts::catalog::list_engines()
        .iter()
        .any(|entry| &entry.engine == engine && entry.supports_voice_refresh)
}

fn account_key(engine: &TtsEngine, tts: &TtsConfig) -> String {
    let (stored, env_name) = match engine {
        TtsEngine::ElevenLabs => (tts.elevenlabs.api_key.as_str(), "ELEVENLABS_API_KEY"),
        TtsEngine::Cartesia => (tts.cartesia.api_key.as_str(), "CARTESIA_API_KEY"),
        TtsEngine::Fish => (tts.fish.api_key.as_str(), "FISH_API_KEY"),
        TtsEngine::Xai => (tts.xai.api_key.as_str(), "XAI_API_KEY"),
        TtsEngine::Mistral => (tts.mistral.api_key.as_str(), "MISTRAL_API_KEY"),
        _ => return String::new(),
    };
    crate::secrets::resolve(stored, &[env_name])
}

/// ponytail: 64-bit DefaultHasher, same trade-off as the speech cache key. A
/// Rust release that changes the hasher only makes old entries miss once.
fn account_fingerprint(engine: &TtsEngine, tts: &TtsConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    account_key(engine, tts).hash(&mut hasher);
    hasher.finish()
}

fn read_store(path: &Path) -> HashMap<String, StoredVoiceList> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("[Voices] ignoring unreadable voice cache: {}", e);
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

fn store_cached_voices(
    path: &Path,
    engine: &TtsEngine,
    account: u64,
    voices: &[VoiceCatalogEntry],
) -> Result<(), String> {
    let _guard = VOICE_CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut store = read_store(path);
    store.insert(
        super::helpers::engine_str(engine).to_string(),
        StoredVoiceList {
            fetched_at_ms: chrono::Utc::now().timestamp_millis(),
            account,
            voices: voices.to_vec(),
        },
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string(&store).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn load_cached_voices(path: &Path, tts: &TtsConfig) -> HashMap<String, CachedVoiceList> {
    let _guard = VOICE_CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let store = read_store(path);
    crate::tts::catalog::list_engines()
        .into_iter()
        .filter(|entry| entry.supports_voice_refresh)
        .filter_map(|entry| {
            let key = super::helpers::engine_str(&entry.engine);
            let stored = store.get(key)?;
            (stored.account == account_fingerprint(&entry.engine, tts)).then(|| {
                (
                    key.to_string(),
                    CachedVoiceList {
                        fetched_at_ms: stored.fetched_at_ms,
                        voices: stored.voices.clone(),
                    },
                )
            })
        })
        .collect()
}

fn voice_entry(
    id: String,
    label: String,
    language: Option<String>,
    description: Option<String>,
    gender: Option<String>,
    preview_url: Option<String>,
) -> VoiceCatalogEntry {
    VoiceCatalogEntry {
        id,
        label,
        language,
        description,
        gender,
        preview_url,
    }
}

/// Provider voice list for a refreshable engine. Blocking: run it off the
/// main thread.
fn fetch_provider_voices(
    engine: &TtsEngine,
    tts: &TtsConfig,
) -> Result<Vec<VoiceCatalogEntry>, String> {
    let fail = |e: crate::tts::TtsError| format!("Failed to fetch voices: {}", e);
    match engine {
        TtsEngine::ElevenLabs => {
            crate::tts::elevenlabs::ElevenLabsTtsBackend::new(tts.elevenlabs.clone())
                .list_voices()
                .map_err(fail)
                .map(|voices| {
                    voices
                        .into_iter()
                        .map(|voice| {
                            let label = |key: &str| {
                                voice.labels.as_ref().and_then(|labels| {
                                    labels.get(key).and_then(|v| v.as_str().map(str::to_string))
                                })
                            };
                            let language = label("language");
                            let gender = label("gender");
                            voice_entry(
                                voice.voice_id.clone(),
                                voice.name.clone().unwrap_or_else(|| voice.voice_id.clone()),
                                language,
                                voice.description.clone(),
                                gender,
                                voice.preview_url.clone(),
                            )
                        })
                        .collect()
                })
        }
        TtsEngine::Cartesia => crate::tts::cartesia::CartesiaTtsBackend::new(tts.cartesia.clone())
            .list_voices()
            .map_err(fail)
            .map(|voices| {
                voices
                    .into_iter()
                    .map(|v| {
                        let label = v.name.unwrap_or_else(|| "Unnamed voice".into());
                        voice_entry(v.id, label, v.language, v.description, None, None)
                    })
                    .collect()
            }),
        TtsEngine::Fish => crate::tts::fish::FishTtsBackend::new(tts.fish.clone())
            .list_voices()
            .map_err(fail)
            .map(|voices| {
                with_builtin_voices(
                    crate::tts::catalog::list_static_voices(engine),
                    voices
                        .into_iter()
                        .map(|v| voice_entry(v.id, v.name, v.language, None, None, None))
                        .collect(),
                )
            }),
        TtsEngine::Xai => crate::tts::xai::XaiTtsBackend::new(tts.xai.clone())
            .list_voices()
            .map_err(fail)
            .map(|voices| {
                voices
                    .into_iter()
                    .map(|v| voice_entry(v.id, v.name, v.language, None, None, None))
                    .collect()
            }),
        TtsEngine::Mistral => crate::tts::mistral::MistralTtsBackend::new(tts.mistral.clone())
            .list_voices()
            .map_err(fail)
            .map(|voices| {
                voices
                    .into_iter()
                    .map(|v| voice_entry(v.id, v.name, v.language, None, None, None))
                    .collect()
            }),
        _ => Ok(crate::tts::catalog::list_static_voices(engine)),
    }
}

/// Built-in entries first (e.g. Fish's default voice, which is no model and
/// so never in the account list), then the provider's, skipping repeated ids.
fn with_builtin_voices(
    builtin: Vec<VoiceCatalogEntry>,
    fetched: Vec<VoiceCatalogEntry>,
) -> Vec<VoiceCatalogEntry> {
    let mut out = builtin;
    for v in fetched {
        if !out.iter().any(|b| b.id == v.id) {
            out.push(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_voices_lead_and_duplicates_are_dropped() {
        let entry = |id: &str| voice_entry(id.into(), id.into(), None, None, None, None);
        let merged = with_builtin_voices(
            vec![entry("")],
            vec![entry("my-clone"), entry(""), entry("other")],
        );
        let ids: Vec<&str> = merged.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["", "my-clone", "other"]);
    }

    fn temp_cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "copyspeak-voice-cache-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("voice-cache.json")
    }

    fn tts_with_xai_key(key: &str) -> TtsConfig {
        let mut tts = AppConfig::default().tts;
        tts.xai.api_key = key.to_string();
        tts
    }

    fn sample() -> Vec<VoiceCatalogEntry> {
        vec![voice_entry(
            "eve".into(),
            "Eve".into(),
            Some("en".into()),
            None,
            None,
            None,
        )]
    }

    #[test]
    fn refreshable_engines_match_the_catalog_flag() {
        for engine in [
            TtsEngine::ElevenLabs,
            TtsEngine::Cartesia,
            TtsEngine::Fish,
            TtsEngine::Xai,
            TtsEngine::Mistral,
        ] {
            assert!(supports_voice_refresh(&engine), "{:?}", engine);
        }
        assert!(!supports_voice_refresh(&TtsEngine::Kokoro));
        assert!(!supports_voice_refresh(&TtsEngine::OpenAI));
    }

    #[test]
    fn cached_list_round_trips_for_the_same_account() {
        let path = temp_cache("roundtrip");
        let tts = tts_with_xai_key("key-a");
        let account = account_fingerprint(&TtsEngine::Xai, &tts);
        store_cached_voices(&path, &TtsEngine::Xai, account, &sample()).unwrap();

        let cached = load_cached_voices(&path, &tts);
        let xai = cached.get("xai").expect("xai entry");
        assert_eq!(xai.voices.len(), 1);
        assert_eq!(xai.voices[0].label, "Eve");
        assert!(xai.fetched_at_ms > 0);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn cached_list_from_another_api_key_is_hidden() {
        let path = temp_cache("account");
        let old = tts_with_xai_key("key-a");
        let account = account_fingerprint(&TtsEngine::Xai, &old);
        store_cached_voices(&path, &TtsEngine::Xai, account, &sample()).unwrap();

        let cached = load_cached_voices(&path, &tts_with_xai_key("key-b"));
        assert!(!cached.contains_key("xai"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn storing_one_engine_keeps_the_others() {
        let path = temp_cache("merge");
        let mut tts = tts_with_xai_key("key-a");
        tts.mistral.api_key = "key-m".into();
        let xai = account_fingerprint(&TtsEngine::Xai, &tts);
        let mistral = account_fingerprint(&TtsEngine::Mistral, &tts);
        store_cached_voices(&path, &TtsEngine::Xai, xai, &sample()).unwrap();
        store_cached_voices(&path, &TtsEngine::Mistral, mistral, &sample()).unwrap();

        let cached = load_cached_voices(&path, &tts);
        assert!(cached.contains_key("xai") && cached.contains_key("mistral"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn unreadable_cache_file_reads_as_empty() {
        let path = temp_cache("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{not json").unwrap();
        assert!(load_cached_voices(&path, &tts_with_xai_key("key-a")).is_empty());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn provider_failure_is_an_error_not_the_builtin_list() {
        // No key in config; the test process sets no XAI_API_KEY (grep
        // set_var before adding one), so the backend fails before any request.
        let result = fetch_provider_voices(&TtsEngine::Xai, &tts_with_xai_key(""));
        let err = result.expect_err("a missing key must surface, not fall back");
        assert!(err.contains("API key is missing"), "{}", err);
    }
}
