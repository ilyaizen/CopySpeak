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
