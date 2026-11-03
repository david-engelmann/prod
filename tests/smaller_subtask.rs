//! PR-30 to PR-34. The smaller subtask rung, its order, and its
//! seam.

mod common;

use std::cell::RefCell;
use std::path::PathBuf;

use common::{assert_py_eq, list, root, run_cli_at_root, text_of, value, without};
use prod::judgment::{FixturePlayer, JudgmentSource};
use prod::ladder::{apply_rung, build_intervention, next_rung};
use prod::pyjson::{self, Value};
use prod::sandbox::{run_session, scenario_names};

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
fn after_both_lower_rungs_the_next_rung_is_smaller_subtask() {
    let history = history_through(&["nudge", "sharper_prompt"]);
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        Some("smaller_subtask".to_string())
    );
}

#[test]
fn subtask_record_carries_the_same_fields_as_the_earlier_rungs() {
    let history = history_through(&["nudge", "sharper_prompt"]);
    let record = apply_rung(
        "smaller_subtask",
        &detection(),
        GOAL,
        Value::int(14),
        &history,
        None,
    )
    .expect("subtask builds");
    assert_eq!(text_of(&record, "rung"), "smaller_subtask");
    assert_eq!(common::int_of(&record, "at_step"), 14);
    let fired_by = record.get("fired_by").expect("fired_by stands");
    assert_eq!(text_of(fired_by, "trace_id"), "trace-ladder-001");
    assert_eq!(text_of(fired_by, "shape"), "looping");
    assert_py_eq(
        fired_by.get("pattern").expect("pattern stands"),
        &value(r#"["grep(pattern=config, path=src)"]"#),
    );
    assert_py_eq(
        fired_by.get("evidence_span").expect("span stands"),
        &value(r#"{"start_step": 1, "end_step": 4}"#),
    );
    let mut keys: Vec<&str> = match &record {
        Value::Object(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
        _ => panic!("record is an object"),
    };
    keys.sort();
    assert_eq!(keys, vec!["at_step", "fired_by", "message", "rung"]);
}

#[test]
fn subtask_message_names_the_pattern_and_the_shape_and_restates_the_goal() {
    let history = history_through(&["nudge", "sharper_prompt"]);
    let record = apply_rung(
        "smaller_subtask",
        &detection(),
        GOAL,
        Value::int(14),
        &history,
        None,
    )
    .expect("subtask builds");
    let message = text_of(&record, "message");
    assert!(message.contains("grep(pattern=config, path=src)"));
    assert!(message.contains("looping"));
    assert!(message.contains(GOAL));
}

#[test]
fn subtask_message_bounds_the_next_step_to_the_evidence_span() {
    let history = history_through(&["nudge", "sharper_prompt"]);
    let record = apply_rung(
        "smaller_subtask",
        &detection(),
        GOAL,
        Value::int(14),
        &history,
        None,
    )
    .expect("subtask builds");
    assert!(text_of(&record, "message").contains("steps 1 to 4"));
}

#[test]
fn subtask_bound_follows_the_evidence_span_of_the_detection() {
    let mut other = detection();
    other.set(
        "evidence_span",
        value(r#"{"start_step": 10, "end_step": 17}"#),
    );
    let history = history_through(&["nudge", "sharper_prompt"]);
    let record = apply_rung(
        "smaller_subtask",
        &other,
        GOAL,
        Value::int(20),
        &history,
        None,
    )
    .expect("subtask builds");
    let message = text_of(&record, "message");
    assert!(message.contains("steps 10 to 17"));
    assert!(!message.contains("steps 1 to 4"));
}

#[test]
fn subtask_record_refuses_a_detection_missing_a_field() {
    let broken = without(&detection(), "evidence_span");
    let history = history_through(&["nudge", "sharper_prompt"]);
    let err = apply_rung(
        "smaller_subtask",
        &broken,
        GOAL,
        Value::int(14),
        &history,
        None,
    )
    .expect_err("the broken detection is refused");
    assert!(err.0.contains("evidence_span"));
}

#[test]
fn subtask_cannot_fire_with_no_lower_rung_tried() {
    assert!(
        apply_rung(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(4),
            &[],
            None
        )
        .is_err()
    );
    assert!(
        build_intervention(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(4),
            &[],
            None
        )
        .is_err()
    );
}

#[test]
fn subtask_cannot_fire_with_only_the_nudge_tried() {
    let history = history_through(&["nudge"]);
    assert!(
        apply_rung(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(9),
            &history,
            None
        )
        .is_err()
    );
    assert!(
        build_intervention(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(9),
            &history,
            None
        )
        .is_err()
    );
}

#[test]
fn subtask_cannot_fire_with_only_the_sharper_prompt_tried() {
    // A sharper prompt with no nudge before it cannot be built
    // through the ladder, so the history record is written out
    // by hand in the shape the ladder itself writes.
    let history = vec![value(
        r#"{
        "rung": "sharper_prompt",
        "fired_by": {
            "trace_id": "trace-ladder-001",
            "shape": "looping",
            "pattern": ["grep(pattern=config, path=src)"],
            "evidence_span": {"start_step": 1, "end_step": 4}
        },
        "message": "recorded elsewhere",
        "at_step": 9
        }"#,
    )];
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        Some("nudge".to_string())
    );
    assert!(
        apply_rung(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(14),
            &history,
            None
        )
        .is_err()
    );
}

#[test]
fn subtask_cannot_fire_twice_for_the_same_stall() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    assert!(
        apply_rung(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(20),
            &history,
            None
        )
        .is_err()
    );
    assert!(
        build_intervention(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(20),
            &history,
            None
        )
        .is_err()
    );
}

#[test]
fn lower_rungs_tried_on_a_different_stall_do_not_unlock_the_subtask() {
    let mut other = detection();
    other.set("shape", Value::Str("tool_fixation".to_string()));
    other.set("pattern", value(r#"["grep"]"#));
    let history = history_through(&["nudge", "sharper_prompt"]);
    assert_eq!(
        next_rung(&other, &history).expect("lookup works"),
        Some("nudge".to_string())
    );
    assert!(
        apply_rung(
            "smaller_subtask",
            &other,
            GOAL,
            Value::int(14),
            &history,
            None
        )
        .is_err()
    );
}

#[test]
fn fork_and_kill_are_not_rungs_of_this_ladder() {
    let history = history_through(&["nudge", "sharper_prompt", "smaller_subtask"]);
    for rung in ["fork", "kill"] {
        assert!(
            build_intervention(rung, &detection(), GOAL, Value::int(20), &history, None).is_err(),
            "{} is refused",
            rung
        );
    }
}

#[test]
fn the_subtask_scenario_is_in_the_committed_scenario_set() {
    assert!(scenario_names().contains(&"loop-needs-subtask"));
}

#[test]
fn agent_that_ignores_both_lower_rungs_clears_on_the_subtask() {
    let record = run_session("loop-needs-subtask", None).expect("session runs");
    assert_eq!(record.get("stall_cleared"), Some(&Value::Bool(true)));
    assert_eq!(text_of(&record, "outcome"), "cleared");
    assert_eq!(
        rungs_of(&record),
        vec![
            "nudge".to_string(),
            "sharper_prompt".to_string(),
            "smaller_subtask".to_string()
        ]
    );
    assert_eq!(text_of(&record, "cleared_by"), "smaller_subtask");
    let interventions = list(&record, "interventions");
    let last = interventions.last().expect("a last intervention stands");
    assert!((common::int_of(last, "at_step") as usize) < list(&record, "events").len());
}

#[test]
fn sessions_that_clear_before_the_subtask_keep_their_round_3_shape() {
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
}

struct Capture {
    captured: RefCell<Option<Value>>,
}

impl JudgmentSource for Capture {
    fn judge(&self, point: &str, request: &Value) -> Option<Value> {
        if point == "recovery"
            && matches!(request.get("rung"), Some(Value::Str(rung)) if rung == "smaller_subtask")
        {
            *self.captured.borrow_mut() = Some(request.clone());
        }
        None
    }
}

#[test]
fn committed_subtask_fixture_request_is_the_builders_output() {
    let text = std::fs::read_to_string(fixtures().join("recovery-loop-needs-subtask-subtask.json"))
        .expect("fixture reads");
    let pair = pyjson::parse(&text).expect("fixture parses");
    let capture = Capture {
        captured: RefCell::new(None),
    };
    run_session("loop-needs-subtask", Some(&capture)).expect("session runs");
    let captured = capture
        .captured
        .borrow()
        .clone()
        .expect("the subtask recovery request was asked");
    assert_py_eq(&captured, pair.get("request").expect("request stands"));
    let request = pair.get("request").expect("request stands");
    assert_eq!(text_of(request, "rung"), "smaller_subtask");
    let response = pair.get("response").expect("response stands");
    assert_eq!(text_of(response, "kind"), "choice");
}

#[test]
fn trusted_subtask_recovery_is_recorded_on_the_subtask_record() {
    let player = FixturePlayer::new(&fixtures()).expect("player loads");
    let record = run_session("loop-needs-subtask", Some(&player)).expect("session runs");
    let interventions = list(&record, "interventions");
    let third = &interventions[2];
    assert_eq!(text_of(third, "rung"), "smaller_subtask");
    assert_eq!(text_of(third, "recovery"), "recovering");
    let plain = run_session("loop-needs-subtask", None).expect("session runs");
    let plain_third = &list(&plain, "interventions")[2];
    assert_py_eq(&without(third, "recovery"), plain_third);
}

#[test]
fn judgments_never_change_the_subtask_session() {
    let player = FixturePlayer::new(&fixtures()).expect("player loads");
    let plain = run_session("loop-needs-subtask", None).expect("session runs");
    let judged = run_session("loop-needs-subtask", Some(&player)).expect("session runs");
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
fn intervene_fires_the_subtask_end_to_end() {
    let out = run_cli_at_root(&["intervene", "--scenario", "loop-needs-subtask"]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let record = pyjson::parse(&out.stdout).expect("record parses");
    assert_eq!(
        rungs_of(&record),
        vec![
            "nudge".to_string(),
            "sharper_prompt".to_string(),
            "smaller_subtask".to_string()
        ]
    );
    assert_eq!(text_of(&record, "cleared_by"), "smaller_subtask");
    assert_eq!(text_of(&record, "outcome"), "cleared");
}

#[test]
fn intervene_with_fixtures_records_the_subtask_recovery() {
    let fixtures_text = fixtures().to_string_lossy().into_owned();
    let out = run_cli_at_root(&[
        "intervene",
        "--scenario",
        "loop-needs-subtask",
        "--fixtures",
        &fixtures_text,
    ]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let record = pyjson::parse(&out.stdout).expect("record parses");
    let interventions = list(&record, "interventions");
    assert_eq!(text_of(&interventions[2], "recovery"), "recovering");
}
