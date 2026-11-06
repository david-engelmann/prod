//! PR-44 to PR-48. The fork rung, its order, and its seam.

mod common;

use std::cell::RefCell;
use std::path::PathBuf;

use common::{assert_py_eq, list, root, run_cli_at_root, temp_dir, text_of, value, without};
use westering::judgment::{FixturePlayer, JudgmentSource, validate_fixture};
use westering::ladder::{apply_rung, build_intervention, next_rung};
use westering::pyjson::{self, Value};
use westering::sandbox::{run_session, scenario_names};

const GOAL: &str = "Find the config file and report its path";

fn fixtures() -> PathBuf {
    root().join("fixtures")
}

fn detection() -> Value {
    value(
        r#"{
        "trace_id": "trace-ladder-001",
        "shape": "looping",
        "pattern": ["grep(pattern=config, path=src)"],
        "evidence_span": {"start_step": 1, "end_step": 4},
        "detail": "call pattern of length 1 repeated 4 times with no progress"
        }"#,
    )
}

fn history_through(rungs: &[&str]) -> Vec<Value> {
    let mut history: Vec<Value> = Vec::new();
    let mut step = 4;
    for rung in rungs {
        let record = apply_rung(rung, &detection(), GOAL, Value::int(step), &history, None)
            .expect("rung builds");
        history.push(record);
        step += 5;
    }
    history
}

fn rungs_of(record: &Value) -> Vec<String> {
    list(record, "interventions")
        .iter()
        .map(|item| text_of(item, "rung").to_string())
        .collect()
}

#[test]
fn after_all_three_lower_rungs_the_next_rung_is_fork() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        Some("fork".to_string())
    );
}

#[test]
fn the_ladder_is_spent_once_the_fork_has_been_tried() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask", "fork"]);
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        None
    );
}

#[test]
fn fork_record_matches_a_hand_written_expectation() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    let record = apply_rung("fork", &detection(), GOAL, Value::int(19), &history, None)
        .expect("fork builds");
    let expected = value(
        r#"{
        "rung": "fork",
        "fired_by": {
            "trace_id": "trace-ladder-001",
            "shape": "looping",
            "pattern": ["grep(pattern=config, path=src)"],
            "evidence_span": {"start_step": 1, "end_step": 4}
        },
        "message": "Fork. Your previous approach stalled. The pattern was grep(pattern=config, path=src) in the shape looping, and it made no progress, and neither a nudge nor a sharper prompt nor a smaller subtask cleared it. The goal is Find the config file and report its path. Split the work from here into separate attempts, and run each attempt toward the goal on its own, and keep to the attempt that makes progress.",
        "at_step": 19
        }"#,
    );
    assert_py_eq(&record, &expected);
}

#[test]
fn fork_record_carries_the_same_fields_as_the_earlier_rungs() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    let record = apply_rung("fork", &detection(), GOAL, Value::int(19), &history, None)
        .expect("fork builds");
    let mut keys: Vec<&str> = match &record {
        Value::Object(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
        _ => panic!("record is an object"),
    };
    keys.sort();
    assert_eq!(keys, vec!["at_step", "fired_by", "message", "rung"]);
}

#[test]
fn fork_message_names_the_pattern_and_the_shape_and_restates_the_goal() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    let record = apply_rung("fork", &detection(), GOAL, Value::int(19), &history, None)
        .expect("fork builds");
    let message = text_of(&record, "message");
    assert!(message.contains("grep(pattern=config, path=src)"));
    assert!(message.contains("looping"));
    assert!(message.contains(GOAL));
}

#[test]
fn fork_message_records_the_failed_rungs_and_directs_the_split() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    let record = apply_rung("fork", &detection(), GOAL, Value::int(19), &history, None)
        .expect("fork builds");
    let message = text_of(&record, "message");
    assert!(
        message.contains("neither a nudge nor a sharper prompt nor a smaller subtask cleared it")
    );
    assert!(message.contains("Split the work from here into separate attempts"));
    assert!(message.contains("keep to the attempt that makes progress"));
}

