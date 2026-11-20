// Tests for the uv resolver in `install.rs`. Kept in a sibling file (AGENTS.md).

use super::*;
use std::fs;
use std::path::Path;

/// A clean temp folder for one test. The caller removes it.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("copyspeech-uv-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp folder");
    dir
}

#[test]
fn find_uv_in_skips_missing_paths_and_folders() {
    let root = scratch("skips");
    let missing = root.join("missing").join("uv");
    let folder_named_uv = root.join("folder").join("uv");
    fs::create_dir_all(&folder_named_uv).unwrap();
    let real = root.join("real").join("uv");
    fs::create_dir_all(real.parent().unwrap()).unwrap();
    fs::write(&real, b"").unwrap();

    assert_eq!(
        find_uv_in(&[missing, folder_named_uv, real.clone()]),
        Some(real)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn each_candidate_under_temp_home_is_found_alone() {
    let root = scratch("each");
    let home = root.join("home");
    let local = root.join("local");
    // Only candidates under the temp folders can be placed here. Absolute
    // system paths such as /usr/bin/uv are covered by the parity test below.
    let placeable: Vec<PathBuf> = uv_candidates_in(Some(&home), Some(&local))
        .into_iter()
        .filter(|c| c.starts_with(&home) || c.starts_with(&local))
        .collect();
    assert!(!placeable.is_empty());
    assert_eq!(find_uv_in(&placeable), None);

    for candidate in &placeable {
        fs::create_dir_all(candidate.parent().unwrap()).unwrap();
        fs::write(candidate, b"").unwrap();
        assert_eq!(
            find_uv_in(&placeable),
            Some(candidate.clone()),
            "candidate {}",
            candidate.display()
        );
        fs::remove_file(candidate).unwrap();
    }
    let _ = fs::remove_dir_all(&root);
}

/// Every folder the resolver checks must appear in the install scripts, so the
/// app and the scripts agree on where uv can be.
#[test]
fn install_scripts_list_every_uv_folder() {
    let home = Path::new("/__copyspeech_home__");
    let local = Path::new("/__copyspeech_local__");
    let scripts: Vec<(&str, &str)> = if cfg!(target_os = "windows") {
        vec![
            (
                "scripts/lib/copyspeech-engine-install.ps1",
                include_str!("../../../scripts/lib/copyspeech-engine-install.ps1"),
            ),
            (
                "scripts/install-uv.ps1",
                include_str!("../../../scripts/install-uv.ps1"),
            ),
        ]
    } else {
        vec![(
            "scripts/lib/copyspeech-engine-install.sh",
            include_str!("../../../scripts/lib/copyspeech-engine-install.sh"),
        )]
    };
    for candidate in uv_candidates_in(Some(home), Some(local)) {
        let dir = candidate.parent().expect("candidate has a folder");
        let fragment = dir
            .strip_prefix(home)
            .or_else(|_| dir.strip_prefix(local))
            .unwrap_or(dir)
            .to_string_lossy()
            .into_owned();
        for (name, text) in &scripts {
            assert!(text.contains(&fragment), "{name} does not list {fragment}");
        }
    }
}

#[test]
fn find_uv_on_path_finds_uv_outside_known_folders() {
    let root = scratch("on-path");
    let shims = root.join("scoop").join("shims");
    fs::create_dir_all(&shims).expect("create shims folder");
    let name = if cfg!(windows) { "uv.exe" } else { "uv" };
    let uv = shims.join(name);
    fs::write(&uv, b"").expect("write fake uv");
    let path = std::env::join_paths([root.join("missing"), shims]).expect("join PATH");
    assert_eq!(find_uv_on_path(&path), Some(uv));
    let _ = fs::remove_dir_all(&root);
}
