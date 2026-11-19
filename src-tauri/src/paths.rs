// App data directory names and the one-time legacy migration.
//
// v0.3.0 shipped with data still under `CopySpeak`; the rename program moved the
// app identity to `CopySpeech` and the owner approved relocating the data roots.
// Migration moves whole directories (Roaming, Documents) and merges children
// (Local, whose `CopySpeech` name is already the payload dir of the installed
// app). It never overwrites: an existing destination wins, and a locked source
// only warns, so a failed move can never destroy data.
pub const APP_DIR: &str = "CopySpeech";
pub const LEGACY_APP_DIR: &str = "CopySpeak";

fn base_dir(get: fn() -> Option<std::path::PathBuf>) -> std::path::PathBuf {
    get().unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// Migrate every legacy data root. Call once at startup, before logging.
/// Windows resolves `config_dir` and `data_dir` to the same `%APPDATA%` and
/// `cache_dir` and `data_local_dir` to the same `%LOCALAPPDATA%`, so duplicate
/// bases are deduplicated by canonicalized path.
pub fn migrate_legacy_data_dirs() {
    let bases = [
        base_dir(dirs::config_dir),
        base_dir(dirs::data_dir),
        base_dir(dirs::cache_dir),
        base_dir(dirs::data_local_dir),
        base_dir(dirs::document_dir),
    ];
    let mut seen: Vec<std::path::PathBuf> = Vec::new();
    for base in bases {
        let key = base.canonicalize().unwrap_or_else(|_| base.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        // Local is the payload dir (exe, .env, native host): its legacy engine
        // and cache folders must merge into the existing directory instead of
        // renaming over it.
        let merge_children = dirs::data_local_dir().map(|d| d == base).unwrap_or(false);
        migrate_root(&base, merge_children);
    }
}

/// Move one legacy root into its new name. With `merge_children`, each child of
/// the legacy dir moves individually (the destination exists), and children
/// that already exist in the destination stay there — the new copy wins.
fn migrate_root(base: &std::path::Path, merge_children: bool) {
    let old = base.join(LEGACY_APP_DIR);
    let new = base.join(APP_DIR);
    if !old.is_dir() {
        return;
    }
    if !merge_children {
        if new.exists() {
            return; // already migrated or independently created; never overwrite
        }
        match std::fs::rename(&old, &new) {
            Ok(()) => log::info!(
                "[paths] migrated data dir {} -> {}",
                old.display(),
                new.display()
            ),
            Err(e) => log::warn!(
                "[paths] could not migrate {} ({}); keeping legacy dir",
                old.display(),
                e
            ),
        }
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&new) {
        log::warn!("[paths] could not create {}: {}", new.display(), e);
        return;
    }
    let entries = match std::fs::read_dir(&old) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("[paths] could not read {}: {}", old.display(), e);
            return;
        }
    };
    for entry in entries.flatten() {
        let target = new.join(entry.file_name());
        if target.exists() {
            continue; // destination wins; legacy copy stays for manual cleanup
        }
        if let Err(e) = std::fs::rename(entry.path(), &target) {
            log::warn!("[paths] could not move {}: {}", entry.path().display(), e);
        }
    }
    if std::fs::read_dir(&old)
        .map(|mut i| i.next().is_none())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(&old);
    }
}

/// Rewrite legacy directory prefixes inside persisted strings (config embeds
/// absolute engine-wrapper paths). Anchored on path separators, so provider
/// keys and URLs never match. The wrapper file name itself is untouched —
/// legacy-named copies keep working until the engine reinstalls.
pub fn rewrite_legacy_dir_prefixes(text: &str) -> String {
    // config.json stores Windows paths JSON-escaped: the raw file text holds
    // doubled backslashes, in-memory strings hold single ones. Handle both.
    text.replace("\\\\CopySpeak\\\\", "\\\\CopySpeech\\\\")
        .replace(&format!("\\{LEGACY_APP_DIR}\\"), &format!("\\{APP_DIR}\\"))
        .replace(&format!("/{LEGACY_APP_DIR}/"), &format!("/{APP_DIR}/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn temp_base(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("copyspeech-paths-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn renames_whole_root_when_destination_absent() {
        let base = temp_base("rename");
        let old = base.join(LEGACY_APP_DIR);
        fs::create_dir_all(old.join("logs")).unwrap();
        fs::write(old.join("config.json"), "{}").unwrap();
        migrate_root(&base, false);
        assert!(base.join(APP_DIR).join("config.json").exists());
        assert!(!old.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn whole_root_never_overwrites_existing_destination() {
        let base = temp_base("noover");
        fs::create_dir_all(base.join(APP_DIR)).unwrap();
        fs::write(base.join(APP_DIR).join("config.json"), "{\"new\":1}").unwrap();
        let old = base.join(LEGACY_APP_DIR);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("config.json"), "{\"old\":1}").unwrap();
        migrate_root(&base, false);
        let text = fs::read_to_string(base.join(APP_DIR).join("config.json")).unwrap();
        assert_eq!(text, "{\"new\":1}");
        assert!(old.exists(), "legacy dir must survive a blocked move");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn merge_moves_children_and_keeps_destination_copies() {
        let base = temp_base("merge");
        let new = base.join(APP_DIR);
        let old = base.join(LEGACY_APP_DIR);
        fs::create_dir_all(new.join("engines")).unwrap();
        fs::create_dir_all(old.join("engines")).unwrap();
        fs::create_dir_all(old.join("speech-cache")).unwrap();
        fs::write(old.join("engines").join("manifest.json"), "{}").unwrap();
        fs::write(old.join("speech-cache").join("a.audio"), "x").unwrap();
        migrate_root(&base, true);
        assert!(
            new.join("speech-cache").join("a.audio").exists(),
            "fresh child moves"
        );
        assert!(
            !old.join("speech-cache").exists(),
            "moved child leaves the legacy dir"
        );
        assert!(
            old.join("engines").exists(),
            "existing destination child wins; the legacy copy stays for manual cleanup"
        );
        assert!(new.join("engines").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn noop_without_legacy_dir() {
        let base = temp_base("noop");
        migrate_root(&base, false);
        assert!(!base.join(APP_DIR).exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rewrites_only_separator_anchored_prefixes() {
        // Raw config.json bytes: Windows separators are JSON-escaped, so a real
        // file holds exactly two backslashes per segment.
        let text = r#"{"wrapper":"C:\\Users\\u\\AppData\\Local\\CopySpeak\\engines\\piper\\scripts\\copyspeak-piper.py","key":"abcCopySpeakdef","url":"https://example.com/CopySpeak/x"}"#;
        let out = rewrite_legacy_dir_prefixes(text);
        assert!(
            out.contains("Local\\\\CopySpeech\\\\engines"),
            "dir prefix rewritten"
        );
        assert!(
            !out.contains("\\\\CopySpeak\\\\"),
            "escaped legacy prefix gone"
        );
        assert!(out.contains("copyspeak-piper.py"), "file name untouched");
        assert!(out.contains("abcCopySpeakdef"), "bare substring untouched");
        assert!(
            out.contains("example.com/CopySpeech/x"),
            "slash-anchored URL path rewritten"
        );
    }
}
