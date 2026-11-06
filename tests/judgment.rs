//! PR-20 to PR-24. The fixture player and the judgment wiring.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::{assert_py_eq, list, root, temp_dir, text_of, value, without, write_json};
use westering::detect;
use westering::judgment::{FixturePlayer, JudgmentSource, trusted, validate_fixture};
use westering::ladder::build_intervention;
use westering::pyjson::{self, Value};
use westering::sandbox::run_session;

fn fixtures() -> PathBuf {
    root().join("fixtures")
}

fn committed_player() -> FixturePlayer {
    FixturePlayer::new(&fixtures()).expect("player loads")
}

fn committed_pair(name: &str) -> Value {
    let text = std::fs::read_to_string(fixtures().join(name)).expect("fixture reads");
    pyjson::parse(&text).expect("fixture parses")
}

fn write_fixture(directory: &std::path::Path, name: &str, pair: &Value) -> PathBuf {
    let path = directory.join(name);
    write_json(&path, pair);
    path
}

#[test]
fn player_answers_a_recorded_request_exactly() {
    let pair = committed_pair("severity-loop-needs-sharper-opening.json");
    let player = committed_player();
    let answer = player
        .judge("severity", pair.get("request").expect("request stands"))
        .expect("an answer is recorded");
    assert_py_eq(&answer, pair.get("response").expect("response stands"));
}

#[test]
fn player_abstains_where_no_fixture_matches() {
    let pair = committed_pair("severity-loop-needs-sharper-opening.json");
    let mut request = pair.get("request").expect("request stands").clone();
    let mut span = request.get("evidence_span").expect("span stands").clone();
    let end = common::int_of(&span, "end_step");
    span.set("end_step", Value::int(end + 1));
    request.set("evidence_span", span);
    assert!(committed_player().judge("severity", &request).is_none());
}