#[test]
fn fork_message_follows_the_detection_it_fired_on() {
    let mut other = detection();
    other.set("shape", Value::Str("tool_fixation".to_string()));
    other.set(
        "pattern",
        value(r#"["grep", "grep(pattern=settings, path=src)"]"#),
    );
    other.set(
        "evidence_span",
        value(r#"{"start_step": 2, "end_step": 9}"#),
    );
    let mut history: Vec<Value> = Vec::new();
    let mut step = 4;
    for rung in ["nudge", "sharper_prompt", "smaller_subtask"] {
        let record = apply_rung(
            rung,
            &other,
            "Sort the inbox",
            Value::int(step),
            &history,
            None,
        )
        .expect("rung builds");
        history.push(record);
        step += 5;
    }
    let record = apply_rung(
        "fork",
        &other,
        "Sort the inbox",
        Value::int(21),
        &history,
        None,
    )
    .expect("fork builds");
    let message = text_of(&record, "message");
    assert!(message.contains("grep then grep(pattern=settings, path=src)"));
    assert!(message.contains("tool_fixation"));
    assert!(message.contains("The goal is Sort the inbox."));
    assert!(!message.contains("Find the config file"));
}

#[test]
fn fork_cannot_fire_with_no_lower_rung_tried() {
    let err = apply_rung("fork", &detection(), GOAL, Value::int(4), &[], None)
        .expect_err("fork is refused");
    assert_eq!(
        err.0,
        "fork cannot fire before nudge and sharper_prompt and smaller_subtask for the same stall"
    );
    assert!(build_intervention("fork", &detection(), GOAL, Value::int(4), &[], None).is_err());
}

#[test]
fn fork_cannot_fire_with_only_the_nudge_tried() {
    let history = history_through(&["nudge"]);
    let err = apply_rung("fork", &detection(), GOAL, Value::int(9), &history, None)
        .expect_err("fork is refused");
    assert_eq!(
        err.0,
        "fork cannot fire before sharper_prompt and smaller_subtask for the same stall"
    );
}

#[test]
fn fork_cannot_fire_with_the_smaller_subtask_untried() {
    let history = history_through(&["nudge", "sharper_prompt"]);
    let err = build_intervention("fork", &detection(), GOAL, Value::int(14), &history, None)
        .expect_err("fork is refused");
    assert_eq!(
        err.0,
        "fork cannot fire before smaller_subtask for the same stall"
    );
    assert!(apply_rung("fork", &detection(), GOAL, Value::int(14), &history, None).is_err());
}

#[test]
fn fork_cannot_fire_twice_for_the_same_stall() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask", "fork"]);
    let err = apply_rung("fork", &detection(), GOAL, Value::int(24), &history, None)
        .expect_err("fork is refused");
    assert_eq!(err.0, "fork has already been tried for this stall");
    assert!(
        build_intervention("fork", &detection(), GOAL, Value::int(24), &history, None).is_err()
    );
}

#[test]
fn lower_rungs_tried_on_a_different_stall_do_not_unlock_the_fork() {
    let mut other = detection();
    other.set("shape", Value::Str("tool_fixation".to_string()));
    other.set("pattern", value(r#"["grep"]"#));
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    assert_eq!(
        next_rung(&other, &history).expect("lookup works"),
        Some("nudge".to_string())
    );
    assert!(apply_rung("fork", &other, GOAL, Value::int(19), &history, None).is_err());
}

#[test]
fn a_trace_change_alone_does_not_make_a_new_stall_for_the_fork() {
    // Stall identity is the shape and the pattern under PR-6.
    // A history whose records fired on the same shape and
    // pattern under another trace identifier still arms the
    // fork, and the fork is refused as already tried once it
    // stands in that history, because the trace never enters
    // the match, as Round 4 Note N1 holds.
    let mut renamed = detection();
    renamed.set("trace_id", Value::Str("trace-other-002".to_string()));
    let mut history: Vec<Value> = Vec::new();
    let mut step = 4;
    for rung in ["nudge", "sharper_prompt", "smaller_subtask"] {
        let record = apply_rung(rung, &renamed, GOAL, Value::int(step), &history, None)
            .expect("rung builds");
        history.push(record);
        step += 5;
    }
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        Some("fork".to_string())
    );
}

#[test]
fn fork_record_refuses_a_detection_missing_a_field() {
    let broken = without(&detection(), "pattern");
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    let err = apply_rung("fork", &broken, GOAL, Value::int(19), &history, None)
        .expect_err("the broken detection is refused");
    assert!(err.0.contains("pattern"));
}

#[test]
fn kill_is_not_a_rung_of_this_ladder() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask", "fork"]);
    let err = build_intervention("kill", &detection(), GOAL, Value::int(24), &history, None)
        .expect_err("kill is refused");
    assert_eq!(err.0, "unknown rung kill");
    assert!(apply_rung("kill", &detection(), GOAL, Value::int(24), &history, None).is_err());
}

#[test]
fn a_trusted_severity_score_is_recorded_on_the_fork_record() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    let plain = apply_rung("fork", &detection(), GOAL, Value::int(19), &history, None)
        .expect("fork builds");
    let scored = apply_rung(
        "fork",
        &detection(),
        GOAL,
        Value::int(19),
        &history,
        Some(Value::Float(0.5)),
    )
    .expect("fork builds");
    assert_py_eq(
        scored.get("severity").expect("severity stands"),
        &Value::Float(0.5),
    );
    assert_py_eq(&without(&scored, "severity"), &plain);
}

