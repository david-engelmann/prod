//! End to end CLI tests for detect and intervene.

mod common;

use common::{root, run_cli_at_root, temp_dir, text_of};
use prod::pyjson;

#[test]
fn detect_looping_example_matches_the_committed_output() {
    let out = run_cli_at_root(&["detect", "--trace", "examples/looping_trace.json"]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let committed = std::fs::read_to_string(root().join("examples/looping_detections.json"))
        .expect("committed output reads");
    assert_eq!(out.stdout, committed);
}

#[test]
fn detect_fixation_example_matches_the_committed_output() {
    let out = run_cli_at_root(&["detect", "--trace", "examples/fixation_trace.json"]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let committed = std::fs::read_to_string(root().join("examples/fixation_detections.json"))
        .expect("committed output reads");
    assert_eq!(out.stdout, committed);
}

#[test]
fn detect_output_is_byte_identical_across_two_runs() {
    let first = run_cli_at_root(&["detect", "--trace", "examples/looping_trace.json"]);
    let second = run_cli_at_root(&["detect", "--trace", "examples/looping_trace.json"]);
    assert_eq!(first.code, 0);
    assert_eq!(second.code, 0);
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn intervene_runs_a_sandbox_session_end_to_end() {
    let out = run_cli_at_root(&["intervene", "--scenario", "loop-needs-sharper"]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let record = pyjson::parse(&out.stdout).expect("record parses");
    assert_eq!(text_of(&record, "outcome"), "cleared");
    let rungs: Vec<String> = common::list(&record, "interventions")
        .iter()
        .map(|item| text_of(item, "rung").to_string())
        .collect();
    assert_eq!(
        rungs,
        vec!["nudge".to_string(), "sharper_prompt".to_string()]
    );
}

#[test]
fn detect_on_a_missing_trace_exits_2_without_a_traceback() {
    let dir = temp_dir("missing-trace");
    let missing = dir.join("nope.json").to_string_lossy().into_owned();
    let out = run_cli_at_root(&["detect", "--trace", &missing]);
    assert_eq!(out.code, 2);
    assert!(!out.stderr.contains("Traceback"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn detect_on_invalid_json_exits_2_without_a_traceback() {
    let dir = temp_dir("bad-json");
    let bad = dir.join("bad.json");
    std::fs::write(&bad, "{not json").expect("file written");
    let bad_text = bad.to_string_lossy().into_owned();
    let out = run_cli_at_root(&["detect", "--trace", &bad_text]);
    assert_eq!(out.code, 2);
    assert!(!out.stderr.contains("Traceback"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn detect_on_non_monotonic_steps_exits_2_without_a_traceback() {
    let dir = temp_dir("unordered");
    let bad = dir.join("unordered.json");
    let payload = common::value(
        r#"{
        "trace_id": "trace-unordered",
        "goal": "Find the config file and report its path",
        "tools_available": ["grep", "read"],
        "events": [
            {"step": 0, "tool": "grep", "call": "grep(pattern=a, path=src)", "progress": false},
            {"step": 2, "tool": "grep", "call": "grep(pattern=b, path=src)", "progress": false},
            {"step": 1, "tool": "grep", "call": "grep(pattern=c, path=src)", "progress": false}
        ]
        }"#,
    );
    std::fs::write(&bad, pyjson::dumps_compact(&payload)).expect("file written");
    let bad_text = bad.to_string_lossy().into_owned();
    let out = run_cli_at_root(&["detect", "--trace", &bad_text]);
    assert_eq!(out.code, 2);
    assert!(!out.stderr.contains("Traceback"));
    std::fs::remove_dir_all(&dir).ok();
}
