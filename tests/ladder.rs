//! PR-4, PR-5, PR-6, PR-13, PR-14, PR-15. The ladder rungs and
//! their order.

mod common;

use common::{assert_py_eq, text_of, value, without};
use westering::ladder::{apply_rung, build_intervention, next_rung};
use westering::pyjson::Value;

const GOAL: &str = "Find the config file and report its path";

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

fn nudge_history() -> Vec<Value> {
    vec![apply_rung("nudge", &detection(), GOAL, Value::int(4), &[], None).expect("nudge builds")]
}

#[test]
fn nudge_is_the_first_rung_offered() {
    assert_eq!(
        next_rung(&detection(), &[]).expect("lookup works"),
        Some("nudge".to_string())
    );
}

#[test]
fn nudge_record_names_the_rung_and_the_firing_detection() {
    let record =
        apply_rung("nudge", &detection(), GOAL, Value::int(4), &[], None).expect("nudge builds");
    assert_eq!(text_of(&record, "rung"), "nudge");
    assert_eq!(common::int_of(&record, "at_step"), 4);
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
    assert!(text_of(&record, "message").contains("looping"));
    assert!(text_of(&record, "message").contains(GOAL));
}

#[test]
fn sharper_prompt_cannot_fire_before_nudge() {
    assert!(
        apply_rung(
            "sharper_prompt",
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
fn after_nudge_the_next_rung_is_sharper_prompt() {
    assert_eq!(
        next_rung(&detection(), &nudge_history()).expect("lookup works"),
        Some("sharper_prompt".to_string())
    );
}

#[test]
fn sharper_prompt_record_names_the_pattern_and_restates_the_goal() {
    let history = nudge_history();
    let record = apply_rung(
        "sharper_prompt",
        &detection(),
        GOAL,
        Value::int(9),
        &history,
        None,
    )
    .expect("sharper prompt builds");
    assert_eq!(text_of(&record, "rung"), "sharper_prompt");
    let fired_by = record.get("fired_by").expect("fired_by stands");
    assert_py_eq(
        fired_by.get("pattern").expect("pattern stands"),
        &value(r#"["grep(pattern=config, path=src)"]"#),
    );
    assert!(text_of(&record, "message").contains("grep(pattern=config, path=src)"));
    assert!(text_of(&record, "message").contains(GOAL));
}

// Round 4 alteration, carried from the Python suite. The pin
// asserted the ladder was spent after the two Round 1 rungs.
// PR-31 adds the smaller subtask rung, so the spent state now
// follows the third rung. The claim the pin carries, that a
// fully tried ladder offers no rung, is unchanged.
// Round 6 alteration. PR-45 adds the fork rung, so the spent
// state now follows the fourth rung, and after the third rung
// the lookup offers the fork. The claim the pin carries is
// unchanged.
#[test]
fn ladder_is_spent_after_all_four_rungs() {
    let mut history = nudge_history();
    history.push(
        apply_rung(
            "sharper_prompt",
            &detection(),
            GOAL,
            Value::int(9),
            &history,
            None,
        )
        .expect("sharper prompt builds"),
    );
    history.push(
        apply_rung(
            "smaller_subtask",
            &detection(),
            GOAL,
            Value::int(14),
            &history,
            None,
        )
        .expect("smaller subtask builds"),
    );
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        Some("fork".to_string())
    );
    history.push(
        apply_rung("fork", &detection(), GOAL, Value::int(19), &history, None)
            .expect("fork builds"),
    );
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        None
    );
}

#[test]
fn nudge_cannot_fire_twice_for_the_same_stall() {
    let history = nudge_history();
    assert!(apply_rung("nudge", &detection(), GOAL, Value::int(6), &history, None).is_err());
}

#[test]
fn a_different_stall_starts_at_nudge_again() {
    let mut other = detection();
    other.set("shape", Value::Str("tool_fixation".to_string()));
    other.set("pattern", value(r#"["grep"]"#));
    assert_eq!(
        next_rung(&other, &nudge_history()).expect("lookup works"),
        Some("nudge".to_string())
    );
}

#[test]
fn build_intervention_rejects_an_unknown_rung() {
    assert!(build_intervention("kill", &detection(), GOAL, Value::int(0), &[], None).is_err());
}

#[test]
fn build_intervention_builds_a_nudge_with_no_history() {
    let record = build_intervention("nudge", &detection(), GOAL, Value::int(4), &[], None)
        .expect("nudge builds");
    assert_eq!(text_of(&record, "rung"), "nudge");
    let fired_by = record.get("fired_by").expect("fired_by stands");
    assert_eq!(text_of(fired_by, "trace_id"), "trace-ladder-001");
}

#[test]
fn build_intervention_refuses_sharper_prompt_without_a_nudge_on_record() {
    assert!(
        build_intervention(
            "sharper_prompt",
            &detection(),
            GOAL,
            Value::int(9),
            &[],
            None
        )
        .is_err()
    );
}

#[test]
fn build_intervention_allows_sharper_prompt_after_a_nudge() {
    let history = vec![
        build_intervention("nudge", &detection(), GOAL, Value::int(4), &[], None)
            .expect("nudge builds"),
    ];
    let record = build_intervention(
        "sharper_prompt",
        &detection(),
        GOAL,
        Value::int(9),
        &history,
        None,
    )
    .expect("sharper prompt builds");
    assert_eq!(text_of(&record, "rung"), "sharper_prompt");
    assert!(text_of(&record, "message").contains(GOAL));
}

#[test]
fn build_intervention_refuses_a_rung_already_tried_for_the_stall() {
    let history = vec![
        build_intervention("nudge", &detection(), GOAL, Value::int(4), &[], None)
            .expect("nudge builds"),
    ];
    assert!(
        build_intervention("nudge", &detection(), GOAL, Value::int(6), &history, None).is_err()
    );
}

#[test]
fn second_sharper_prompt_is_refused_by_apply_rung() {
    let mut history = nudge_history();
    history.push(
        apply_rung(
            "sharper_prompt",
            &detection(),
            GOAL,
            Value::int(9),
            &history,
            None,
        )
        .expect("sharper prompt builds"),
    );
    // Round 4 alteration, carried from the Python suite. The
    // lookup no longer reports the ladder spent at this history,
    // because PR-31 offers the smaller subtask after the sharper
    // prompt. The refusal this pin exists for, a second sharper
    // prompt for the same stall, is unchanged.
    assert_eq!(
        next_rung(&detection(), &history).expect("lookup works"),
        Some("smaller_subtask".to_string())
    );
    assert!(
        apply_rung(
            "sharper_prompt",
            &detection(),
            GOAL,
            Value::int(12),
            &history,
            None
        )
        .is_err()
    );
}

#[test]
fn second_sharper_prompt_is_refused_by_build_intervention() {
    let mut history = nudge_history();
    history.push(
        apply_rung(
            "sharper_prompt",
            &detection(),
            GOAL,
            Value::int(9),
            &history,
            None,
        )
        .expect("sharper prompt builds"),
    );
    assert!(
        build_intervention(
            "sharper_prompt",
            &detection(),
            GOAL,
            Value::int(12),
            &history,
            None
        )
        .is_err()
    );
}

fn detection_missing(field: &str) -> Value {
    without(&detection(), field)
}

fn assert_missing_field_refused(field: &str) {
    let broken = detection_missing(field);
    let err = apply_rung("nudge", &broken, GOAL, Value::int(4), &[], None)
        .expect_err("the broken detection is refused");
    assert!(
        err.0.contains(field),
        "the refusal names {}, got {}",
        field,
        err.0
    );
    let err = build_intervention("nudge", &broken, GOAL, Value::int(4), &[], None)
        .expect_err("the broken detection is refused");
    assert!(err.0.contains(field));
}

#[test]
fn detection_missing_trace_id_is_refused_with_the_field_named() {
    assert_missing_field_refused("trace_id");
}

#[test]
fn detection_missing_shape_is_refused_with_the_field_named() {
    assert_missing_field_refused("shape");
}

#[test]
fn detection_missing_pattern_is_refused_with_the_field_named() {
    assert_missing_field_refused("pattern");
}

#[test]
fn detection_missing_evidence_span_is_refused_with_the_field_named() {
    assert_missing_field_refused("evidence_span");
}

#[test]
fn history_record_missing_its_rung_is_refused_with_the_field_named() {
    let record = nudge_history().into_iter().next().expect("one record");
    let broken = without(&record, "rung");
    let err = apply_rung(
        "sharper_prompt",
        &detection(),
        GOAL,
        Value::int(9),
        &[broken],
        None,
    )
    .expect_err("the broken record is refused");
    assert!(err.0.contains("rung"));
}

#[test]
fn history_record_missing_fired_by_is_refused_with_the_field_named() {
    let record = nudge_history().into_iter().next().expect("one record");
    let broken = without(&record, "fired_by");
    let err = build_intervention(
        "sharper_prompt",
        &detection(),
        GOAL,
        Value::int(9),
        &[broken],
        None,
    )
    .expect_err("the broken record is refused");
    assert!(err.0.contains("fired_by"));
}

#[test]
fn history_record_with_an_unhashable_rung_is_refused_with_the_field_named() {
    let mut record = nudge_history().into_iter().next().expect("one record");
    record.set("rung", Value::List(vec![]));
    let err = apply_rung(
        "sharper_prompt",
        &detection(),
        GOAL,
        Value::int(9),
        &[record],
        None,
    )
    .expect_err("the broken record is refused");
    assert!(err.0.contains("rung"));
}

#[test]
fn history_fired_by_missing_a_field_is_refused_with_the_field_named() {
    let record = nudge_history().into_iter().next().expect("one record");
    let fired_by = record.get("fired_by").expect("fired_by stands").clone();
    for field in ["trace_id", "evidence_span"] {
        let mut broken = record.clone();
        broken.set("fired_by", without(&fired_by, field));
        let err = apply_rung(
            "sharper_prompt",
            &detection(),
            GOAL,
            Value::int(9),
            &[broken],
            None,
        )
        .expect_err("the broken record is refused");
        assert!(err.0.contains(field), "the refusal names {}", field);
    }
}

#[test]
fn malformed_records_never_surface_as_a_bare_key_error() {
    let broken = value(r#"{"shape": "looping"}"#);
    assert!(apply_rung("nudge", &broken, GOAL, Value::int(4), &[], None).is_err());
    let history = vec![value(r#"{"rung": "nudge"}"#)];
    assert!(next_rung(&detection(), &history).is_err());
}
