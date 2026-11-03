//! PR-27. The judgment seam hardening against malformed
//! answers.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::{root, temp_dir, value, write_json};
use prod::judgment::{FixturePlayer, JudgmentSource, check_answer, trusted, validate_fixture};
use prod::pyjson::{self, Value};
use prod::sandbox::run_session;

fn fixtures() -> PathBuf {
    root().join("fixtures")
}

fn committed_pair(name: &str) -> Value {
    let text = std::fs::read_to_string(fixtures().join(name)).expect("fixture reads");
    pyjson::parse(&text).expect("fixture parses")
}

fn huge() -> Value {
    Value::Int(format!("1{}", "0".repeat(400)))
}

fn set_field(pair: &mut Value, field: &str, replacement: Value) {
    let mut response = pair.get("response").expect("response stands").clone();
    response.set(field, replacement);
    pair.set("response", response);
}

struct StubSource {
    answers: BTreeMap<String, Value>,
}

impl JudgmentSource for StubSource {
    fn judge(&self, point: &str, _request: &Value) -> Option<Value> {
        self.answers.get(point).cloned()
    }
}

fn session_with(answer: Value) -> Value {
    let source = StubSource {
        answers: BTreeMap::from([
            ("severity".to_string(), answer.clone()),
            ("recovery".to_string(), answer),
        ]),
    };
    run_session("loop-needs-sharper", Some(&source)).expect("session runs")
}

fn plain_session() -> Value {
    run_session("loop-needs-sharper", None).expect("session runs")
}

