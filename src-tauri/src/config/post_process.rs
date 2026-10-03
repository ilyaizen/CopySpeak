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

pub const DEFAULT_POST_PROCESS_PROMPT: &str = "Rewrite text terse like smart caveman for software developer listening. All technical substance stay. Only fluff die.\n\nRules:\n- Drop articles, filler words, pleasantries, hedging, repetition, and boilerplate.\n- Keep technical facts, names, numbers, code identifiers, commands, and original language exact.\n- Max 3 bullets/points. No framing or commentary. Output only rewritten text.\n\nPattern: [thing] [action] [reason]. [next step].\n\nText:\n${output}";

fn default_post_process_prompt() -> String {
    DEFAULT_POST_PROCESS_PROMPT.to_string()
}

impl PostProcessConfig {
    /// Fold legacy single-provider fields into the provider registry and
    /// repair inconsistent state. Idempotent; safe to call on every load.
    pub fn normalize(&mut self) {
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