#[test]
fn player_returns_an_abstention_response_where_one_is_recorded() {
    let pair = committed_pair("severity-loop-clears-on-nudge.json");
    let answer = committed_player()
        .judge("severity", pair.get("request").expect("request stands"))
        .expect("an answer is recorded");
    assert_py_eq(&answer, &value(r#"{"kind": "abstention"}"#));
    assert!(!trusted(Some(&answer)));
}

#[test]
fn player_refuses_a_fixture_that_breaks_the_shape() {
    let dir = temp_dir("broken-fixture");
    let mut pair = committed_pair("recovery-loop-needs-sharper-nudge.json");
    let mut response = pair.get("response").expect("response stands").clone();
    response = without(&response, "probabilities");
    pair.set("response", response);
    write_fixture(&dir, "broken.json", &pair);
    let err = FixturePlayer::new(&dir).expect_err("the player refuses");
    assert!(
        err.to_string().contains("broken.json"),
        "the refusal names the file, got {}",
        err
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn player_refuses_two_fixtures_for_the_same_request() {
    let dir = temp_dir("duplicate-fixture");
    let pair = committed_pair("severity-loop-needs-sharper-opening.json");
    write_fixture(&dir, "one.json", &pair);
    write_fixture(&dir, "two.json", &pair);
    assert!(FixturePlayer::new(&dir).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn player_refuses_a_missing_directory() {
    let dir = temp_dir("missing-fixture").join("nope");
    assert!(FixturePlayer::new(&dir).is_err());
}

#[test]
fn validator_refuses_a_choice_with_a_missing_label_probability() {
    let mut pair = committed_pair("recovery-loop-needs-sharper-nudge.json");
    let mut response = pair.get("response").expect("response stands").clone();
    let mut probabilities = response
        .get("probabilities")
        .expect("probabilities stand")
        .clone();
    probabilities = without(&probabilities, "getting_worse");
    response.set("probabilities", probabilities);
    pair.set("response", response);
    assert!(validate_fixture(&pair, "mutant").is_err());
}

#[test]
fn validator_refuses_a_response_for_the_wrong_point() {
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    pair.set("judgment_point", Value::Str("recovery".to_string()));
    assert!(validate_fixture(&pair, "mutant").is_err());
}

#[test]
fn validator_refuses_a_confidence_outside_the_unit_interval() {
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    let mut response = pair.get("response").expect("response stands").clone();
    response.set("confidence", Value::Float(1.4));
    pair.set("response", response);
    assert!(validate_fixture(&pair, "mutant").is_err());
}

#[test]
fn validator_refuses_a_score_response_carrying_a_choice_field() {
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    let mut response = pair.get("response").expect("response stands").clone();
    response.set("choice", Value::Str("recovering".to_string()));
    pair.set("response", response);
    assert!(validate_fixture(&pair, "mutant").is_err());
}

#[test]
fn validator_refuses_an_abstention_carrying_a_choice_and_a_score() {
    let mut pair = committed_pair("severity-fixation-clears-on-nudge.json");
    pair.set(
        "response",
        value(
            r#"{"kind": "abstention", "choice": "recovering", "score": 0.9, "confidence": 0.99}"#,
        ),
    );
    assert!(validate_fixture(&pair, "mutant").is_err());
}

struct StubSource {
    answers: BTreeMap<String, Value>,
}

impl JudgmentSource for StubSource {
    fn judge(&self, point: &str, _request: &Value) -> Option<Value> {
        self.answers.get(point).cloned()
    }
}

fn stub(pairs: Vec<(&str, Value)>) -> StubSource {
    StubSource {
        answers: pairs
            .into_iter()
            .map(|(point, answer)| (point.to_string(), answer))
            .collect(),
    }
}

#[test]
fn a_malformed_foreign_answer_counts_as_no_judgment() {
    let source = stub(vec![
        ("severity", value(r#"{"kind": "score", "confidence": 0.9}"#)),
        (
            "recovery",
            value(r#"{"kind": "choice", "confidence": 0.95}"#),
        ),
    ]);
    let judged = run_session("loop-needs-sharper", Some(&source)).expect("session runs");
    let plain = run_session("loop-needs-sharper", None).expect("session runs");
    assert_py_eq(&judged, &plain);
}

#[test]
fn a_wrong_kind_foreign_answer_counts_as_no_judgment() {
    let source = stub(vec![
        (
            "severity",
            value(
                r#"{"kind": "choice", "choice": "recovering", "confidence": 1.0,
                "probabilities": {"recovering": 1.0, "still_stalled": 0.0, "getting_worse": 0.0}}"#,
            ),
        ),
        (
            "recovery",
            value(r#"{"kind": "score", "score": 0.5, "confidence": 1.0}"#),
        ),
    ]);
    let judged = run_session("loop-needs-sharper", Some(&source)).expect("session runs");
    let plain = run_session("loop-needs-sharper", None).expect("session runs");
    assert_py_eq(&judged, &plain);
}

#[test]
fn validator_refuses_a_fixture_not_marked_synthetic() {
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    pair.set("synthetic", Value::Bool(false));
    assert!(validate_fixture(&pair, "mutant").is_err());
}

#[test]
fn trust_follows_the_contract_threshold() {
    let at = value(r#"{"kind": "score", "score": 0.5, "confidence": 0.6}"#);
    assert!(trusted(Some(&at)));
    let below = value(r#"{"kind": "score", "score": 0.5, "confidence": 0.59}"#);
    assert!(!trusted(Some(&below)));
    let choice = value(r#"{"kind": "choice", "choice": "recovering", "confidence": 0.2}"#);
    assert!(!trusted(Some(&choice)));
    let abstention = value(r#"{"kind": "abstention"}"#);
    assert!(!trusted(Some(&abstention)));
    assert!(!trusted(None));
}

#[test]
fn ladder_records_a_handed_in_severity_and_nothing_else_changes() {
    let events: Vec<Value> = (0..3)
        .map(|step| common::ev(step, "grep", "grep(pattern=config, path=src)", false))
        .collect();
    let trace = common::make_trace(&events);
    let detection = detect::detect(&trace, 0)
        .into_iter()
        .next()
        .expect("a detection stands");
    let plain = build_intervention("nudge", &detection, "goal", Value::int(2), &[], None)
        .expect("nudge builds");
    let stamped = build_intervention(
        "nudge",
        &detection,
        "goal",
        Value::int(2),
        &[],
        Some(Value::Float(0.82)),
    )
    .expect("nudge builds");
    assert!(plain.get("severity").is_none());
    assert_py_eq(
        stamped.get("severity").expect("severity stands"),
        &Value::Float(0.82),
    );
    assert_py_eq(&without(&stamped, "severity"), &plain);
}

#[test]
fn trusted_severity_is_recorded_on_the_intervention_record() {
    let player = committed_player();
    let record = run_session("loop-needs-sharper", Some(&player)).expect("session runs");
    let interventions = list(&record, "interventions");
    let first = &interventions[0];
    assert_py_eq(
        first.get("severity").expect("severity stands"),
        &Value::Float(0.82),
    );
    let plain = run_session("loop-needs-sharper", None).expect("session runs");
    let plain_first = &list(&plain, "interventions")[0];
    assert_py_eq(
        &common::without_many(first, &["severity", "recovery"]),
        plain_first,
    );
    assert_eq!(text_of(&interventions[1], "rung"), "sharper_prompt");
}

#[test]
fn below_threshold_severity_leaves_the_record_as_it_was() {
    let player = committed_player();
    let record = run_session("loop-needs-sharper", Some(&player)).expect("session runs");
    let plain = run_session("loop-needs-sharper", None).expect("session runs");
    let second = &list(&record, "interventions")[1];
    assert!(second.get("severity").is_none());
    assert_py_eq(
        &without(second, "recovery"),
        &list(&plain, "interventions")[1],
    );
}

#[test]
fn a_low_score_with_a_trusted_confidence_is_still_recorded() {
    let player = committed_player();
    let record = run_session("loop-never-clears", Some(&player)).expect("session runs");
    let first = &list(&record, "interventions")[0];
    assert_py_eq(
        first.get("severity").expect("severity stands"),
        &Value::Float(0.35),
    );
}

#[test]
fn an_abstaining_source_leaves_the_session_bit_for_bit() {
    let dir = temp_dir("abstain-fixture");
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    pair.set("response", value(r#"{"kind": "abstention"}"#));
    write_fixture(&dir, "abstain.json", &pair);
    let player = FixturePlayer::new(&dir).expect("player loads");
    let record = run_session("loop-needs-sharper", Some(&player)).expect("session runs");
    let plain = run_session("loop-needs-sharper", None).expect("session runs");
    assert_py_eq(&record, &plain);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_below_threshold_source_leaves_the_session_bit_for_bit() {
    let dir = temp_dir("quiet-fixture");
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    pair.set(
        "response",
        value(r#"{"kind": "score", "score": 0.99, "confidence": 0.59}"#),
    );
    write_fixture(&dir, "quiet.json", &pair);
    let player = FixturePlayer::new(&dir).expect("player loads");
    let record = run_session("loop-needs-sharper", Some(&player)).expect("session runs");
    let plain = run_session("loop-needs-sharper", None).expect("session runs");
    assert_py_eq(&record, &plain);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn recovery_choices_are_recorded_verbatim() {
    let player = committed_player();
    let record = run_session("loop-needs-sharper", Some(&player)).expect("session runs");
    let interventions = list(&record, "interventions");
    assert_eq!(text_of(&interventions[0], "recovery"), "still_stalled");
    assert_eq!(text_of(&interventions[1], "recovery"), "recovering");
    let record = run_session("loop-never-clears", Some(&player)).expect("session runs");
    let interventions = list(&record, "interventions");
    assert_eq!(text_of(&interventions[1], "recovery"), "getting_worse");
}

#[test]
fn untrusted_and_abstaining_recovery_answers_record_nothing() {
    let player = committed_player();
    let record = run_session("loop-never-clears", Some(&player)).expect("session runs");
    assert!(list(&record, "interventions")[0].get("recovery").is_none());
    let record = run_session("fixation-clears-on-nudge", Some(&player)).expect("session runs");
    let first = &list(&record, "interventions")[0];
    assert!(first.get("recovery").is_none());
    assert_py_eq(
        first.get("severity").expect("severity stands"),
        &Value::Float(0.64),
    );
}

#[test]
fn judgments_never_change_rungs_or_outcomes() {
    let player = committed_player();
    for name in [
        "loop-clears-on-nudge",
        "loop-needs-sharper",
        "fixation-clears-on-nudge",
        "loop-never-clears",
    ] {
        let plain = run_session(name, None).expect("session runs");
        let judged = run_session(name, Some(&player)).expect("session runs");
        let judged_rungs: Vec<String> = list(&judged, "interventions")
            .iter()
            .map(|item| text_of(item, "rung").to_string())
            .collect();
        let plain_rungs: Vec<String> = list(&plain, "interventions")
            .iter()
            .map(|item| text_of(item, "rung").to_string())
            .collect();
        assert_eq!(judged_rungs, plain_rungs, "scenario {}", name);
        assert_eq!(text_of(&judged, "outcome"), text_of(&plain, "outcome"));
        assert_py_eq(
            judged.get("events").expect("events stand"),
            plain.get("events").expect("events stand"),
        );
        assert_py_eq(
            judged.get("detections").expect("detections stand"),
            plain.get("detections").expect("detections stand"),
        );
    }
}
