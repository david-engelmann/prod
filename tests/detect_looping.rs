//! PR-1. The looping detector.

mod common;

use common::{detections_of, ev, make_trace, pattern_of, span_of};
use prod::detect;
use prod::pyjson::Value;

fn looping(events: Vec<Value>) -> Vec<Value> {
    detections_of(&detect::detect(&make_trace(&events), 0), "looping")
}

#[test]
fn identical_call_run_is_detected_with_the_right_evidence_span() {
    let events = vec![
        ev(0, "read", "read(path=README)", true),
        ev(1, "grep", "grep(pattern=config, path=src)", false),
        ev(2, "grep", "grep(pattern=config, path=src)", false),
        ev(3, "grep", "grep(pattern=config, path=src)", false),
        ev(4, "grep", "grep(pattern=config, path=src)", false),
    ];
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(
        pattern_of(&found[0]),
        vec!["grep(pattern=config, path=src)"]
    );
    assert_eq!(span_of(&found[0]), (1, 4));
}

#[test]
fn two_call_cycle_repeated_is_detected() {
    let calls = ["open(path=a)", "open(path=b)"];
    let events: Vec<Value> = (0..6)
        .map(|step| ev(step, "read", calls[(step % 2) as usize], false))
        .collect();
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(pattern_of(&found[0]), vec!["open(path=a)", "open(path=b)"]);
    assert_eq!(span_of(&found[0]), (0, 5));
}

#[test]
fn two_looping_spans_in_one_window_are_both_reported() {
    let events = vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(1, "grep", "grep(pattern=a, path=src)", false),
        ev(2, "grep", "grep(pattern=a, path=src)", false),
        ev(3, "read", "read(path=src/x.py)", false),
        ev(4, "read", "read(path=src/x.py)", false),
        ev(5, "read", "read(path=src/x.py)", false),
    ];
    let found = looping(events);
    assert_eq!(found.len(), 2);
    assert_eq!(span_of(&found[0]), (0, 2));
    assert_eq!(span_of(&found[1]), (3, 5));
}

#[test]
fn three_call_cycle_repeated_is_detected() {
    let calls = ["open(path=a)", "open(path=b)", "open(path=c)"];
    let events: Vec<Value> = (0..6)
        .map(|step| ev(step, "read", calls[(step % 3) as usize], false))
        .collect();
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(span_of(&found[0]), (0, 5));
}

#[test]
fn four_call_cycle_repeated_is_detected() {
    let calls = [
        "open(path=a)",
        "open(path=b)",
        "open(path=c)",
        "open(path=d)",
    ];
    let events: Vec<Value> = (0..8)
        .map(|step| ev(step, "read", calls[(step % 4) as usize], false))
        .collect();
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(span_of(&found[0]), (0, 7));
}

#[test]
fn two_identical_calls_are_not_a_loop() {
    let events = vec![
        ev(0, "grep", "grep(pattern=config, path=src)", false),
        ev(1, "grep", "grep(pattern=config, path=src)", false),
    ];
    assert!(looping(events).is_empty());
}

#[test]
fn a_progress_call_inside_the_span_breaks_the_loop() {
    let events = vec![
        ev(0, "grep", "grep(pattern=config, path=src)", false),
        ev(1, "grep", "grep(pattern=config, path=src)", false),
        ev(2, "grep", "grep(pattern=config, path=src)", true),
        ev(3, "grep", "grep(pattern=config, path=src)", false),
        ev(4, "grep", "grep(pattern=config, path=src)", false),
    ];
    assert!(looping(events).is_empty());
}

#[test]
fn healthy_trace_with_varied_tool_use_produces_no_detection() {
    let events = vec![
        ev(0, "grep", "grep(pattern=config, path=src)", false),
        ev(1, "read", "read(path=src/config.py)", true),
        ev(2, "edit", "edit(path=src/config.py)", false),
        ev(3, "test", "test(target=config)", true),
        ev(4, "grep", "grep(pattern=other, path=docs)", false),
        ev(5, "read", "read(path=docs/guide.md)", true),
    ];
    assert!(detect::detect(&make_trace(&events), 0).is_empty());
}
