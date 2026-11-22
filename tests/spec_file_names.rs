//! Guard test: every file in `docs/specs/` other than `README.md` is named `YYYY-MM-DD_short-name.md`.

use std::fs;
use std::path::Path;

#[test]
fn spec_file_names_follow_the_pattern() {
    let specs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/specs");
    let entries = fs::read_dir(&specs_dir).expect("docs/specs should exist in the repo");

    let mut bad_names = Vec::new();
    for entry in entries {
        let entry = entry.expect("a directory entry in docs/specs should be readable");
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "README.md" {
            continue;
        }
        if !is_valid_spec_name(&name) {
            bad_names.push(name);
        }
    }

    assert!(
        bad_names.is_empty(),
        "spec files must be named YYYY-MM-DD_short-name.md (see docs/specs/README.md): {:?}",
        bad_names
    );
}

/// Matches `^\d{4}-\d{2}-\d{2}_[a-z0-9]+(-[a-z0-9]+)*\.md$`, spelled out to avoid a regex dependency.
fn is_valid_spec_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".md") else {
        return false;
    };
    let Some((date, words)) = stem.split_once('_') else {
        return false;
    };
    is_valid_date(date) && is_valid_words(words)
}

/// `YYYY-MM-DD` in shape only: digits with dashes at positions 4 and 7.
fn is_valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        let should_be_dash = index == 4 || index == 7;
        if should_be_dash {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}

/// Lowercase letters and digits, in words joined by single dashes.
fn is_valid_words(words: &str) -> bool {
    for word in words.split('-') {
        if word.is_empty() {
            return false;
        }
        for character in word.chars() {
            if !(character.is_ascii_lowercase() || character.is_ascii_digit()) {
                return false;
            }
        }
    }
    true
}

#[test]
fn accepts_good_names() {
    assert!(is_valid_spec_name("2026-11-18_device-names.md"));
    assert!(is_valid_spec_name("2026-01-02_m2.md"));
}

#[test]
fn rejects_bad_names() {
    assert!(!is_valid_spec_name("2026-11-18-device-names.md"));
    assert!(!is_valid_spec_name("2026-11-18_Device-Names.md"));
    assert!(!is_valid_spec_name("2026-11-18_device_names.md"));
    assert!(!is_valid_spec_name("2026-11-18_device--names.md"));
    assert!(!is_valid_spec_name("2026-11-18_.md"));
    assert!(!is_valid_spec_name("26-11-18_device-names.md"));
    assert!(!is_valid_spec_name("2026-11-18_device-names.txt"));
}
