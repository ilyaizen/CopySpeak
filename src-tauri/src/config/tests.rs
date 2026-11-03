#[cfg(test)]
mod tests {
    use crate::config::*;
    use crate::config::{load_or_default_from, save_to};

    #[test]
    fn unset_locale_remains_distinct_from_an_explicit_english_choice() {
        let mut config = serde_json::to_value(AppConfig::default()).unwrap();
        assert_eq!(config["general"]["locale"], "");
        config["general"].as_object_mut().unwrap().remove("locale");
        let restored: AppConfig = serde_json::from_value(config).unwrap();
        assert_eq!(restored.general.locale, "");
        let mut explicit = restored;
        explicit.general.locale = "en".to_string();
        let restored: AppConfig =
            serde_json::from_str(&serde_json::to_string(&explicit).unwrap()).unwrap();
        assert_eq!(restored.general.locale, "en");
    }

    #[test]
    fn streaming_setting_defaults_on_and_persists_when_disabled() {
        let mut config = serde_json::to_value(AppConfig::default()).unwrap();
        assert_eq!(config["playback"]["streaming_enabled"], true);
        config["playback"]
            .as_object_mut()
            .unwrap()
            .remove("streaming_enabled");
        let mut restored: AppConfig = serde_json::from_value(config).unwrap();
        assert!(restored.playback.streaming_enabled);
        restored.playback.streaming_enabled = false;
        let saved = serde_json::to_string(&restored).unwrap();
        let restored: AppConfig = serde_json::from_str(&saved).unwrap();
        assert!(!restored.playback.streaming_enabled);
    }

    #[test]
    fn test_expand_filename_pattern_timestamp() {
        let result = expand_filename_pattern("output_{timestamp}.wav", "af_heart", "hello");
        assert!(result.starts_with("output_"));
        assert!(result.ends_with(".wav"));
        let ts_part = result
            .strip_prefix("output_")
            .unwrap()
            .strip_suffix(".wav")
            .unwrap();
        assert!(ts_part.parse::<i64>().is_ok());
    }

    #[test]
    fn test_expand_filename_pattern_voice() {
        let result = expand_filename_pattern("tts_{voice}.wav", "af_heart", "hello");
        assert_eq!(result, "tts_af_heart.wav");
    }

    #[test]
    fn test_expand_filename_pattern_text_sanitization() {
        let result = expand_filename_pattern("{text}.wav", "voice", "Hello World");
        assert_eq!(result, "hello_world.wav");

        let result = expand_filename_pattern("{text}.wav", "voice", "Hello! @World#");
        assert_eq!(result, "hello_world.wav");

        let long_text = "This is a very long text that should be truncated at thirty characters";
        let result = expand_filename_pattern("{text}.wav", "voice", long_text);
        let text_part = result.strip_suffix(".wav").unwrap();
        assert!(
            text_part.len() <= 30,
            "Text part '{}' is {} chars",
            text_part,
            text_part.len()
        );
    }

    #[test]
    fn test_expand_filename_pattern_datetime() {
        let result = expand_filename_pattern("copyspeak_{datetime}.wav", "voice", "text");
        assert!(result.starts_with("copyspeak_"));
        assert!(result.ends_with(".wav"));
        let datetime_part = result
            .strip_prefix("copyspeak_")
            .unwrap()
            .strip_suffix(".wav")
            .unwrap();
        assert_eq!(datetime_part.len(), 17); // YYYY-MM-DD_HHMMSS
        assert!(datetime_part.chars().nth(4) == Some('-'));
        assert!(datetime_part.chars().nth(7) == Some('-'));
        assert!(datetime_part.chars().nth(10) == Some('_'));
    }

