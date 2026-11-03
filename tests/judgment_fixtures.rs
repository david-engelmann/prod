//! PR-20 and PR-22. The committed fixtures and their schema
//! pin.

mod common;

use std::path::PathBuf;

use common::root;
use prod::judgment::{CONFIDENCE_THRESHOLD, FixturePlayer, validate_fixture};
use prod::pyjson::{self, Value};

fn fixtures() -> PathBuf {
    root().join("fixtures")
}

fn committed_pairs() -> Vec<(String, Value)> {
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(fixtures()).expect("fixtures read") {
        let entry = entry.expect("entry reads");
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") {
            names.push(name);
        }
    }
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let text = std::fs::read_to_string(fixtures().join(&name)).expect("fixture reads");
            let pair = pyjson::parse(&text).expect("fixture parses");
            (name, pair)
        })
        .collect()
}

#[test]
fn every_committed_fixture_matches_the_recorded_shape() {
    let pairs = committed_pairs();
    assert!(!pairs.is_empty(), "no committed fixtures found");
    for (name, pair) in &pairs {
        let validated = validate_fixture(pair, name).expect("fixture validates");
        common::assert_py_eq(&validated, pair);
    }
}

#[test]
fn every_committed_fixture_is_marked_synthetic() {
    for (name, pair) in committed_pairs() {
        assert_eq!(
            pair.get("synthetic"),
            Some(&Value::Bool(true)),
            "fixture {} is not marked synthetic",
            name
        );
    }
}

#[test]
fn the_player_loads_the_committed_directory() {
    let player = FixturePlayer::new(&fixtures()).expect("player loads");
    for (_, pair) in committed_pairs() {
        let point = common::text_of(&pair, "judgment_point");
        let request = pair.get("request").expect("request stands");
        assert!(player.judge(point, request).is_some());
    }
}

#[test]
fn the_committed_set_covers_both_judgment_points() {
    let pairs = committed_pairs();
    let mut points: Vec<&str> = pairs
        .iter()
        .map(|(_, pair)| common::text_of(pair, "judgment_point"))
        .collect();
    points.sort();
    points.dedup();
    assert_eq!(points, vec!["recovery", "severity"]);
}

#[test]
fn the_committed_set_covers_every_recovery_label() {
    let pairs = committed_pairs();
    let mut labels: Vec<&str> = Vec::new();
    for (_, pair) in &pairs {
        let response = pair.get("response").expect("response stands");
        if matches!(response.get("kind"), Some(Value::Str(kind)) if kind == "choice") {
            labels.push(common::text_of(response, "choice"));
        }
    }
    labels.sort();
    labels.dedup();
    assert_eq!(labels, vec!["getting_worse", "recovering", "still_stalled"]);
}

#[test]
fn each_point_has_an_abstention_or_below_threshold_case() {
    let pairs = committed_pairs();
    for point in ["severity", "recovery"] {
        let responses: Vec<&Value> = pairs
            .iter()
            .filter(|(_, pair)| common::text_of(pair, "judgment_point") == point)
            .map(|(_, pair)| pair.get("response").expect("response stands"))
            .collect();
        let fallback = responses.iter().any(|response| {
            if matches!(response.get("kind"), Some(Value::Str(kind)) if kind == "abstention") {
                return true;
            }
            match response.get("confidence") {
                Some(Value::Float(number)) => *number < CONFIDENCE_THRESHOLD,
                Some(Value::Int(text)) => {
                    text.parse::<f64>().expect("confidence parses") < CONFIDENCE_THRESHOLD
                }
                _ => false,
            }
        });
        assert!(
            fallback,
            "no fallback case recorded for the {} point",
            point
        );
    }
}
