#[cfg(test)]
mod tests {
    use crate::config::*;

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
        if let Some(profile) = config.tts.profiles.iter_mut().find(|p| p.id == config.tts.active_profile_id) {
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
        if let Some(profile) = config.tts.profiles.iter_mut().find(|p| p.id == config.tts.active_profile_id) {
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
    fn test_default_tts_config_has_one_default_profile() {
        let tts = TtsConfig::default();
        assert_eq!(tts.schema_version, 3);
        assert_eq!(tts.active_profile_id, "default");
        assert_eq!(tts.profiles.len(), 4);
        assert_eq!(tts.profiles[0].id, "default");
        assert_eq!(tts.profiles[0].engine, TtsEngine::Edge);
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
            text_processing: crate::config::ProfileTextProcessing::default(),
            engine_options: ProfileEngineOptions::Http(HttpEngineOptions {
                body_template: Some(r#"{"model":"chatterbox"}"#.into()),
                ..HttpEngineOptions::default()
            }),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let back: VoiceProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "p1");
        assert_eq!(back.engine, TtsEngine::Http);
        assert!(back.effects.enabled);
        assert_eq!(back.effects.active_effect, EffectId::WalkieTalkie);
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
}
