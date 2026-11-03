//! PR-8, PR-10, PR-11, PR-18. Sandbox only, deterministic,
//! synthetic.
//!
//! The Python pins read the package's syntax tree for forbidden
//! capabilities and for imports outside the standard library.
//! The port re-expresses them against the crate. The manifest
//! pin holds the dependency list to the adopted tool crates,
//! which is the Rust form of the standard library only rule.
//! The capability scan reads the crate's sources with comments
//! and string literals set aside, so prose that merely names a
//! capability is not a finding, exactly as prose was not a
//! finding under the syntax tree scan.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::root;

/// One dependency of the crate as the manifest declares it, the
/// section it stands in and the features it carries.
#[derive(Default)]
struct ManifestScan {
    runtime: BTreeMap<String, Vec<String>>,
    dev_deps: BTreeMap<String, Vec<String>>,
    build: BTreeMap<String, Vec<String>>,
    target: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Bucket {
    Runtime,
    Dev,
    Build,
    Target,
    Other,
}

/// The bucket a section header opens, and the dependency it
/// names when the header itself carries one, the bracketed
/// form.
fn section_bucket(header: &str) -> (Bucket, Option<String>) {
    match header {
        "dependencies" => return (Bucket::Runtime, None),
        "dev-dependencies" => return (Bucket::Dev, None),
        "build-dependencies" => return (Bucket::Build, None),
        _ => {}
    }
    for (prefix, bucket) in [
        ("dependencies.", Bucket::Runtime),
        ("dev-dependencies.", Bucket::Dev),
        ("build-dependencies.", Bucket::Build),
    ] {
        if let Some(name) = header.strip_prefix(prefix) {
            return (bucket, Some(name.to_string()));
        }
    }
    if header.starts_with("target.") && header.contains("dependencies") {
        if let Some((_, name)) = header.rsplit_once("dependencies.") {
            return (Bucket::Target, Some(name.to_string()));
        }
        return (Bucket::Target, None);
    }
    (Bucket::Other, None)
}

/// The features list of one dependency spec, read from the spec
/// itself so a feature the ruling does not name is a finding.
fn features_of(spec: &str) -> Vec<String> {
    let Some(at) = spec.find("features") else {
        return Vec::new();
    };
    let rest = &spec[at..];
    let (Some(open), Some(close)) = (rest.find('['), rest.find(']')) else {
        return Vec::new();
    };
    rest[open + 1..close]
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn scan_manifest(manifest: &str) -> ManifestScan {
    let mut scan = ManifestScan::default();
    let record = |scan: &mut ManifestScan, bucket: Bucket, name: String, features: Vec<String>| {
        let map = match bucket {
            Bucket::Runtime => &mut scan.runtime,
            Bucket::Dev => &mut scan.dev_deps,
            Bucket::Build => &mut scan.build,
            Bucket::Target => &mut scan.target,
            Bucket::Other => return,
        };
        map.insert(name, features);
    };
    let mut bucket = Bucket::Other;
    let mut header_entry: Option<String> = None;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            let header = trimmed.trim_matches(['[', ']']);
            let (next, entry) = section_bucket(header);
            bucket = next;
            header_entry = entry;
            if let Some(name) = &header_entry {
                let name = name.clone();
                record(&mut scan, bucket, name, Vec::new());
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, spec)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        match &header_entry {
            Some(name) => {
                if key == "features" {
                    let features = features_of(trimmed);
                    let name = name.clone();
                    record(&mut scan, bucket, name, features);
                }
            }
            None => {
                if bucket != Bucket::Other {
                    record(&mut scan, bucket, key.to_string(), features_of(spec));
                }
            }
        }
    }
    scan
}

#[test]
fn the_crate_depends_on_the_adopted_tool_crates_only() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let scan = scan_manifest(&manifest);
    let names = |map: &BTreeMap<String, Vec<String>>| map.keys().cloned().collect::<Vec<String>>();
    assert_eq!(
        names(&scan.runtime),
        vec![
            "anyhow".to_string(),
            "clap".to_string(),
            "thiserror".to_string()
        ],
        "the runtime dependency list is the adopted tool crates and nothing else"
    );
    assert_eq!(
        scan.runtime.get("clap"),
        Some(&vec!["derive".to_string()]),
        "clap carries the derive feature the ruling names and no other"
    );
    assert_eq!(
        names(&scan.dev_deps),
        vec!["sha2".to_string()],
        "sha2 stands in the dev dependencies, where its one test consumer reads it"
    );
    assert!(scan.build.is_empty(), "no build dependencies");
    assert!(scan.target.is_empty(), "no target dependencies");
    // Positive controls. The scan must be able to fail, so it is
    // shown reading the bracketed header form, distinguishing
    // the sections from each other, and reading a feature the
    // ruling does not name, each of which the assertions above
    // would catch in the real manifest.
    let header_form = "[dependencies.clap]\nversion = \"4\"\nfeatures = [\"derive\", \"env\"]\n";
    let scan = scan_manifest(header_form);
    assert_eq!(
        scan.runtime.get("clap"),
        Some(&vec!["derive".to_string(), "env".to_string()]),
        "the bracketed header form is read with its features"
    );
    let misplaced = "[dependencies]\nsha2 = \"0.10\"\n";
    let scan = scan_manifest(misplaced);
    assert!(
        scan.runtime.contains_key("sha2") && scan.dev_deps.is_empty(),
        "a runtime placement of sha2 reads as runtime, not dev"
    );
    let dev_form = "[dev-dependencies]\nsha2 = \"0.10\"\n";
    let scan = scan_manifest(dev_form);
    assert!(
        scan.dev_deps.contains_key("sha2") && scan.runtime.is_empty(),
        "a dev placement of sha2 reads as dev, not runtime"
    );
}

