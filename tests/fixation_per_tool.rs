//! PR-19. One fixation detection per dominant tool in a window.

mod common;

use common::{detections_of, ev, make_trace, pattern_of, span_of};
use westering::detect;
use westering::pyjson::Value;

fn two_tool_events() -> Vec<Value> {
    vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(1, "grep", "grep(pattern=b, path=src)", false),
        ev(2, "grep", "grep(pattern=c, path=src)", false),
        ev(3, "grep", "grep(pattern=d, path=src)", false),
        ev(4, "read", "read(path=src/a.py)", false),
        ev(5, "read", "read(path=src/b.py)", false),
        ev(6, "read", "read(path=src/c.py)", false),
        ev(7, "read", "read(path=src/d.py)", false),
    ]
}

fn fixation_in(trace: &westering::trace::Trace) -> Vec<Value> {
    detections_of(&detect::detect(trace, 0), "tool_fixation")
}

#[test]
fn two_dominant_tools_in_one_window_yield_two_detections() {
    let fixation = fixation_in(&make_trace(&two_tool_events()));
    assert_eq!(fixation.len(), 2);
    assert_eq!(pattern_of(&fixation[0]), vec!["grep"]);
    assert_eq!(span_of(&fixation[0]), (0, 3));
    assert_eq!(pattern_of(&fixation[1]), vec!["read"]);
    assert_eq!(span_of(&fixation[1]), (4, 7));
}

#[test]
fn detections_from_one_window_come_in_span_start_order() {
    let mut events = two_tool_events();
    events.reverse();
    let events: Vec<Value> = events
        .iter()
        .enumerate()
        .map(|(step, event)| {
            ev(
                step as i64,
                common::text_of(event, "tool"),
                common::text_of(event, "call"),
                false,
            )
        })
        .collect();
    let fixation = fixation_in(&make_trace(&events));
    let patterns: Vec<Vec<String>> = fixation.iter().map(pattern_of).collect();
    assert_eq!(
        patterns,
        vec![vec!["read".to_string()], vec!["grep".to_string()]]
    );
    assert_eq!(span_of(&fixation[0]), (0, 3));
    assert_eq!(span_of(&fixation[1]), (4, 7));
}

#[test]
fn a_second_tool_below_the_call_count_yields_one_detection() {
    let mut events = two_tool_events()[..7].to_vec();
    events.push(ev(7, "edit", "edit(path=src/a.py)", false));
    let fixation = fixation_in(&make_trace(&events));
    assert_eq!(fixation.len(), 1);
    assert_eq!(pattern_of(&fixation[0]), vec!["grep"]);
    assert_eq!(span_of(&fixation[0]), (0, 3));
}

#[test]
fn a_second_tool_with_identical_calls_is_not_fixation() {
    let mut events = two_tool_events()[..4].to_vec();
    for step in 4..8 {
        events.push(ev(step, "read", "read(path=src/a.py)", false));
    }
    let fixation = fixation_in(&make_trace(&events));
    assert_eq!(fixation.len(), 1);
    assert_eq!(pattern_of(&fixation[0]), vec!["grep"]);
}

#[test]
fn detections_never_merge_spans_across_tools() {
    let fixation = fixation_in(&make_trace(&two_tool_events()));
    assert_eq!(fixation.len(), 2);
    for detection in &fixation {
        assert_eq!(common::list(detection, "pattern").len(), 1);
    }
    let mut spans: Vec<(String, i64, i64)> = fixation
        .iter()
        .map(|detection| {
            let (start, end) = span_of(detection);
            (pattern_of(detection)[0].clone(), start, end)
        })
        .collect();
    spans.sort();
    assert_eq!(
        spans,
        vec![("grep".to_string(), 0, 3), ("read".to_string(), 4, 7),]
    );
    for detection in &fixation {
        assert_ne!(span_of(detection), (0, 7));
    }
}