    #[test]
    fn test_expand_filename_pattern_date_and_time() {
        let result = expand_filename_pattern("{date}_{time}.wav", "voice", "text");
        assert!(result.ends_with(".wav"));
        let parts: Vec<&str> = result.strip_suffix(".wav").unwrap().split('_').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].len(), 10); // YYYY-MM-DD
        assert_eq!(parts[1].len(), 6); // HHMMSS
    }

    #[test]
    fn test_expand_filename_pattern_multiple_placeholders() {
        let result = expand_filename_pattern("{voice}_{text}_{date}.wav", "af_heart", "Hello");
        assert!(result.starts_with("af_heart_hello_"));
        assert!(result.ends_with(".wav"));
    }

    #[test]
    fn test_expand_filename_pattern_no_placeholders() {
        let result = expand_filename_pattern("static_filename.wav", "voice", "text");
        assert_eq!(result, "static_filename.wav");
    }

    #[test]
    fn test_expand_filename_pattern_voice_sanitization() {
        let result = expand_filename_pattern("{voice}.wav", "voice/with:bad<chars>", "text");
        assert_eq!(result, "voicewithbadchars.wav");
    }

    // ========== AudioFormat Tests ==========

    #[test]
    fn test_audio_format_default_extension() {
        assert_eq!(AudioFormat::Wav.default_extension(), "wav");
        assert_eq!(AudioFormat::Mp3.default_extension(), "mp3");
        assert_eq!(AudioFormat::Ogg.default_extension(), "ogg");
        assert_eq!(AudioFormat::Flac.default_extension(), "flac");
    }

    #[test]
    fn test_audio_format_from_extension() {
        assert_eq!(AudioFormat::from_extension("wav"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_extension("WAV"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_extension("mp3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("MP3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("ogg"), Some(AudioFormat::Ogg));
        assert_eq!(AudioFormat::from_extension("flac"), Some(AudioFormat::Flac));
        assert_eq!(AudioFormat::from_extension("xyz"), None);
    }

    #[test]
    fn test_audio_format_serialization() {
        assert_eq!(serde_json::to_string(&AudioFormat::Wav).unwrap(), "\"wav\"");
        assert_eq!(serde_json::to_string(&AudioFormat::Mp3).unwrap(), "\"mp3\"");
        assert_eq!(serde_json::to_string(&AudioFormat::Ogg).unwrap(), "\"ogg\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Flac).unwrap(),
            "\"flac\""
        );
    }

    #[test]
    fn test_audio_format_deserialization() {
        assert_eq!(
            serde_json::from_str::<AudioFormat>("\"wav\"").unwrap(),
            AudioFormat::Wav
        );
        assert_eq!(
            serde_json::from_str::<AudioFormat>("\"mp3\"").unwrap(),
            AudioFormat::Mp3
        );
        assert_eq!(
            serde_json::from_str::<AudioFormat>("\"ogg\"").unwrap(),
            AudioFormat::Ogg
        );
        assert_eq!(
            serde_json::from_str::<AudioFormat>("\"flac\"").unwrap(),
            AudioFormat::Flac
        );
    }

    #[test]
    fn test_format_config_default() {
        let config = FormatConfig::default();
        assert_eq!(config.format, AudioFormat::Wav);
        assert_eq!(config.mp3_bitrate, 192);
        assert_eq!(config.ogg_bitrate, 192);
        assert_eq!(config.flac_compression, 5);
    }

    #[test]
    fn test_format_config_serialization_roundtrip() {
        let config = FormatConfig {
            format: AudioFormat::Mp3,
            mp3_bitrate: 256,
            ogg_bitrate: 192,
            flac_compression: 8,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: FormatConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.format, config.format);
        assert_eq!(deserialized.mp3_bitrate, config.mp3_bitrate);
        assert_eq!(deserialized.ogg_bitrate, config.ogg_bitrate);
        assert_eq!(deserialized.flac_compression, config.flac_compression);
    }

    #[test]
    fn test_output_config_with_format() {
        let config = OutputConfig {
            enabled: true,
            directory: "/tmp/audio".into(),
            filename_pattern: "test_{timestamp}.mp3".into(),
            format_config: FormatConfig {
                format: AudioFormat::Mp3,
                mp3_bitrate: 320,
                ogg_bitrate: 192,
                flac_compression: 5,
            },
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: OutputConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(deserialized.directory, config.directory);
        assert_eq!(deserialized.filename_pattern, config.filename_pattern);
        assert_eq!(deserialized.format_config.format, AudioFormat::Mp3);
        assert_eq!(deserialized.format_config.mp3_bitrate, 320);
    }

    #[test]
    fn test_output_config_missing_format_uses_default() {
        let json = r#"{
            "enabled": true,
            "directory": "/tmp",
            "filename_pattern": "test.wav"
        }"#;

        let config: OutputConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.format_config.format, AudioFormat::Wav);
        assert_eq!(config.format_config.mp3_bitrate, 192);
    }

    // ========== Validation Tests ==========

    #[test]
    fn test_validation_valid_config() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_command_empty() {
        let mut config = AppConfig::default();
        config.tts.active_backend = TtsEngine::Local;
        if let Some(profile) = config
            .tts
            .profiles
            .iter_mut()
            .find(|p| p.id == config.tts.active_profile_id)
        {
            profile.engine = TtsEngine::Local;
        }
        config.tts.command = "".into();
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::CommandEmpty)));
    }

    #[test]
    fn test_validation_args_template_missing_placeholders() {
        let mut config = AppConfig::default();
        config.tts.active_backend = TtsEngine::Local;
        if let Some(profile) = config
            .tts
            .profiles
            .iter_mut()
            .find(|p| p.id == config.tts.active_profile_id)
        {
            profile.engine = TtsEngine::Local;
        }
        config.tts.args_template = vec!["-v".into(), "{voice}".into()];
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::ArgsTemplateMissingPlaceholder { .. })));
    }

    // ========== TTS Profile / Migration Tests ==========

    #[test]
    fn test_default_tts_config_uses_kokoro_profile() {
        let tts = TtsConfig::default();
        assert_eq!(TtsEngine::default(), TtsEngine::Kokoro);
        assert_eq!(tts.schema_version, 8);
        assert_eq!(tts.active_backend, TtsEngine::Kokoro);
        // The Edge seed profile is back (engine revived on edge-tts 7.3.1):
        // zero-download fallback for onboarding.
        assert!(tts
            .profiles
            .iter()
            .any(|profile| profile.engine == TtsEngine::Edge));
        assert_eq!(tts.profiles.len(), 10);
        // The ACTIVE profile is the bundled Kokoro one.
        assert_eq!(tts.active_profile_id, "profile-kokoro-default");
        assert_eq!(
            tts.profiles
                .iter()
                .find(|profile| profile.id == tts.active_profile_id)
                .map(|profile| &profile.engine),
            Some(&TtsEngine::Kokoro)
        );
    }

    #[test]
    fn test_migrate_add_kokoro_profile_v6_adds_and_is_idempotent() {
        // A v5 config (no Kokoro profile) gains one; the active engine is
        // never touched by the migration.
        let mut tts = TtsConfig::default();
        tts.schema_version = 5;
        tts.profiles.retain(|p| p.id != "profile-kokoro-default");
        tts.active_backend = TtsEngine::Kitten;
        tts.active_profile_id = crate::config::tts::KITTEN_DEFAULT_PROFILE_ID.into();
        let before = tts.profiles.len();

        crate::config::tts::migrate_add_kokoro_profile_v6(&mut tts);
        assert_eq!(tts.schema_version, 6);
        assert_eq!(tts.profiles.len(), before + 1);
        assert!(tts
            .profiles
            .iter()
            .any(|p| p.id == "profile-kokoro-default" && p.engine == TtsEngine::Kokoro));
        assert_eq!(tts.active_backend, TtsEngine::Kitten);
        assert_eq!(
            tts.active_profile_id,
            crate::config::tts::KITTEN_DEFAULT_PROFILE_ID
        );

        // Idempotent.
        crate::config::tts::migrate_add_kokoro_profile_v6(&mut tts);
        assert_eq!(tts.profiles.len(), before + 1);
        assert_eq!(tts.schema_version, 6);
    }

    #[test]
    fn v7_promotes_installer_shape_local_piper_and_refreshes_the_mirror() {
        let mut tts = TtsConfig::default();
        let mut profile = default_kokoro_profile();
        profile.id = "piper-local".into();
        profile.engine = TtsEngine::Local;
        profile.voice = "en_US-joe-medium".into();
        profile.voice_label = None;
        profile.engine_options = ProfileEngineOptions::Local(LocalEngineOptions {
            command: Some("uv".into()),
            args_template: Some(
                [
                    "run",
                    "--project",
                    "{engine_dir}/piper",
                    "python",
                    "{engine_dir}/piper/scripts/copyspeak-piper.py",
                    "--text-file",
                    "{input}",
                    "--voice",
                    "{voice}",
                    "--output",
                    "{output}",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ),
        });
        tts.profiles.push(profile);
        tts.active_backend = TtsEngine::Local;
        tts.active_profile_id = "piper-local".into();

        crate::config::tts::migrate_tts_profiles_v7(&mut tts);

        assert_eq!(tts.schema_version, 7);
        let promoted = tts.profiles.iter().find(|p| p.id == "piper-local").unwrap();
        assert_eq!(promoted.engine, TtsEngine::Piper);
        assert!(matches!(
            promoted.engine_options,
            ProfileEngineOptions::Piper(_)
        ));
        assert_eq!(promoted.voice_label.as_deref(), Some("Joe"));
        // The derived mirror follows the promoted active profile.
        assert_eq!(tts.active_backend, TtsEngine::Piper);
    }

    #[test]
    fn v7_leaves_hand_written_local_commands_alone() {
        let mut tts = TtsConfig::default();
        let mut profile = default_kokoro_profile();
        profile.id = "my-cli".into();
        profile.engine = TtsEngine::Local;
        profile.engine_options = ProfileEngineOptions::Local(LocalEngineOptions {
            command: Some("python".into()),
            args_template: Some(vec!["-m".into(), "my_tts".into(), "{input}".into()]),
        });
        let before = profile.clone();
        tts.profiles.push(profile);

        crate::config::tts::migrate_tts_profiles_v7(&mut tts);

        let kept = tts.profiles.iter().find(|p| p.id == "my-cli").unwrap();
        assert_eq!(kept.engine, TtsEngine::Local);
        assert_eq!(kept.engine_options, before.engine_options);
        assert_eq!(tts.schema_version, 7);
    }

    #[test]
    fn v7_remaps_legacy_and_unset_gemini_models_but_not_custom_ones() {
        let mut tts = TtsConfig::default();
        let mut legacy = default_google_profile();
        legacy.id = "g-legacy".into();
        legacy.engine_options = ProfileEngineOptions::Google(GoogleEngineOptions {
            model: Some(crate::config::tts::LEGACY_GEMINI_TTS_MODEL.into()),
            output_format: Some("wav".into()),
        });
        let mut unset = default_google_profile();
        unset.id = "g-unset".into();
        unset.engine_options = ProfileEngineOptions::Google(GoogleEngineOptions {
            model: None,
            output_format: Some("wav".into()),
        });
        let mut custom = default_google_profile();
        custom.id = "g-custom".into();
        custom.engine_options = ProfileEngineOptions::Google(GoogleEngineOptions {
            model: Some("gemini-3.8-flash-lite-tts".into()),
            output_format: Some("wav".into()),
        });
        tts.profiles.extend([legacy, unset, custom]);
        tts.google.model = crate::config::tts::LEGACY_GEMINI_TTS_MODEL.into();

        crate::config::tts::migrate_tts_profiles_v7(&mut tts);

        let model = |tts: &TtsConfig, id: &str| {
            tts.profiles
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .engine_options
                .google()
                .unwrap()
                .model
                .clone()
                .unwrap()
        };
        assert_eq!(
            model(&tts, "g-legacy"),
            crate::config::tts::GEMINI_TTS_MODEL
        );
        assert_eq!(model(&tts, "g-unset"), crate::config::tts::GEMINI_TTS_MODEL);
        assert_eq!(model(&tts, "g-custom"), "gemini-3.8-flash-lite-tts");
        assert_eq!(tts.google.model, crate::config::tts::GEMINI_TTS_MODEL);

        // Idempotent: a second pass changes nothing.
        crate::config::tts::migrate_tts_profiles_v7(&mut tts);
        assert_eq!(model(&tts, "g-custom"), "gemini-3.8-flash-lite-tts");
        assert_eq!(tts.schema_version, 7);
    }

    #[test]
    fn test_migrate_legacy_config_creates_default_profile() {
        // Simulate a v0 config: no profiles, ElevenLabs active.
        let mut tts = TtsConfig::default();
        tts.schema_version = 0;
        tts.profiles = Vec::new();
        tts.active_backend = TtsEngine::ElevenLabs;
        tts.elevenlabs.voice_id = "voice-xyz".into();

        let migrated = migrate_tts_config(tts);
        assert_eq!(migrated.schema_version, 2);
        assert_eq!(migrated.active_profile_id, "default");
        assert_eq!(migrated.profiles.len(), 1);
        assert_eq!(migrated.profiles[0].engine, TtsEngine::ElevenLabs);
        assert_eq!(migrated.profiles[0].voice, "voice-xyz");
    }

    #[test]
    fn test_migrate_is_idempotent() {
        let tts = TtsConfig::default();
        let once = migrate_tts_config(tts);
        let twice = migrate_tts_config(once.clone());
        assert_eq!(once.profiles.len(), twice.profiles.len());
        assert_eq!(twice.schema_version, 2);
    }

    #[test]
    fn test_profile_json_roundtrip() {
        let profile = VoiceProfile {
            id: "p1".into(),
            name: "P1".into(),
            description: None,
            engine: TtsEngine::Http,
            voice: "amy.wav".into(),
            voice_label: None,
            speed: 1.05,
            pitch: 1.0,
            effects: ProfileEffects {
                enabled: true,
                active_effect: EffectId::WalkieTalkie,
            },
            post_process: crate::config::ProfilePostProcess {
                enabled: true,
                active_template_id: Some("dramatize".into()),
            },
            text_processing: crate::config::ProfileTextProcessing::default(),
            engine_options: ProfileEngineOptions::Http(HttpEngineOptions {
                body_template: Some(r#"{"model":"tts"}"#.into()),
                ..HttpEngineOptions::default()
            }),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let back: VoiceProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "p1");
        assert_eq!(back.engine, TtsEngine::Http);
        assert!(back.effects.enabled);
        assert_eq!(back.effects.active_effect, EffectId::WalkieTalkie);
        assert!(back.post_process.enabled);
        assert_eq!(
            back.post_process.active_template_id.as_deref(),
            Some("dramatize")
        );
    }

    #[test]
    fn test_migrate_post_process_to_profiles_v8() {
        let mut cfg = AppConfig::default();
        cfg.tts.schema_version = 7;
        cfg.post_process.enabled = true;
        cfg.post_process.active_template_id = "concise".into();

        // Reset profile post-process settings
        for profile in &mut cfg.tts.profiles {
            profile.post_process.enabled = false;
            profile.post_process.active_template_id = None;
        }

        crate::config::tts::migrate_post_process_to_profiles_v8(&mut cfg);
        assert_eq!(cfg.tts.schema_version, 8);
        for profile in &cfg.tts.profiles {
            assert!(profile.post_process.enabled);
            assert_eq!(
                profile.post_process.active_template_id.as_deref(),
                Some("concise")
            );
        }
    }

    #[test]
    fn test_validation_double_copy_window_too_small() {
        let mut config = AppConfig::default();
        config.trigger.double_copy_window_ms = 50;
        let result = config.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::DoubleCopyWindowTooSmall { .. })));
    }

    #[test]
    fn default_profile_speed_and_pitch_are_neutral() {
        let profile = TtsConfig::default()
            .profiles
            .into_iter()
            .find(|p| p.id == crate::config::tts::KOKORO_DEFAULT_PROFILE_ID)
            .expect("default config bundles the Kokoro profile");
        assert_eq!(profile.speed, 1.0);
        assert_eq!(profile.pitch, 1.0);
    }

    #[test]
    fn edge_profiles_are_readopted_on_load() {
        use crate::config::tts::{migrate_add_kokoro_profile_v6, KOKORO_DEFAULT_PROFILE_ID};

        // A v5-era config: active Edge profile, no Kokoro profile yet. Since
        // edge-tts 7.3.1 speaks the revived endpoint, load keeps the profile
        // and the active selection instead of retiring them (the 0.2.11
        // retirement is reverted; Edge profiles never re-enter saved configs
        // on their own, so this only guards the load path).
        let mut tts = TtsConfig::default();
        tts.schema_version = 5;
        tts.profiles.retain(|p| p.id != KOKORO_DEFAULT_PROFILE_ID);
        let mut edge_profile = default_kokoro_profile();
        edge_profile.id = "default".into();
        edge_profile.engine = TtsEngine::Edge;
        edge_profile.voice = "en-US-AvaMultilingualNeural".into();
        edge_profile.voice_label = None;
        tts.profiles.push(edge_profile);
        tts.active_backend = TtsEngine::Edge;
        tts.active_profile_id = "default".into();

        migrate_add_kokoro_profile_v6(&mut tts);

        let active = tts
            .profiles
            .iter()
            .find(|p| p.id == tts.active_profile_id)
            .unwrap();
        assert_eq!(active.engine, TtsEngine::Edge);
        assert_eq!(active.voice, "en-US-AvaMultilingualNeural");
        assert!(tts
            .profiles
            .iter()
            .any(|p| p.id == KOKORO_DEFAULT_PROFILE_ID));
    }

    #[test]
    fn loaded_config_gets_current_version_stamp() {
        let mut config = AppConfig::default();
        config.version = "0.1.10-fossil".into();
        let stamped = stamp_config_version(config);
        assert_eq!(stamped.version, CONFIG_VERSION);
    }

    #[test]
    fn legacy_configs_keep_the_tray_visible() {
        let legacy: GeneralConfig =
            serde_json::from_str(r#"{"launch_on_login": false, "start_minimized": true}"#).unwrap();
        assert!(legacy.show_tray_icon);
        assert!(AppConfig::default().general.show_tray_icon);
    }

    /// Experimental features (Dramatize, post-processing panel, effects) are
    /// opt-in: fresh and legacy configs must both land on `false`.
    #[test]
    fn experimental_defaults_off_and_survives_legacy_configs() {
        assert!(!AppConfig::default().general.experimental_enabled);
        let legacy: crate::config::GeneralConfig =
            serde_json::from_str(r#"{"launch_on_login": false, "start_minimized": false}"#)
                .unwrap();
        assert!(!legacy.experimental_enabled);
        assert!(
            !legacy.effects_enabled,
            "effects_enabled must default to disabled in partial configs"
        );
    }

    #[test]
    fn fresh_defaults_duck_on_effects_off_and_streaming_is_on() {
        let config = AppConfig::default();
        assert!(config.playback.duck.enabled, "duck must default to enabled");
        assert_eq!(
            config.playback.duck.level_percent, 10,
            "duck level defaults to 10%"
        );
        assert!(
            config.playback.streaming_enabled,
            "streaming must default to enabled"
        );
        assert!(!config.history.enabled, "history must default to disabled");
        assert!(
            !config.post_process.enabled,
            "post_process must default to disabled"
        );
        assert!(
            !config.general.experimental_enabled,
            "experimental must default to disabled"
        );
        assert!(
            !config.general.effects_enabled,
            "the master effects gate must default to disabled"
        );

        let pe = ProfileEffects::default();
        assert!(!pe.enabled, "ProfileEffects default must be disabled");
        assert_eq!(
            pe.active_effect,
            EffectId::None,
            "ProfileEffects default effect must be None"
        );

        for profile in &config.tts.profiles {
            assert!(
                !profile.effects.enabled,
                "default profile '{}' effects must be disabled",
                profile.name
            );
            assert_eq!(
                profile.effects.active_effect,
                EffectId::None,
                "default profile '{}' active_effect must be None",
                profile.name
            );
        }
    }

    #[test]
    fn missing_fields_deserialize_to_safe_defaults() {
        let playback: PlaybackConfig =
            serde_json::from_str(r#"{"on_retrigger": "interrupt"}"#).unwrap();
        assert!(playback.duck.enabled);
        assert_eq!(playback.duck.level_percent, 10);
        assert!(playback.streaming_enabled);

        let pe: ProfileEffects = serde_json::from_str(r#"{}"#).unwrap();
        assert!(!pe.enabled);
        assert_eq!(pe.active_effect, EffectId::None);
    }

    #[test]
    fn explicit_legacy_values_are_preserved() {
        let json = r#"{
            "on_retrigger": "interrupt",
            "duck": {"enabled": true, "level_percent": 35},
            "streaming_enabled": false
        }"#;
        let playback: PlaybackConfig = serde_json::from_str(json).unwrap();
        assert!(playback.duck.enabled);
        assert_eq!(playback.duck.level_percent, 35);
        assert!(!playback.streaming_enabled);

        let pe_json = r#"{"enabled": true, "active_effect": "walkie_talkie"}"#;
        let pe: ProfileEffects = serde_json::from_str(pe_json).unwrap();
        assert!(pe.enabled);
        assert_eq!(pe.active_effect, EffectId::WalkieTalkie);
    }

    #[test]
    fn effective_hotkey_preserves_saved_preference() {
        let mut config = AppConfig::default();
        assert!(!config.hotkey.enabled);
        for experimental in [false, true] {
            for enabled in [false, true] {
                config.general.experimental_enabled = experimental;
                config.hotkey.enabled = enabled;
                let effective = config.effective_hotkey();
                assert_eq!(effective.enabled, experimental && enabled);
                assert_eq!(effective.shortcut, config.hotkey.shortcut);
                assert_eq!(config.hotkey.enabled, enabled);
            }
        }
    }

    #[test]
    fn effective_history_predicate_truth_table() {
        let mut config = AppConfig::default();

        config.general.experimental_enabled = false;
        config.history.enabled = false;
        assert!(!config.is_history_effective());

        config.general.experimental_enabled = false;
        config.history.enabled = true;
        assert!(!config.is_history_effective());

        config.general.experimental_enabled = true;
        config.history.enabled = false;
        assert!(!config.is_history_effective());

        config.general.experimental_enabled = true;
        config.history.enabled = true;
        assert!(config.is_history_effective());
    }

    #[test]
    fn effective_duck_predicate_ignores_experimental() {
        let mut config = AppConfig::default();

        for experimental in [false, true] {
            config.general.experimental_enabled = experimental;
            config.playback.duck.enabled = false;
            assert!(!config.is_duck_effective());
            config.playback.duck.enabled = true;
            assert!(config.is_duck_effective());
        }
    }

    #[test]
    fn effective_hud_predicate_truth_table() {
        let mut config = AppConfig::default();

        config.general.experimental_enabled = false;
        config.hud.enabled = false;
        assert!(!config.is_hud_effective());

        config.general.experimental_enabled = false;
        config.hud.enabled = true;
        assert!(!config.is_hud_effective());

        config.general.experimental_enabled = true;
        config.hud.enabled = false;
        assert!(!config.is_hud_effective());

        config.general.experimental_enabled = true;
        config.hud.enabled = true;
        assert!(config.is_hud_effective());
    }

    // ── On-disk load / reload migration regression tests ─────────────────────
    //
    // These write real files through save_to and read them back through
    // load_or_default_from (the load_or_default/save bodies with the path
    // injected), so the full disk path is exercised without ever touching the
    // user's live config at config_path().

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Unique per-test config path: parallel test threads each get their own
    /// temp directory, keyed by pid + monotonic counter + label.
    fn temp_config_path(label: &str) -> PathBuf {
        static NEXT_ID: AtomicU32 = AtomicU32::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "copyspeak-config-test-{}-{}-{label}",
            std::process::id(),
            id
        ));
        std::fs::create_dir_all(&dir).expect("create temp config dir");
        dir.join("config.json")
    }

    /// Best-effort cleanup of a test's temp config directory.
    fn discard_temp_config(path: &PathBuf) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// A pre-profiles (schema_version-less) config on disk must deserialize,
    /// walk the whole migration ladder, and land on v8: legacy engine/voice
    /// becomes the active profile, legacy playback knobs move onto it (v3),
    /// bundled profiles are added (v4-v6), and per-profile post-process is
    /// seeded from the global settings (v8).
    #[test]
    fn legacy_on_disk_config_migrates_to_v8_and_seeds_post_process() {
        let path = temp_config_path("legacy-v0");
        // A 0.1.x-era file: no `schema_version`, no `profiles`, no
        // `active_profile_id`, legacy playback knobs, single global
        // post-process prompt. `prompt` is deliberately non-default user
        // content so normalize() folds it into a "custom-legacy" template
        // instead of forcing the Dramatize builtin — that folded id is what
        // the v8 seed must propagate per profile.
        std::fs::write(
            &path,
            r#"{
                "version": "0.1.5",
                "general": {"launch_on_login": false, "start_minimized": true},
                "trigger": {
                    "listen_enabled": true,
                    "double_copy_window_ms": 1500,
                    "max_text_length": 100000
                },
                "tts": {
                    "active_backend": "local",
                    "command": "uv",
                    "args_template": [
                        "run",
                        "--project",
                        "{engine_dir}/kitten",
                        "python",
                        "{engine_dir}/kitten/scripts/copyspeak-kitten.py",
                        "--text-file",
                        "{input}",
                        "--voice",
                        "{voice}",
                        "--output",
                        "{output}"
                    ],
                    "voice": "Rosie"
                },
                "playback": {"on_retrigger": "queue", "playback_speed": 1.35, "pitch": 1.15},
                "hud": {
                    "enabled": false,
                    "position": "bottom-left",
                    "width": 300,
                    "height": 100,
                    "opacity": 0.85
                },
                "post_process": {
                    "enabled": true,
                    "prompt": "Polish this for listening, keep every name: ${output}"
                }
            }"#,
        )
        .expect("write legacy config fixture");

        let cfg = load_or_default_from(&path);

        // Fully migrated and re-stamped to the running app version.
        assert_eq!(cfg.tts.schema_version, 8);
        assert_eq!(cfg.version, CONFIG_VERSION);

        // The legacy engine/voice became the active "default" profile, and
        // the non-default legacy playback knobs moved onto it (v2 → v3).
        let default_profile = cfg
            .tts
            .profiles
            .iter()
            .find(|p| p.id == "default")
            .expect("legacy config rebuilds a 'default' profile");
        assert_eq!(default_profile.engine, TtsEngine::Local);
        assert_eq!(default_profile.voice, "Rosie");
        assert_eq!(default_profile.speed, 1.35);
        assert_eq!(default_profile.pitch, 1.15);
        assert_eq!(cfg.tts.active_profile_id, "default");

        // v3 → v6 made the bundled profiles reachable.
        for bundled in [
            crate::config::tts::KITTEN_DEFAULT_PROFILE_ID,
            crate::config::tts::QWEN_DEFAULT_PROFILE_ID,
            crate::config::tts::KOKORO_DEFAULT_PROFILE_ID,
        ] {
            assert!(
                cfg.tts.profiles.iter().any(|p| p.id == bundled),
                "bundled profile {bundled} missing after migration"
            );
        }

        // v7 → v8 seeded every profile's post-process ownership from the
        // global settings (the folded "custom-legacy" template), and the
        // global master gate survived.
        for profile in &cfg.tts.profiles {
            assert!(
                profile.post_process.enabled,
                "profile {} post_process not seeded",
                profile.id
            );
            assert_eq!(
                profile.post_process.active_template_id.as_deref(),
                Some("custom-legacy"),
                "profile {} post_process template not seeded",
                profile.id
            );
        }
        assert!(cfg.post_process.enabled);

        discard_temp_config(&path);
    }

    /// The v8 regression: a v7 config's first load seeds every profile's
    /// post-process from the global settings; after the user gives profiles
    /// different per-profile settings and saves, a reload of the now-v8 file
    /// must NOT re-run the seed and clobber them back.
    #[test]
    fn v8_seed_does_not_overwrite_edited_per_profile_post_process_on_reload() {
        let path = temp_config_path("v7-seed-then-reload");

        // A v7-era config on disk: post-process is still global-only.
        let mut on_disk = AppConfig::default();
        on_disk.tts.schema_version = 7;
        on_disk.post_process.enabled = true;
        save_to(&path, &on_disk).expect("write v7 config");

        // First load: migrates to v8 and seeds per-profile post-process from
        // the global gate. Derive the seeded template from the loaded global
        // (normalize() owns which builtin ends up active) rather than
        // hardcoding it.
        let migrated = load_or_default_from(&path);
        assert_eq!(migrated.tts.schema_version, 8);
        assert!(migrated.post_process.enabled);
        let seeded_template = if migrated.post_process.active_template_id.is_empty() {
            None
        } else {
            Some(migrated.post_process.active_template_id.clone())
        };
        for profile in &migrated.tts.profiles {
            assert!(
                profile.post_process.enabled,
                "profile {} was not seeded on the v7 load",
                profile.id
            );
            assert_eq!(
                profile.post_process.active_template_id, seeded_template,
                "profile {} was not seeded with the global template",
                profile.id
            );
        }

        // The user splits per-profile settings away from the global ones and
        // saves. The custom template id can never equal a seeded builtin, and
        // the disabled profile can never equal the seeded enabled=true.
        let mut edited = migrated;
        let kokoro = edited
            .tts
            .profiles
            .iter_mut()
            .find(|p| p.id == crate::config::tts::KOKORO_DEFAULT_PROFILE_ID)
            .expect("bundled Kokoro profile");
        kokoro.post_process = ProfilePostProcess {
            enabled: true,
            active_template_id: Some("my-custom-template".into()),
        };
        let kitten = edited
            .tts
            .profiles
            .iter_mut()
            .find(|p| p.id == crate::config::tts::KITTEN_DEFAULT_PROFILE_ID)
            .expect("bundled Kitten profile");
        kitten.post_process = ProfilePostProcess {
            enabled: false,
            active_template_id: None,
        };
        save_to(&path, &edited).expect("save edited v8 config");

        // Reload: the per-profile settings must survive untouched, and the
        // ladder must not re-add anything or re-seed from the global gate.
        let reloaded = load_or_default_from(&path);
        assert_eq!(reloaded.tts.schema_version, 8);
        assert_eq!(
            reloaded.tts.profiles.len(),
            edited.tts.profiles.len(),
            "reload changed the profile set"
        );
        let kokoro = reloaded
            .tts
            .profiles
            .iter()
            .find(|p| p.id == crate::config::tts::KOKORO_DEFAULT_PROFILE_ID)
            .unwrap();
        assert!(kokoro.post_process.enabled, "Kokoro opt-in was overwritten");
        assert_eq!(
            kokoro.post_process.active_template_id.as_deref(),
            Some("my-custom-template"),
            "Kokoro template was re-seeded on reload"
        );
        let kitten = reloaded
            .tts
            .profiles
            .iter()
            .find(|p| p.id == crate::config::tts::KITTEN_DEFAULT_PROFILE_ID)
            .unwrap();
        assert!(
            !kitten.post_process.enabled,
            "Kitten opt-out was overwritten by the v8 seed"
        );
        assert_eq!(kitten.post_process.active_template_id, None);
        // The global master gate is untouched by the reload.
        assert!(reloaded.post_process.enabled);

        discard_temp_config(&path);
    }

    /// A v8 config on disk loads as-is: per-profile post-process survives
    /// even where it disagrees with the global gate (global off, profiles
    /// opted in), proving the seed does not run again on an already-v8 file.
    #[test]
    fn v8_on_disk_config_keeps_per_profile_post_process() {
        let path = temp_config_path("v8-on-disk");

        let mut on_disk = AppConfig::default();
        // Global gate OFF while two profiles opted in: a re-run of the v8
        // seed would stamp the global disabled state over the opt-ins.
        on_disk.post_process.enabled = false;
        let kokoro = on_disk
            .tts
            .profiles
            .iter_mut()
            .find(|p| p.id == crate::config::tts::KOKORO_DEFAULT_PROFILE_ID)
            .expect("bundled Kokoro profile");
        kokoro.post_process = ProfilePostProcess {
            enabled: true,
            active_template_id: Some("my-custom-template".into()),
        };
        let kitten = on_disk
            .tts
            .profiles
            .iter_mut()
            .find(|p| p.id == crate::config::tts::KITTEN_DEFAULT_PROFILE_ID)
            .expect("bundled Kitten profile");
        kitten.post_process = ProfilePostProcess {
            enabled: true,
            active_template_id: None,
        };
        save_to(&path, &on_disk).expect("write v8 config");

        let loaded = load_or_default_from(&path);
        assert_eq!(loaded.tts.schema_version, 8);

        let kokoro = loaded
            .tts
            .profiles
            .iter()
            .find(|p| p.id == crate::config::tts::KOKORO_DEFAULT_PROFILE_ID)
            .unwrap();
        assert!(kokoro.post_process.enabled);
        assert_eq!(
            kokoro.post_process.active_template_id.as_deref(),
            Some("my-custom-template")
        );
        let kitten = loaded
            .tts
            .profiles
            .iter()
            .find(|p| p.id == crate::config::tts::KITTEN_DEFAULT_PROFILE_ID)
            .unwrap();
        assert!(kitten.post_process.enabled);
        assert_eq!(kitten.post_process.active_template_id, None);

        // Every other profile keeps the untouched default.
        for profile in loaded.tts.profiles.iter().filter(|p| {
            p.id != crate::config::tts::KOKORO_DEFAULT_PROFILE_ID
                && p.id != crate::config::tts::KITTEN_DEFAULT_PROFILE_ID
        }) {
            assert!(
                !profile.post_process.enabled,
                "profile {} post_process was rewritten on load",
                profile.id
            );
            assert_eq!(
                profile.post_process.active_template_id, None,
                "profile {} post_process template was rewritten on load",
                profile.id
            );
        }
        assert!(!loaded.post_process.enabled);

        discard_temp_config(&path);
    }
}
