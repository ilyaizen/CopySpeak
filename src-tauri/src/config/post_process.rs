// LLM post-processing configuration — Handy-style provider registry.
//
// Any OpenAI-compatible endpoint becomes a first-class provider: the active
// entry is selected by `provider_id` from `providers`, its `base_url` hosts
// both POST {base}/chat/completions and GET {base}/models.
//
// Prompt TEMPLATES are a library with exactly one active entry
// (`active_template_id`), mirroring Handy's `LLMPrompt` list + selection.
// Each template carries a class: `preserve` keeps every word and only adds
// delivery tags (Dramatize semantics, needs a tag-capable engine); `rewrite`
// may change words and runs on any engine.
//
// Legacy v0.2.x fields are folded at config load by `normalize()` and never
// serialized back once folded: `api_key`/`model` (the Groq-only era) merge
// into the matching provider entry, and the single global `prompt` folds
// into a template.

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

/// What the LLM may do to the text. The class drives the pipeline: gate,
/// word-check, prompt override, and output sanitization all branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostProcessTemplateClass {
    /// Dramatize semantics: the LLM may insert `[...]` tags, emphasize, and
    /// punctuate, but every spoken word must survive (`words_preserved`),
    /// the engine's tag-dialect `prompt_override` applies, and the engine
    /// must read emote tags.
    Preserve,
    /// Rewrite semantics: words may change, no word-check, works on ANY
    /// engine (the emote-tag gate does not apply). Custom templates
    /// default to this class.
    Rewrite,
}

/// One prompt in the managed library. Builtin ids (`dramatize`, `concise`)
/// are editable and resettable but not deletable; custom ids are deletable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostProcessTemplate {
    pub id: String,
    pub name: String,
    pub class: PostProcessTemplateClass,
    pub prompt: String,
}

pub const DRAMATIZE_TEMPLATE_ID: &str = "dramatize";
pub const CONCISE_TEMPLATE_ID: &str = "concise";

/// The Concise builtin: compress for listening, words may change.
pub const CONCISE_PROMPT: &str = "You compress text so it is fast to listen to. Reply with ONLY the compressed text.\nRules:\n- Keep every fact, name, number, URL, code identifier, and command exact.\n- Drop filler, pleasantries, hedging, and repetition.\n- Prefer short sentences; keep lists as lists.\n- Preserve the text's language. Never add commentary or headers.\nText:\n${output}";

/// The Dramatize prompt: tag text for engines that interpret `[...]` emote
/// tags instead of speaking them. The ${output} placeholder is required.
pub const DEFAULT_POST_PROCESS_PROMPT: &str = "You add expressive audio tags to text for a speech synthesizer. Reply with ONLY the tagged text.\nRules:\n- Never add, remove, or change a word. You may insert tags, CAPITALIZE a word for emphasis, or add ?, ! or ellipses.\n- A tag is a short phrase in [square brackets], placed right before the words it affects, or right after for a reaction such as [sighs].\n- Tags describe the speaker's voice only: emotion, delivery, or a vocal sound. Never music, sound effects, scenery, or actions.\n- Use tags sparingly: at most about one per sentence, and none for flat, factual, or code-like text.\n- Leave code, URLs, numbers, and names untouched. Keep the text's language; tags stay in English.\n- Do not use <break> or SSML.\nTags to use: [happy] [sad] [excited] [angry] [whispers] [thoughtful] [surprised] [laughs] [chuckles] [sighs] [clears throat] [short pause] [long pause]. Similar tags are fine, e.g. [low, steady voice].\nExample: \"Are you serious? I can't believe you did that!\" becomes \"[appalled] Are you serious? [sighs] I can't believe you did that!\"\nText:\n${output}";

