//! PR-28. The looping match order and the window segmentation.

mod common;

use common::{detections_of, ev, make_trace, pattern_of, span_of, text_of};
use westering::detect;
use westering::pyjson::Value;

fn looping(events: Vec<Value>) -> Vec<Value> {
    detections_of(&detect::detect(&make_trace(&events), 0), "looping")
}

#[test]
fn the_earlier_offset_wins_even_when_the_later_pattern_is_longer() {
    let events = vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(1, "grep", "grep(pattern=a, path=src)", false),
        ev(2, "grep", "grep(pattern=a, path=src)", false),
        ev(3, "read", "read(path=src/b.py)", false),
        ev(4, "read", "read(path=src/c.py)", false),
        ev(5, "read", "read(path=src/b.py)", false),
        ev(6, "read", "read(path=src/c.py)", false),
        ev(7, "read", "read(path=src/b.py)", false),
        ev(8, "read", "read(path=src/c.py)", false),
    ];
    let found = looping(events);
    assert_eq!(found.len(), 2);
    assert_eq!(pattern_of(&found[0]), vec!["grep(pattern=a, path=src)"]);
    assert_eq!(span_of(&found[0]), (0, 2));
    assert_eq!(
        pattern_of(&found[1]),
        vec!["read(path=src/b.py)", "read(path=src/c.py)"]
    );
    assert_eq!(span_of(&found[1]), (3, 8));
}

#[test]
fn the_shortest_length_wins_at_one_offset() {
    let events: Vec<Value> = (0..6)
        .map(|step| ev(step, "grep", "grep(pattern=a, path=src)", false))
        .collect();
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(pattern_of(&found[0]), vec!["grep(pattern=a, path=src)"]);
    assert_eq!(span_of(&found[0]), (0, 5));
    assert_eq!(
        text_of(&found[0], "detail"),
        "call pattern of length 1 repeated 6 times with no progress"
    );
}

#[test]
fn the_maximal_repeat_count_sets_the_evidence_span_end() {
    let mut events: Vec<Value> = (0..5)
        .map(|step| ev(step, "grep", "grep(pattern=a, path=src)", false))
        .collect();
    events.push(ev(5, "read", "read(path=src/other.py)", false));
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(span_of(&found[0]), (0, 4));
    assert_eq!(
        text_of(&found[0], "detail"),
        "call pattern of length 1 repeated 5 times with no progress"
    );
}

#[test]
fn a_maximal_cycle_repeat_sets_the_evidence_span_end() {
    let calls = ["read(path=src/b.py)", "read(path=src/c.py)"];
    let mut events: Vec<Value> = (0..6)
        .map(|step| ev(step, "read", calls[(step % 2) as usize], false))
        .collect();
    events.push(ev(6, "grep", "grep(pattern=other, path=docs)", false));
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(
        pattern_of(&found[0]),
        vec!["read(path=src/b.py)", "read(path=src/c.py)"]
    );
    assert_eq!(span_of(&found[0]), (0, 5));
    assert_eq!(
        text_of(&found[0], "detail"),
        "call pattern of length 2 repeated 3 times with no progress"
    );
}

#[test]
fn two_loops_in_one_window_are_separated_by_the_resume_rule() {
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
fn a_pattern_beginning_inside_a_reported_match_is_never_reported() {
    let events: Vec<Value> = (0..4)
        .map(|step| ev(step, "grep", "grep(pattern=a, path=src)", false))
        .collect();
    let found = looping(events);
    assert_eq!(found.len(), 1);
    assert_eq!(span_of(&found[0]), (0, 3));
}

#[test]
fn a_pattern_inside_a_reported_span_is_reported_only_by_its_tail() {
    let p = "read(path=src/b.py)";
    let q = "read(path=src/c.py)";
    let calls = [p, q, p, q, q, q, q];
    let events: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(step, call)| ev(step as i64, "read", call, false))
        .collect();
    let found = looping(events);
    assert_eq!(found.len(), 2);
    assert_eq!(pattern_of(&found[0]), vec![p, q]);
    assert_eq!(span_of(&found[0]), (0, 3));
    assert_eq!(pattern_of(&found[1]), vec![q]);
    assert_eq!(span_of(&found[1]), (4, 6));
}
