//! PR-25. The intervene command with a fixtures directory.

mod common;

use std::path::PathBuf;

use common::{root, run_cli_at_root, temp_dir, text_of};
use westering::pyjson::{self, Value};

fn fixtures() -> PathBuf {
    root().join("fixtures")
}

#[test]
fn intervene_with_fixtures_records_the_judgments() {
    let fixtures_text = fixtures().to_string_lossy().into_owned();
    let out = run_cli_at_root(&[
        "intervene",
        "--scenario",
        "loop-needs-sharper",
        "--fixtures",
        &fixtures_text,
    ]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let record = pyjson::parse(&out.stdout).expect("record parses");
    let interventions = common::list(&record, "interventions");
    let first = &interventions[0];
    let second = &interventions[1];
    assert_eq!(first.get("severity"), Some(&Value::Float(0.82)));
    assert_eq!(text_of(first, "recovery"), "still_stalled");
    assert!(second.get("severity").is_none());
    assert_eq!(text_of(second, "recovery"), "recovering");
    assert_eq!(text_of(&record, "outcome"), "cleared");
}

#[test]
fn intervene_without_fixtures_carries_no_judgment_fields() {
    let out = run_cli_at_root(&["intervene", "--scenario", "loop-needs-sharper"]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let record = pyjson::parse(&out.stdout).expect("record parses");
    for intervention in common::list(&record, "interventions") {
        assert!(intervention.get("severity").is_none());
        assert!(intervention.get("recovery").is_none());
    }
}

#[test]
fn intervene_with_a_missing_fixtures_directory_exits_2() {
    let dir = temp_dir("missing-fixtures");
    let missing = dir.join("nope").to_string_lossy().into_owned();
    let out = run_cli_at_root(&[
        "intervene",
        "--scenario",
        "loop-needs-sharper",
        "--fixtures",
        &missing,
    ]);
    assert_eq!(out.code, 2);
    assert!(!out.stderr.contains("Traceback"));
    assert_eq!(out.stderr.matches('\n').count(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_fixture_field_python_calls_not_printable_is_named_as_python_names_it() {
    // PR-37 and PR-38. The envelope refusal names an extra
    // field with the Python repr of its name, and Python's
    // repr escapes every code point str.isprintable calls not
    // printable, the unassigned points and the private use
    // points included. Each expected line below is the line
    // the Python tool prints for the same fixture, byte for
    // byte.
    let samples = [
        ("x\u{378}y", "'x\\u0378y'"),
        ("x\u{E000}y", "'x\\ue000y'"),
        ("x\u{200B}y", "'x\\u200by'"),
        ("x\u{2028}y", "'x\\u2028y'"),
        ("x\u{A0}y", "'x\\xa0y'"),
        ("café-λ", "'café-λ'"),
    ];
    for (name, named) in samples {
        let dir = temp_dir("not-printable-field");
        let pair = Value::Object(vec![(name.to_string(), Value::int(1))]);
        std::fs::write(dir.join("probe.json"), pyjson::dumps_compact(&pair)).expect("file written");
        let dir_text = dir.to_string_lossy().into_owned();
        let out = run_cli_at_root(&[
            "intervene",
            "--scenario",
            "loop-needs-sharper",
            "--fixtures",
            &dir_text,
        ]);
        assert_eq!(out.code, 2, "field {:?}", name);
        assert_eq!(
            out.stderr,
            format!(
                "error: fixture probe.json carries the top level field {}, which the fixture envelope does not carry\n",
                named
            ),
            "field {:?}",
            name
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn intervene_with_a_broken_fixture_exits_2() {
    let dir = temp_dir("broken-fixture-cli");
    let text = std::fs::read_to_string(fixtures().join("recovery-loop-needs-sharper-nudge.json"))
        .expect("fixture reads");
    let mut pair = pyjson::parse(&text).expect("fixture parses");
    let mut response = pair.get("response").expect("response stands").clone();
    response.set("confidence", Value::int(7));
    pair.set("response", response);
    std::fs::write(dir.join("broken.json"), pyjson::dumps_compact(&pair)).expect("file written");
    let dir_text = dir.to_string_lossy().into_owned();
    let out = run_cli_at_root(&[
        "intervene",
        "--scenario",
        "loop-needs-sharper",
        "--fixtures",
        &dir_text,
    ]);
    assert_eq!(out.code, 2);
    assert!(!out.stderr.contains("Traceback"));
    assert!(out.stderr.contains("broken.json"));
    std::fs::remove_dir_all(&dir).ok();
}