fn recovery_pair(rung: &str) -> Value {
    value(&format!(
        r#"{{
        "synthetic": true,
        "judgment_point": "recovery",
        "request": {{
            "trace_id": "sandbox-loop-needs-fork",
            "rung": "{}",
            "shape": "looping",
            "pattern": ["grep(pattern=config, path=src)"],
            "events": [{{"step": 12, "tool": "read", "call": "read(path=src/config.py)", "progress": true}}]
        }},
        "response": {{"kind": "abstention"}}
        }}"#,
        rung
    ))
}

#[test]
fn a_recovery_request_naming_fork_validates() {
    let pair = recovery_pair("fork");
    let validated = validate_fixture(&pair, "probe fixture").expect("fork request validates");
    assert_py_eq(&validated, &pair);
}

#[test]
fn a_recovery_request_naming_kill_is_refused() {
    let err = validate_fixture(&recovery_pair("kill"), "probe fixture")
        .expect_err("kill request is refused");
    assert!(err.0.contains("not a ladder rung"), "got {}", err.0);
}

#[test]
fn the_fork_scenario_is_in_the_committed_scenario_set() {
    assert!(scenario_names().contains(&"loop-needs-fork"));
}

#[test]
fn agent_that_ignores_three_lower_rungs_clears_on_the_fork() {
    let record = run_session("loop-needs-fork", None).expect("session runs");
    assert_eq!(record.get("stall_cleared"), Some(&Value::Bool(true)));
    assert_eq!(text_of(&record, "outcome"), "cleared");
    assert_eq!(
        rungs_of(&record),
        vec![
            "nudge".to_string(),
            "sharper_prompt".to_string(),
            "smaller_subtask".to_string(),
            "fork".to_string()
        ]
    );
    assert_eq!(text_of(&record, "cleared_by"), "fork");
    let interventions = list(&record, "interventions");
    let last = interventions.last().expect("a last intervention stands");
    assert!((common::int_of(last, "at_step") as usize) < list(&record, "events").len());
}

#[test]
fn sessions_that_clear_before_the_fork_keep_their_earlier_shape() {
    let record = run_session("loop-clears-on-nudge", None).expect("session runs");
    assert_eq!(rungs_of(&record), vec!["nudge".to_string()]);
    assert_eq!(list(&record, "events").len(), 6);
    let record = run_session("loop-needs-sharper", None).expect("session runs");
    assert_eq!(
        rungs_of(&record),
        vec!["nudge".to_string(), "sharper_prompt".to_string()]
    );
    assert_eq!(list(&record, "events").len(), 9);
    let record = run_session("fixation-clears-on-nudge", None).expect("session runs");
    assert_eq!(rungs_of(&record), vec!["nudge".to_string()]);
    assert_eq!(list(&record, "events").len(), 7);
    let record = run_session("loop-needs-subtask", None).expect("session runs");
    assert_eq!(
        rungs_of(&record),
        vec![
            "nudge".to_string(),
            "sharper_prompt".to_string(),
            "smaller_subtask".to_string()
        ]
    );
    assert_eq!(list(&record, "events").len(), 12);
}

struct Capture {
    captured: RefCell<Option<Value>>,
}

impl JudgmentSource for Capture {
    fn judge(&self, point: &str, request: &Value) -> Option<Value> {
        if point == "recovery"
            && matches!(request.get("rung"), Some(Value::Str(rung)) if rung == "fork")
        {
            *self.captured.borrow_mut() = Some(request.clone());
        }
        None
    }
}