/// The capability markers the crate's code must never carry,
/// assembled from pieces so this file never carries a whole
/// marker itself.
fn forbidden_markers() -> Vec<String> {
    vec![
        ["std::", "net"].concat(),
        ["Tcp", "Stream"].concat(),
        ["Udp", "Socket"].concat(),
        ["System", "Time"].concat(),
        ["std::process::", "Command"].concat(),
        ["env::", "var"].concat(),
        ["thread::", "sleep"].concat(),
        ["get", "random"].concat(),
        ["::", "rand"].concat(),
    ]
}

/// The code of a Rust source with comments and string literal
/// contents set aside, so prose is never a finding.
fn code_only(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else if bytes[i] == b'"' {
            out.push(b'"');
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(bytes.len());
            out.push(b'"');
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn forbidden_uses(source: &str) -> Vec<String> {
    let code = code_only(source);
    forbidden_markers()
        .into_iter()
        .filter(|marker| code.contains(marker.as_str()))
        .collect()
}

#[test]
fn crate_uses_no_network_wall_clock_or_randomness_capabilities() {
    let mut offenders = Vec::new();
    let src = root().join("src");
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|suffix| suffix == "rs") {
            let source = std::fs::read_to_string(&path).unwrap();
            for marker in forbidden_uses(&source) {
                offenders.push((
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    marker,
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "forbidden capabilities in crate source, {:?}",
        offenders
    );
}

#[test]
fn prose_mentioning_a_capability_is_not_a_forbidden_use() {
    let marker = ["std::process::", "Command"].concat();
    let source = format!(
        "//! Nothing here spawns a {} at all.\n\nfn main() {{}}\n",
        marker
    );
    assert!(forbidden_uses(&source).is_empty());
    let in_string = format!("fn label() -> &'static str {{ \"{}\" }}\n", marker);
    assert!(forbidden_uses(&in_string).is_empty());
}

#[test]
fn a_real_capability_use_is_a_forbidden_use() {
    let spawn = ["let _ = std::process::", "Command::new(\"x\");\n"].concat();
    assert_eq!(forbidden_uses(&spawn).len(), 1);
    let clock = ["let _ = std::time::", "System", "Time::now();\n"].concat();
    assert_eq!(forbidden_uses(&clock).len(), 1);
    let env = ["let _ = std::env::", "var(\"X\");\n"].concat();
    assert_eq!(forbidden_uses(&env).len(), 1);
}

#[test]
fn session_output_is_byte_identical_across_runs() {
    let first = prod::sandbox::run_session("loop-needs-sharper", None).unwrap();
    let second = prod::sandbox::run_session("loop-needs-sharper", None).unwrap();
    let first_text = prod::pyjson::dumps_indent2(&first);
    let second_text = prod::pyjson::dumps_indent2(&second);
    assert_eq!(first_text, second_text);
}

#[test]
fn examples_and_fixtures_are_synthetic() {
    let examples = root().join("examples");
    let mut traces: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&examples).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.ends_with("_trace.json") {
            traces.push(path);
        }
    }
    assert!(!traces.is_empty(), "no example traces found");
    for path in traces {
        let text = std::fs::read_to_string(&path).unwrap();
        let data = prod::pyjson::parse(&text).unwrap();
        assert_eq!(
            data.get("synthetic"),
            Some(&prod::pyjson::Value::Bool(true)),
            "example {} is not marked synthetic",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}
