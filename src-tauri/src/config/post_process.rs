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
#[path = "post_process_tests.rs"]
mod tests;