#[test]
fn committed_fork_fixture_request_is_the_builders_output() {
    let text = std::fs::read_to_string(fixtures().join("recovery-loop-needs-fork-fork.json"))
        .expect("fixture reads");
    let pair = pyjson::parse(&text).expect("fixture parses");
    let capture = Capture {
        captured: RefCell::new(None),
    };
    run_session("loop-needs-fork", Some(&capture)).expect("session runs");
    let captured = capture
        .captured
        .borrow()
        .clone()
        .expect("the fork recovery request was asked");
    assert_py_eq(&captured, pair.get("request").expect("request stands"));
    let request = pair.get("request").expect("request stands");
    assert_eq!(text_of(request, "rung"), "fork");
    let response = pair.get("response").expect("response stands");
    assert_eq!(text_of(response, "kind"), "choice");
}

#[test]
fn trusted_fork_recovery_is_recorded_on_the_fork_record() {
    let player = FixturePlayer::new(&fixtures()).expect("player loads");
    let record = run_session("loop-needs-fork", Some(&player)).expect("session runs");
    let interventions = list(&record, "interventions");
    let fourth = &interventions[3];
    assert_eq!(text_of(fourth, "rung"), "fork");
    assert_eq!(text_of(fourth, "recovery"), "recovering");
    let plain = run_session("loop-needs-fork", None).expect("session runs");
    let plain_fourth = &list(&plain, "interventions")[3];
    assert_py_eq(&without(fourth, "recovery"), plain_fourth);
}

#[test]
fn an_untrusted_fork_recovery_answer_records_nothing() {
    let text = std::fs::read_to_string(fixtures().join("recovery-loop-needs-fork-fork.json"))
        .expect("fixture reads");
    let mut pair = pyjson::parse(&text).expect("fixture parses");
    let mut response = pair.get("response").expect("response stands").clone();
    response.set("confidence", Value::Float(0.59));
    let mut probabilities = response
        .get("probabilities")
        .expect("probabilities stand")
        .clone();
    probabilities.set("recovering", Value::Float(0.59));
    probabilities.set("still_stalled", Value::Float(0.3));
    probabilities.set("getting_worse", Value::Float(0.11));
    response.set("probabilities", probabilities);
    pair.set("response", response);
    let dir = temp_dir("fork-quiet-fixture");
    let path = dir.join("recovery-loop-needs-fork-fork.json");
    common::write_json(&path, &pair);
    let player = FixturePlayer::new(&dir).expect("player loads");
    let judged = run_session("loop-needs-fork", Some(&player)).expect("session runs");
    let plain = run_session("loop-needs-fork", None).expect("session runs");
    let fourth = &list(&judged, "interventions")[3];
    assert!(fourth.get("recovery").is_none());
    assert_py_eq(&judged, &plain);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn judgments_never_change_the_fork_session() {
    let player = FixturePlayer::new(&fixtures()).expect("player loads");
    let plain = run_session("loop-needs-fork", None).expect("session runs");
    let judged = run_session("loop-needs-fork", Some(&player)).expect("session runs");
    assert_eq!(rungs_of(&judged), rungs_of(&plain));
    assert_py_eq(
        judged.get("events").expect("events stand"),
        plain.get("events").expect("events stand"),
    );
    assert_py_eq(
        judged.get("detections").expect("detections stand"),
        plain.get("detections").expect("detections stand"),
    );
    assert_eq!(text_of(&judged, "outcome"), text_of(&plain, "outcome"));
}

#[test]
fn intervene_fires_the_fork_end_to_end() {
    let out = run_cli_at_root(&["intervene", "--scenario", "loop-needs-fork"]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let record = pyjson::parse(&out.stdout).expect("record parses");
    assert_eq!(
        rungs_of(&record),
        vec![
            "nudge".to_string(),
            "sharper_prompt".to_string(),
            "smaller_subtask".to_string(),
            "fork".to_string()
        ]
    );
    assert_eq!(text_of(&record, "cleared_by"), "fork");
    assert_eq!(text_of(&record, "outcome"), "cleared");
}

#[test]
fn intervene_with_fixtures_records_the_fork_recovery() {
    let fixtures_text = fixtures().to_string_lossy().into_owned();
    let out = run_cli_at_root(&[
        "intervene",
        "--scenario",
        "loop-needs-fork",
        "--fixtures",
        &fixtures_text,
    ]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let record = pyjson::parse(&out.stdout).expect("record parses");
    let interventions = list(&record, "interventions");
    assert_eq!(text_of(&interventions[3], "recovery"), "recovering");
}
