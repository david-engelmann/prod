//! The setup invariants, re-expressed as Rust tests under
//! PR-42.

mod common;

use common::root;

const CANONICAL_PREFIX_SHA256: &str =
    "63d2f04005dc8eb111d3efca1be55c0911f9ad46bc7cf3c74d9f9908f4a86f11";
const EXPECTED_PREFIX: &str = "PR";
const TEXT_SUFFIXES: [&str; 9] = [
    "md", "yml", "yaml", "py", "toml", "txt", "json", "jsonl", "rs",
];
const SKIP_PARTS: [&str; 8] = [
    ".git",
    "target",
    ".pytest_cache",
    "__pycache__",
    ".venv",
    "venv",
    "node_modules",
    ".tox",
];

#[test]
fn contract_stub_obligations_exist() {
    let text = std::fs::read_to_string(root().join("CONTRACT.md")).unwrap();
    let mut found: Vec<(u32, String)> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(EXPECTED_PREFIX) else {
            continue;
        };
        let Some(rest) = rest.strip_prefix("-S") else {
            continue;
        };
        let Some((number, _)) = rest.split_once('.') else {
            continue;
        };
        if number.len() == 1
            && let Ok(number) = number.parse::<u32>()
        {
            found.push((number, line.to_string()));
        }
    }
    assert!(
        found.len() >= 5,
        "expected at least 5 stub obligations, found {}",
        found.len()
    );
    let mut numbers: Vec<u32> = found.iter().map(|(number, _)| *number).collect();
    numbers.sort_unstable();
    assert_eq!(
        numbers,
        vec![1, 2, 3, 4, 5],
        "stub obligations must be exactly S1 to S5"
    );
    for (_, line) in &found {
        let words = line.split_whitespace().count();
        assert!(words >= 20, "obligation too thin at {} words", words);
    }
}

#[test]
fn agents_canonical_block_present() {
    let text = std::fs::read_to_string(root().join("AGENTS.md")).unwrap();
    assert!(text.contains("repo annex below this line"));
    let prefix = text.split("## Repo annex").next().unwrap();
    let digest = sha256_hex(prefix.as_bytes());
    assert_eq!(
        digest, CANONICAL_PREFIX_SHA256,
        "canonical block above the annex line has drifted"
    );
    let annex = text.split("## Repo annex").nth(1).unwrap();
    assert!(annex.contains("CONTRACT.md"));
}

#[test]
fn readme_status_reports_rounds() {
    let text = std::fs::read_to_string(root().join("README.md")).unwrap();
    let counts = closed_round_counts(&text);
    assert_eq!(
        counts,
        vec!["4".to_string()],
        "README status must report exactly one closed round count"
    );
}

/// Every match of the pinned form, a closed rounds count whose
/// number is not followed by another digit.
fn closed_round_counts(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let marker = "Closed rounds ";
    let mut search = text;
    while let Some(at) = search.find(marker) {
        let after = &search[at + marker.len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        let rest = &after[digits.len()..];
        if !digits.is_empty() && rest.starts_with(" of 100") {
            let tail = &rest[" of 100".len()..];
            if !tail.starts_with(|c: char| c.is_ascii_digit()) {
                found.push(digits);
            }
        }
        search = &search[at + marker.len()..];
    }
    found
}

#[test]
fn no_em_or_en_dash_in_text_files() {
    let em = "\u{2014}".as_bytes();
    let en = "\u{2013}".as_bytes();
    let mut bad = Vec::new();
    let mut stack = vec![root()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root()).unwrap().to_path_buf();
            if relative
                .components()
                .any(|part| SKIP_PARTS.contains(&part.as_os_str().to_str().unwrap_or("")))
            {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let suffix = path
                .extension()
                .map(|s| s.to_str().unwrap_or(""))
                .unwrap_or("");
            if !TEXT_SUFFIXES.contains(&suffix) {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            if contains_bytes(&bytes, em) || contains_bytes(&bytes, en) {
                bad.push(relative);
            }
        }
    }
    assert!(bad.is_empty(), "em or en dash found in {:?}", bad);
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// SHA-256 from the sha2 crate, the adopted tool for it. The
/// AGENTS pin below is its own check, and the known vector test
/// pins the digest itself.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(data);
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

#[test]
fn sha256_matches_a_known_vector() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
