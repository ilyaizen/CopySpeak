// LLM post-processing configuration — Handy-style provider registry.
//
// Any OpenAI-compatible endpoint becomes a first-class provider: the active
// entry is selected by `provider_id` from `providers`, its `base_url` hosts
// both POST {base}/chat/completions and GET {base}/models. `prompt` is global
// (one shared template with the ${output} placeholder), not per-provider.
//
// Legacy v0.2.x fields (`api_key`, `model` — the Groq-only era) are folded
// into the matching provider entry by `normalize()` at config load and never
// serialized back once folded.

use serde::{Deserialize, Serialize};

pub const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const ZAI_BASE_URL: &str = "https://api.z.ai/api/paas/v4";
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";
pub const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostProcessProvider {
    pub id: String,
    pub label: String,
    /// OpenAI-compatible API root (no /chat/completions suffix).
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PostProcessConfig {
    pub enabled: bool,
    pub provider_id: String,
    pub prompt: String,
    pub providers: Vec<PostProcessProvider>,
    /// Minimum input length (in characters) before the condenser runs;
    /// shorter text is spoken verbatim. 0 = condense everything.
    #[serde(default = "default_min_chars")]
    pub min_chars: usize,
    /// When true, condensing runs only for requests that explicitly ask for
    /// it (the control server's `condense` field or the CLI `--condense`
    /// flag); when false, every qualifying text is condensed globally.
    #[serde(default = "default_require_request_flag")]
    pub require_request_flag: bool,
    /// HTTP timeout for the condenser request, in seconds. 0 = no timeout.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    // Legacy single-provider fields (Groq-only era). Folded into `providers`
    // by normalize() and blanked; skip-serialized once empty so a folded
    // config no longer carries them on disk.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
}

fn default_provider_id() -> String {
    "groq".to_string()
}

fn default_min_chars() -> usize {
    0
}

fn default_require_request_flag() -> bool {
    false
}

fn default_timeout_secs() -> u64 {
    5
}

fn default_providers() -> Vec<PostProcessProvider> {
    vec![
        PostProcessProvider {
            id: "groq".to_string(),
            label: "Groq".to_string(),
            base_url: GROQ_BASE_URL.to_string(),
            api_key: String::new(),
            model: "openai/gpt-oss-20b".to_string(),
        },
        PostProcessProvider {
            id: "openai".to_string(),
            label: "OpenAI".to_string(),
            base_url: OPENAI_BASE_URL.to_string(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
        },
        PostProcessProvider {
            id: "zai".to_string(),
            label: "Z.AI".to_string(),
            base_url: ZAI_BASE_URL.to_string(),
            api_key: String::new(),
            model: "glm-4.6".to_string(),
        },
        PostProcessProvider {
            id: "openrouter".to_string(),
            label: "OpenRouter".to_string(),
            base_url: OPENROUTER_BASE_URL.to_string(),
            api_key: String::new(),
            model: "openai/gpt-4o-mini".to_string(),
        },
        PostProcessProvider {
            id: "ollama".to_string(),
            label: "Ollama (local)".to_string(),
            base_url: OLLAMA_BASE_URL.to_string(),
            api_key: String::new(),
            model: "llama3.1".to_string(),
        },
        PostProcessProvider {
            id: "cerebras".to_string(),
            label: "Cerebras".to_string(),
            base_url: CEREBRAS_BASE_URL.to_string(),
            api_key: String::new(),
            model: "llama-3.3-70b".to_string(),
        },
    ]
}

/// The Dramatize prompt: tag text for engines that interpret `[...]` emote
/// tags instead of speaking them. The ${output} placeholder is required.
pub const DEFAULT_POST_PROCESS_PROMPT: &str = "You add expressive audio tags to text for a speech synthesizer. Reply with ONLY the tagged text.\nRules:\n- Never add, remove, or change a word. You may insert tags, CAPITALIZE a word for emphasis, or add ?, ! or ellipses.\n- A tag is a short phrase in [square brackets], placed right before the words it affects, or right after for a reaction such as [sighs].\n- Tags describe the speaker's voice only: emotion, delivery, or a vocal sound. Never music, sound effects, scenery, or actions.\n- Use tags sparingly: at most about one per sentence, and none for flat, factual, or code-like text.\n- Leave code, URLs, numbers, and names untouched. Keep the text's language; tags stay in English.\n- Do not use <break> or SSML.\nTags to use: [happy] [sad] [excited] [angry] [whispers] [thoughtful] [surprised] [laughs] [chuckles] [sighs] [clears throat] [short pause] [long pause]. Similar tags are fine, e.g. [low, steady voice].\nExample: \"Are you serious? I can't believe you did that!\" becomes \"[appalled] Are you serious? [sighs] I can't believe you did that!\"\nText:\n${output}";

/// Prompts that were defaults of past versions: the shipped caveman default
/// and the never-wired dead-file presets. A stored prompt equal to any of
/// these (or empty) is stale default, not user content — `normalize()`
/// upgrades it to the current default on every load. Custom prompts are
/// never touched.
pub const LEGACY_DEFAULT_PROMPTS: &[&str] = &[
    "Rewrite text terse like smart caveman for software developer listening. All technical substance stay. Only fluff die.\n\nRules:\n- Drop articles, filler words, pleasantries, hedging, repetition, and boilerplate.\n- Keep technical facts, names, numbers, code identifiers, commands, and original language exact.\n- Max 3 bullets/points. No framing or commentary. Output only rewritten text.\n\nPattern: [thing] [action] [reason]. [next step].\n\nText:\n${output}",
    "Compress aggressively into terse caveman-style developer notes. Keep technical facts, names, paths, and commands exact. Prefer short sentences over lists unless the input is a list. Return only the compressed text.",
    "Rewrite the text to be shorter, clearer, and easier for a developer to understand. Preserve meaning. Return only the rewritten text, not bullets unless the input is already a list.",
    "Summarize the text for a developer in 1-3 concise sentences. Preserve key decisions, requirements, and action items. Return only the summary.",
];

fn default_post_process_prompt() -> String {
    DEFAULT_POST_PROCESS_PROMPT.to_string()
}

impl PostProcessConfig {
    /// Fold legacy single-provider fields into the provider registry and
    /// repair inconsistent state. Idempotent; safe to call on every load.
    pub fn normalize(&mut self) {
        // Stale defaults (empty, or any past version's default) upgrade to
        // the current default. Runs on every load, never behind a schema
        // gate: the old default can also sit in a profile created after the
        // last migration ran. Custom prompts are never rewritten.
        if self.prompt.trim().is_empty() || LEGACY_DEFAULT_PROMPTS.contains(&self.prompt.as_str()) {
            self.prompt = default_post_process_prompt();
        }

        // Legacy Groq key/model → the `groq` entry (or the first entry when
        // groq was removed). Unconditional: the outer gate clears the legacy
        // fields, so a second run is a no-op — user edits made AFTER a fold
        // live in the entry, never in the legacy fields.
        if !self.api_key.trim().is_empty() || !self.model.trim().is_empty() {
            let has_groq = self.providers.iter().any(|p| p.id == "groq");
            let target = if has_groq {
                self.providers.iter_mut().find(|p| p.id == "groq")
            } else {
                self.providers.first_mut()
            };
            if let Some(entry) = target {
                if !self.api_key.trim().is_empty() {
                    entry.api_key = std::mem::take(&mut self.api_key);
                }
                if !self.model.trim().is_empty() {
                    entry.model = std::mem::take(&mut self.model);
                }
            }
            self.api_key.clear();
            self.model.clear();
        }

        // Dangling selection (provider deleted, or an id from an edited
        // import) falls back to the first entry.
        if self.providers.iter().all(|p| p.id != self.provider_id) {
            if let Some(first) = self.providers.first() {
                self.provider_id = first.id.clone();
            }
        }
    }

    /// The active provider entry, or `None` when the registry is empty.
    pub fn active_provider(&self) -> Option<&PostProcessProvider> {
        self.providers
            .iter()
            .find(|p| p.id == self.provider_id)
            .or_else(|| self.providers.first())
    }
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_id: default_provider_id(),
            prompt: default_post_process_prompt(),
            providers: default_providers(),
            min_chars: default_min_chars(),
            require_request_flag: default_require_request_flag(),
            timeout_secs: default_timeout_secs(),
            api_key: String::new(),
            model: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_seeds_builtin_providers() {
        let cfg = PostProcessConfig::default();
        assert_eq!(cfg.provider_id, "groq");
        assert_eq!(cfg.active_provider().map(|p| p.id.as_str()), Some("groq"));
        let ids: Vec<&str> = cfg.providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"zai"));
        assert!(ids.contains(&"openrouter"));
    }

    #[test]
    fn normalize_folds_legacy_groq_credentials_once() {
        let mut cfg = PostProcessConfig {
            api_key: "gsk_legacy".to_string(),
            model: "llama-3.1-8b-instant".to_string(),
            ..Default::default()
        };
        cfg.normalize();
        assert!(cfg.api_key.is_empty());
        assert!(cfg.model.is_empty());
        let groq = cfg.providers.iter().find(|p| p.id == "groq").unwrap();
        assert_eq!(groq.api_key, "gsk_legacy");
        assert_eq!(groq.model, "llama-3.1-8b-instant");

        // Idempotent: second run must not clobber the folded values.
        cfg.normalize();
        let groq = cfg.providers.iter().find(|p| p.id == "groq").unwrap();
        assert_eq!(groq.api_key, "gsk_legacy");
        assert_eq!(groq.model, "llama-3.1-8b-instant");
    }

    #[test]
    fn normalize_does_not_resurrect_from_empty_legacy_fields() {
        let mut cfg = PostProcessConfig::default();
        let groq = cfg.providers.iter_mut().find(|p| p.id == "groq").unwrap();
        groq.api_key = "gsk_current".to_string();
        // Empty legacy fields: normalize must be a no-op for credentials.
        cfg.normalize();
        let groq = cfg.providers.iter().find(|p| p.id == "groq").unwrap();
        assert_eq!(groq.api_key, "gsk_current");
    }

    #[test]
    fn normalize_repairs_dangling_provider_id() {
        let mut cfg = PostProcessConfig {
            provider_id: "deleted-provider".to_string(),
            ..Default::default()
        };
        cfg.normalize();
        assert_eq!(cfg.provider_id, "groq");
    }

    #[test]
    fn normalize_survives_empty_registry() {
        let mut cfg = PostProcessConfig {
            providers: Vec::new(),
            ..Default::default()
        };
        cfg.normalize();
        assert!(cfg.active_provider().is_none());
    }

    #[test]
    fn legacy_json_parses_and_folds() {
        let json = r#"{
            "enabled": true,
            "prompt": "p ${output}",
            "api_key": "gsk_old",
            "model": "openai/gpt-oss-20b"
        }"#;
        let mut cfg: PostProcessConfig = serde_json::from_str(json).unwrap();
        cfg.normalize();
        assert!(cfg.enabled);
        assert_eq!(cfg.provider_id, "groq");
        let groq = cfg.providers.iter().find(|p| p.id == "groq").unwrap();
        assert_eq!(groq.api_key, "gsk_old");
        assert_eq!(groq.model, "openai/gpt-oss-20b");
        // Folded config re-serializes without the legacy fields.
        let out = serde_json::to_string(&cfg).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("api_key").is_none());
        assert!(v.get("model").is_none());
        assert_eq!(v["providers"].as_array().unwrap()[0]["api_key"], "gsk_old");
    }

    #[test]
    fn missing_prompt_field_defaults_from_template() {
        let cfg: PostProcessConfig = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert!(cfg.prompt.contains("${output}"));
        assert_eq!(cfg.providers.len(), 6);
    }

    /// Old configs (written before min_chars/require_request_flag/timeout_secs
    /// existed) must deserialize unchanged to the documented defaults.
    #[test]
    fn legacy_config_gains_gating_defaults() {
        let cfg: PostProcessConfig = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert_eq!(cfg.min_chars, 0);
        assert!(!cfg.require_request_flag);
        assert_eq!(cfg.timeout_secs, 5);
    }

    #[test]
    fn default_prompt_is_the_dramatize_prompt() {
        let prompt = DEFAULT_POST_PROCESS_PROMPT;
        assert!(
            prompt.starts_with("You add expressive audio tags"),
            "{prompt:?}"
        );
        assert!(prompt.contains("${output}"), "placeholder must survive");
        assert!(prompt.contains("[sighs]"), "{prompt:?}");
        assert!(prompt.contains("Do not use <break> or SSML"), "{prompt:?}");
        assert!(!prompt.to_lowercase().contains("caveman"));
    }

    /// Every prompt a config could have carried from an earlier default (the
    /// shipped caveman default, the never-wired dead-file presets, or an
    /// empty string) is upgraded to the Dramatize default. Custom prompts
    /// survive untouched, and the upgrade is idempotent.
    #[test]
    fn normalize_replaces_legacy_and_empty_prompts_with_the_dramatize_default() {
        let legacy_prompts = [
            "",
            "Rewrite text terse like smart caveman for software developer listening. All technical substance stay. Only fluff die.\n\nRules:\n- Drop articles, filler words, pleasantries, hedging, repetition, and boilerplate.\n- Keep technical facts, names, numbers, code identifiers, commands, and original language exact.\n- Max 3 bullets/points. No framing or commentary. Output only rewritten text.\n\nPattern: [thing] [action] [reason]. [next step].\n\nText:\n${output}",
            "Compress aggressively into terse caveman-style developer notes. Keep technical facts, names, paths, and commands exact. Prefer short sentences over lists unless the input is a list. Return only the compressed text.",
            "Rewrite the text to be shorter, clearer, and easier for a developer to understand. Preserve meaning. Return only the rewritten text, not bullets unless the input is already a list.",
            "Summarize the text for a developer in 1-3 concise sentences. Preserve key decisions, requirements, and action items. Return only the summary.",
        ];
        for prompt in legacy_prompts {
            let mut cfg = PostProcessConfig {
                prompt: prompt.to_string(),
                ..Default::default()
            };
            cfg.normalize();
            assert_eq!(cfg.prompt, DEFAULT_POST_PROCESS_PROMPT, "input {prompt:?}");
            cfg.normalize();
            assert_eq!(cfg.prompt, DEFAULT_POST_PROCESS_PROMPT, "idempotence");
        }
    }

    #[test]
    fn normalize_keeps_custom_prompts() {
        let mut cfg = PostProcessConfig {
            prompt: "my own style ${output}".to_string(),
            ..Default::default()
        };
        cfg.normalize();
        assert_eq!(cfg.prompt, "my own style ${output}");
    }

    #[test]
    fn default_min_chars_is_zero() {
        assert_eq!(PostProcessConfig::default().min_chars, 0);
    }

    #[test]
    fn gating_fields_round_trip_and_override() {
        let cfg: PostProcessConfig = serde_json::from_str(
            r#"{ "enabled": true, "min_chars": 0, "require_request_flag": true, "timeout_secs": 0 }"#,
        )
        .unwrap();
        assert_eq!(cfg.min_chars, 0);
        assert!(cfg.require_request_flag);
        assert_eq!(cfg.timeout_secs, 0); // 0 = never (daemon_idle_timeout_secs convention)
    }

    #[test]
    fn active_provider_falls_back_to_first() {
        let cfg = PostProcessConfig {
            provider_id: "nope".to_string(),
            ..Default::default()
        };
        // Without normalize, active_provider() itself still falls back.
        assert_eq!(cfg.active_provider().map(|p| p.id.as_str()), Some("groq"));
    }
}