#[test]
fn a_list_kind_refuses_at_fixture_load_with_the_judgment_error() {
    let dir = temp_dir("listkind");
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    set_field(&mut pair, "kind", value(r#"["score"]"#));
    let err = validate_fixture(&pair, "mutant").expect_err("the mutant is refused");
    assert!(
        err.0.contains("kind"),
        "the refusal names kind, got {}",
        err.0
    );
    write_json(&dir.join("listkind.json"), &pair);
    let err = FixturePlayer::new(&dir).expect_err("the player refuses");
    assert!(err.to_string().contains("listkind.json"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_list_kind_from_a_foreign_source_counts_as_no_judgment() {
    let answer = value(r#"{"kind": ["score"], "score": 0.5, "confidence": 0.9}"#);
    assert!(check_answer("severity", Some(answer.clone())).is_none());
    let source = StubSource {
        answers: BTreeMap::from([
            ("severity".to_string(), answer),
            (
                "recovery".to_string(),
                value(
                    r#"{"kind": ["choice"], "choice": "recovering", "confidence": 0.95,
                    "probabilities": {"recovering": 0.95, "still_stalled": 0.04, "getting_worse": 0.01}}"#,
                ),
            ),
        ]),
    };
    let judged = run_session("loop-needs-sharper", Some(&source)).expect("session runs");
    common::assert_py_eq(&judged, &plain_session());
}

#[test]
fn a_non_string_kind_of_any_shape_never_raises_out_of_checking() {
    for kind in [
        value(r#"["score"]"#),
        value(r#"{"name": "score"}"#),
        Value::int(7),
        Value::Null,
    ] {
        let answer = Value::object(vec![
            ("kind".to_string(), kind),
            ("score".to_string(), Value::Float(0.5)),
            ("confidence".to_string(), Value::Float(0.9)),
        ]);
        assert!(check_answer("severity", Some(answer.clone())).is_none());
        assert!(!trusted(Some(&answer)));
    }
}

#[test]
fn a_huge_integer_confidence_refuses_at_load_and_counts_as_no_judgment() {
    let dir = temp_dir("huge-confidence");
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    set_field(&mut pair, "confidence", huge());
    assert!(validate_fixture(&pair, "mutant").is_err());
    write_json(&dir.join("huge.json"), &pair);
    assert!(FixturePlayer::new(&dir).is_err());
    let answer = Value::object(vec![
        ("kind".to_string(), Value::Str("score".to_string())),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), huge()),
    ]);
    assert!(check_answer("severity", Some(answer.clone())).is_none());
    assert!(!trusted(Some(&answer)));
    let judged = session_with(answer);
    common::assert_py_eq(&judged, &plain_session());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_huge_integer_score_refuses_at_load_and_counts_as_no_judgment() {
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    set_field(&mut pair, "score", huge());
    assert!(validate_fixture(&pair, "mutant").is_err());
    let answer = Value::object(vec![
        ("kind".to_string(), Value::Str("score".to_string())),
        ("score".to_string(), huge()),
        ("confidence".to_string(), Value::Float(0.9)),
    ]);
    assert!(check_answer("severity", Some(answer)).is_none());
}

#[test]
fn a_huge_integer_probability_refuses_at_load_and_counts_as_no_judgment() {
    let mut pair = committed_pair("recovery-loop-clears-on-nudge.json");
    let mut response = pair.get("response").expect("response stands").clone();
    let mut probabilities = response
        .get("probabilities")
        .expect("probabilities stand")
        .clone();
    probabilities.set("recovering", huge());
    response.set("probabilities", probabilities);
    pair.set("response", response.clone());
    assert!(validate_fixture(&pair, "mutant").is_err());
    assert!(check_answer("recovery", Some(response)).is_none());
}

#[test]
fn a_non_finite_number_is_not_a_number_for_the_seam() {
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    set_field(&mut pair, "confidence", Value::Float(f64::INFINITY));
    assert!(validate_fixture(&pair, "mutant").is_err());
    for confidence in [Value::Float(f64::NAN), Value::Float(f64::INFINITY), huge()] {
        let answer = Value::object(vec![
            ("kind".to_string(), Value::Str("score".to_string())),
            ("score".to_string(), Value::Float(0.5)),
            ("confidence".to_string(), confidence),
        ]);
        assert!(check_answer("severity", Some(answer.clone())).is_none());
        assert!(!trusted(Some(&answer)));
    }
}

#[test]
fn extra_fields_of_mixed_types_are_refused_without_a_comparison_error() {
    // At the JSON surface every key is a string, so the mixed
    // type extra fields of the Python pin stand here as the
    // numeric named field and the word named field. The refusal
    // and the no judgment outcome are the pin.
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    let mut response = pair.get("response").expect("response stands").clone();
    response.set("123", Value::Str("x".to_string()));
    response.set("zzz", Value::int(1));
    pair.set("response", response);
    assert!(validate_fixture(&pair, "mutant").is_err());
    let answer =
        value(r#"{"kind": "score", "score": 0.5, "confidence": 0.9, "123": "x", "zzz": 1}"#);
    assert!(check_answer("severity", Some(answer.clone())).is_none());
    assert!(check_answer("recovery", Some(answer)).is_none());
}

#[test]
fn an_extra_envelope_field_refuses_at_load_naming_it() {
    let dir = temp_dir("extra-envelope");
    let mut pair = committed_pair("severity-loop-needs-sharper-opening.json");
    pair.set("mystery_field", Value::Str("surprise".to_string()));
    let err = validate_fixture(&pair, "mutant").expect_err("the mutant is refused");
    assert!(err.0.contains("mystery_field"));
    write_json(&dir.join("extra.json"), &pair);
    let err = FixturePlayer::new(&dir).expect_err("the player refuses");
    assert!(err.to_string().contains("mystery_field"));
    assert!(err.to_string().contains("extra.json"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_exact_envelope_still_loads() {
    let pair = committed_pair("severity-loop-needs-sharper-opening.json");
    let mut keys: Vec<&str> = match &pair {
        Value::Object(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
        _ => panic!("pair is an object"),
    };
    keys.sort();
    assert_eq!(
        keys,
        vec!["judgment_point", "request", "response", "synthetic"]
    );
    let validated = validate_fixture(&pair, "mutant").expect("the pair validates");
    common::assert_py_eq(&validated, &pair);
}

fn recovery_pair_with_probabilities(probabilities: Value) -> Value {
    let mut pair = committed_pair("recovery-loop-clears-on-nudge.json");
    let mut response = pair.get("response").expect("response stands").clone();
    response.set("probabilities", probabilities);
    pair.set("response", response);
    pair
}

#[test]
fn a_probability_sum_outside_the_named_band_is_refused() {
    for probabilities in [
        value(r#"{"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.1}"#),
        value(r#"{"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.202}"#),
    ] {
        let pair = recovery_pair_with_probabilities(probabilities);
        assert!(validate_fixture(&pair, "mutant").is_err());
        let response = pair.get("response").expect("response stands").clone();
        assert!(check_answer("recovery", Some(response)).is_none());
    }
}

#[test]
fn a_probability_sum_inside_the_named_band_is_accepted() {
    for probabilities in [
        value(r#"{"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.1995}"#),
        value(r#"{"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.2009}"#),
    ] {
        let pair = recovery_pair_with_probabilities(probabilities);
        let validated = validate_fixture(&pair, "mutant").expect("the pair validates");
        common::assert_py_eq(&validated, &pair);
        let response = pair.get("response").expect("response stands").clone();
        let answer = check_answer("recovery", Some(response.clone())).expect("the answer stands");
        common::assert_py_eq(&answer, &response);
    }
}