/// Prompts that were defaults of past versions: the shipped caveman default
/// and the never-wired dead-file presets. A stored legacy `prompt` equal to
/// any of these (or empty) is stale default, not user content — the fold
/// migrates it to the builtins instead of preserving it as a template.
pub const LEGACY_DEFAULT_PROMPTS: &[&str] = &[
    "Rewrite text terse like smart caveman for software developer listening. All technical substance stay. Only fluff die.\n\nRules:\n- Drop articles, filler words, pleasantries, hedging, repetition, and boilerplate.\n- Keep technical facts, names, numbers, code identifiers, commands, and original language exact.\n- Max 3 bullets/points. No framing or commentary. Output only rewritten text.\n\nPattern: [thing] [action] [reason]. [next step].\n\nText:\n${output}",
    "Compress aggressively into terse caveman-style developer notes. Keep technical facts, names, paths, and commands exact. Prefer short sentences over lists unless the input is a list. Return only the compressed text.",
    "Rewrite the text to be shorter, clearer, and easier for a developer to understand. Preserve meaning. Return only the rewritten text, not bullets unless the input is already a list.",
    "Summarize the text for a developer in 1-3 concise sentences. Preserve key decisions, requirements, and action items. Return only the summary.",
];

/// The two builtins, in display order. Fresh configs and template-less
/// legacy configs both seed with these.
pub fn builtin_templates() -> Vec<PostProcessTemplate> {
    vec![
        PostProcessTemplate {
            id: DRAMATIZE_TEMPLATE_ID.to_string(),
            name: "Dramatize".to_string(),
            class: PostProcessTemplateClass::Preserve,
            prompt: DEFAULT_POST_PROCESS_PROMPT.to_string(),
        },
        PostProcessTemplate {
            id: CONCISE_TEMPLATE_ID.to_string(),
            name: "Concise".to_string(),
            class: PostProcessTemplateClass::Rewrite,
            prompt: CONCISE_PROMPT.to_string(),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PostProcessConfig {
    pub enabled: bool,
    pub provider_id: String,
    pub providers: Vec<PostProcessProvider>,
    /// The prompt library; exactly one entry is active at a time.
    #[serde(default)]
    pub templates: Vec<PostProcessTemplate>,
    /// The active template's id; repaired to a builtin when it dangles.
    #[serde(default)]
    pub active_template_id: String,
    /// Minimum input length (in characters) before post-processing runs;
    /// shorter text is spoken verbatim. 0 = process everything.
    #[serde(default = "default_min_chars")]
    pub min_chars: usize,
    /// When true, post-processing runs only for requests that explicitly ask
    /// for it (the control server's `post_process` field or the CLI
    /// `--post-process` flag); when false, every qualifying text is
    /// processed globally.
    #[serde(default = "default_require_request_flag")]
    pub require_request_flag: bool,
    /// HTTP timeout for the post-process request, in seconds. 0 = no timeout.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    // Legacy fields, folded by normalize() and blanked; skip-serialized once
    // empty so a folded config no longer carries them on disk.
    /// Legacy single global prompt (pre-template configs).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prompt: String,
    /// Legacy Groq-era provider key.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    /// Legacy Groq-era provider model.
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

impl PostProcessConfig {
    /// Fold legacy fields into their modern homes and repair inconsistent
    /// state. Idempotent; safe to call on every load.
    pub fn normalize(&mut self) {
        // Legacy single prompt → the template library. The fold runs once:
        // its outer gate is an empty library, so after seeding (or for
        // configs that already carry templates) the legacy field is residue
        // and is dropped. A stale default (empty, or any past version's
        // default) is not user content: it migrates to the builtins with
        // Dramatize active. Any other prompt IS user content: it becomes a
        // rewrite-class "Custom" template and stays active.
        if self.templates.is_empty() {
            let legacy = std::mem::take(&mut self.prompt);
            let is_stale_default = legacy.trim().is_empty()
                || legacy == DEFAULT_POST_PROCESS_PROMPT
                || LEGACY_DEFAULT_PROMPTS.contains(&legacy.as_str());
            self.templates = builtin_templates();
            if is_stale_default {
                self.active_template_id = DRAMATIZE_TEMPLATE_ID.to_string();
            } else {
                let custom = PostProcessTemplate {
                    id: "custom-legacy".to_string(),
                    name: "Custom".to_string(),
                    class: PostProcessTemplateClass::Rewrite,
                    prompt: legacy,
                };
                self.active_template_id = custom.id.clone();
                self.templates.push(custom);
            }
        } else {
            self.prompt.clear();
        }

        // Hand-edited configs may have deleted a builtin: reseed any missing
        // one. An EDITED builtin keeps its drift — builtins are editable by
        // design; only deletion is repaired.
        for builtin in builtin_templates() {
            if self.templates.iter().all(|t| t.id != builtin.id) {
                self.templates.push(builtin);
            }
        }

        // Dangling selection (template deleted, or an id from an edited
        // import) falls back to the Dramatize builtin, then the first entry.
        if self
            .templates
            .iter()
            .all(|t| t.id != self.active_template_id)
        {
            let fallback = self
                .templates
                .iter()
                .find(|t| t.id == DRAMATIZE_TEMPLATE_ID)
                .or_else(|| self.templates.first())
                .map(|t| t.id.clone())
                .unwrap_or_default();
            self.active_template_id = fallback;
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

    /// The active template, or `None` when the library is empty (a
    /// non-normalized config; `normalize()` guarantees both builtins).
    pub fn active_template(&self) -> Option<&PostProcessTemplate> {
        self.templates
            .iter()
            .find(|t| t.id == self.active_template_id)
            .or_else(|| self.templates.first())
    }
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        let mut cfg = Self {
            enabled: false,
            provider_id: default_provider_id(),
            providers: default_providers(),
            templates: Vec::new(),
            active_template_id: String::new(),
            min_chars: default_min_chars(),
            require_request_flag: default_require_request_flag(),
            timeout_secs: default_timeout_secs(),
            prompt: String::new(),
            api_key: String::new(),
            model: String::new(),
        };
        cfg.normalize();
        cfg
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
    fn default_config_seeds_builtin_templates_with_dramatize_active() {
        let cfg = PostProcessConfig::default();
        let ids: Vec<&str> = cfg.templates.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["dramatize", "concise"]);
        assert_eq!(cfg.active_template_id, "dramatize");
        assert_eq!(
            cfg.active_template().map(|t| t.class),
            Some(PostProcessTemplateClass::Preserve)
        );
        let concise = cfg.templates.iter().find(|t| t.id == "concise").unwrap();
        assert_eq!(concise.class, PostProcessTemplateClass::Rewrite);
    }

    #[test]
    fn concise_prompt_keeps_placeholder_and_pins_shape() {
        assert!(CONCISE_PROMPT.starts_with("You compress text"));
        assert!(
            CONCISE_PROMPT.contains("${output}"),
            "placeholder must survive"
        );
        assert!(CONCISE_PROMPT.contains("Keep every fact, name, number"));
        assert!(CONCISE_PROMPT.ends_with("Text:\n${output}"));
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

    /// Every prompt a pre-template config could have carried (an old
    /// version's default or an empty string) is stale default, not user
    /// content: the fold migrates to the builtins with Dramatize active.
    #[test]
    fn legacy_and_empty_prompts_fold_to_builtins_with_dramatize_active() {
        let mut legacy_prompts = vec!["".to_string()];
        legacy_prompts.extend(LEGACY_DEFAULT_PROMPTS.iter().map(|s| s.to_string()));
        for prompt in legacy_prompts {
            let mut cfg = PostProcessConfig {
                prompt,
                templates: Vec::new(),
                active_template_id: String::new(),
                ..Default::default()
            };
            cfg.normalize();
            assert_eq!(cfg.prompt, "");
            assert_eq!(
                cfg.templates
                    .iter()
                    .map(|t| t.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["dramatize", "concise"]
            );
            assert_eq!(cfg.active_template_id, "dramatize");
            // Idempotent: the second run must not re-fold or duplicate.
            cfg.normalize();
            assert_eq!(cfg.templates.len(), 2);
            assert_eq!(cfg.active_template_id, "dramatize");
        }
    }

    /// A custom legacy prompt is user content: it folds into a rewrite-class
    /// "Custom" template that stays active, verbatim.
    #[test]
    fn custom_prompts_fold_into_a_rewrite_template() {
        let mut cfg = PostProcessConfig {
            prompt: "my own style ${output}".to_string(),
            templates: Vec::new(),
            active_template_id: String::new(),
            ..Default::default()
        };
        cfg.normalize();
        assert_eq!(cfg.prompt, "");
        assert_eq!(cfg.templates.len(), 3);
        let custom = cfg
            .templates
            .iter()
            .find(|t| t.id == "custom-legacy")
            .unwrap();
        assert_eq!(custom.name, "Custom");
        assert_eq!(custom.class, PostProcessTemplateClass::Rewrite);
        assert_eq!(custom.prompt, "my own style ${output}");
        assert_eq!(cfg.active_template_id, "custom-legacy");
        // Idempotent: no duplicate Custom on the second run.
        cfg.normalize();
        assert_eq!(cfg.templates.len(), 3);
        assert_eq!(cfg.active_template_id, "custom-legacy");
    }

    /// A template-less JSON config (pre-template writer) folds its prompt
    /// and re-serializes without the legacy field; templates land on disk.
    #[test]
    fn legacy_json_parses_folds_and_drops_the_legacy_prompt() {
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
        // The custom prompt survives as the active rewrite template.
        let active = cfg.active_template().unwrap();
        assert_eq!(active.prompt, "p ${output}");
        assert_eq!(active.class, PostProcessTemplateClass::Rewrite);
        // Folded config re-serializes without the legacy fields.
        let out = serde_json::to_string(&cfg).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.get("api_key").is_none());
        assert!(v.get("model").is_none());
        assert!(v.get("prompt").is_none());
        assert_eq!(v["templates"].as_array().unwrap().len(), 3);
        assert_eq!(v["active_template_id"], "custom-legacy");
        assert_eq!(v["providers"].as_array().unwrap()[0]["api_key"], "gsk_old");
    }

    /// Missing template fields deserialize to an empty library (the field
    /// default), which the load-time normalize() then seeds with builtins.
    #[test]
    fn missing_template_fields_default_to_builtins() {
        let mut cfg: PostProcessConfig = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert_eq!(cfg.providers.len(), 6);
        assert!(cfg.templates.is_empty());
        cfg.normalize();
        let active = cfg.active_template().expect("builtins seeded by normalize");
        assert_eq!(active.id, "dramatize");
        assert!(active.prompt.contains("${output}"));
        // The legacy prompt field is absent (Default leaves it empty).
        assert_eq!(cfg.prompt, "");
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

    #[test]
    fn normalize_reseeds_deleted_builtins() {
        let mut cfg = PostProcessConfig::default();
        cfg.normalize();
        // A hand-edited config dropped Concise: normalize restores it without
        // disturbing the edited Dramatize prompt or the active selection.
        cfg.templates.retain(|t| t.id != "concise");
        let dramatize = cfg
            .templates
            .iter_mut()
            .find(|t| t.id == "dramatize")
            .unwrap();
        dramatize.prompt = "edited ${output}".to_string();
        cfg.normalize();
        let ids: Vec<&str> = cfg.templates.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"concise"));
        let dramatize = cfg.templates.iter().find(|t| t.id == "dramatize").unwrap();
        assert_eq!(dramatize.prompt, "edited ${output}");
        assert_eq!(cfg.active_template_id, "dramatize");
    }

    #[test]
    fn normalize_repairs_dangling_active_template_id() {
        let mut cfg = PostProcessConfig {
            active_template_id: "deleted-template".to_string(),
            ..Default::default()
        };
        cfg.normalize();
        assert_eq!(cfg.active_template_id, "dramatize");
    }

    #[test]
    fn active_template_falls_back_to_first() {
        let mut cfg = PostProcessConfig {
            active_template_id: "nope".to_string(),
            ..Default::default()
        };
        cfg.normalize();
        cfg.templates.retain(|t| t.id != "dramatize");
        cfg.active_template_id = "nope".to_string();
        // Without normalize, active_template() itself still falls back.
        assert_eq!(
            cfg.active_template().map(|t| t.id.as_str()),
            Some("concise")
        );
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
