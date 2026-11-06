//! PR-2 and PR-12. The tool fixation detector and its subspan
//! search.

mod common;

use common::{GOAL, detections_of, ev, make_trace_full, pattern_of, span_of, text_of};
use westering::detect;
use westering::pyjson::Value;
use westering::trace::Trace;

fn fixation_events() -> Vec<Value> {
    vec![
        ev(0, "grep", "grep(pattern=config, path=src)", false),
        ev(1, "grep", "grep(pattern=settings, path=src)", false),
        ev(2, "grep", "grep(pattern=config, path=docs)", false),
        ev(3, "grep", "grep(pattern=conf, path=src)", false),
        ev(4, "grep", "grep(pattern=config, path=tests)", false),
    ]
}

fn fixation_in(trace: &Trace) -> Vec<Value> {
    detections_of(&detect::detect(trace, 0), "tool_fixation")
}

#[test]
fn fixation_trace_is_detected_with_its_span() {
    let trace = make_trace_full(&fixation_events(), "trace-fixation", &common::TOOLS, GOAL);
    let fixation = fixation_in(&trace);
    assert_eq!(fixation.len(), 1);
    assert_eq!(pattern_of(&fixation[0]), vec!["grep"]);
    // PR-12. The reported span is the shortest qualifying
    // subspan, the four calls at steps 0 to 3, not the whole
    // five call window.
    assert_eq!(span_of(&fixation[0]), (0, 3));
    assert!(text_of(&fixation[0], "detail").contains("4 of 4"));
}

#[test]
fn identical_calls_to_one_tool_are_looping_not_fixation() {
    let events: Vec<Value> = (0..5)
        .map(|step| ev(step, "grep", "grep(pattern=config, path=src)", false))
        .collect();
    let found = detect::detect(&common::make_trace(&events), 0);
    let shapes: Vec<&str> = found.iter().map(|item| text_of(item, "shape")).collect();
    assert!(!shapes.contains(&"tool_fixation"));
    assert!(shapes.contains(&"looping"));
}

#[test]
fn fixation_span_at_the_head_of_a_longer_window_is_detected() {
    let events = vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(1, "grep", "grep(pattern=b, path=src)", false),
        ev(2, "grep", "grep(pattern=c, path=src)", false),
        ev(3, "grep", "grep(pattern=d, path=src)", false),
        ev(4, "read", "read(path=src/x.py)", false),
        ev(5, "edit", "edit(path=src/x.py)", false),
        ev(6, "test", "test(target=x)", false),
        ev(7, "read", "read(path=src/y.py)", false),
    ];
    let fixation = fixation_in(&common::make_trace(&events));
    assert_eq!(fixation.len(), 1);
    assert_eq!(pattern_of(&fixation[0]), vec!["grep"]);
    assert_eq!(span_of(&fixation[0]), (0, 3));
}

#[test]
fn fixation_span_at_the_tail_of_a_longer_window_is_detected() {
    let events = vec![
        ev(0, "read", "read(path=src/x.py)", false),
        ev(1, "edit", "edit(path=src/x.py)", false),
        ev(2, "test", "test(target=x)", false),
        ev(3, "read", "read(path=src/y.py)", false),
        ev(4, "grep", "grep(pattern=a, path=src)", false),
        ev(5, "grep", "grep(pattern=b, path=src)", false),
        ev(6, "grep", "grep(pattern=c, path=src)", false),
        ev(7, "grep", "grep(pattern=d, path=src)", false),
    ];
    let fixation = fixation_in(&common::make_trace(&events));
    assert_eq!(fixation.len(), 1);
    assert_eq!(pattern_of(&fixation[0]), vec!["grep"]);
    assert_eq!(span_of(&fixation[0]), (4, 7));
}

#[test]
fn dominant_tool_below_the_share_threshold_is_not_fixation() {
    // The grep calls are spread through the window, so every
    // subspan holding four of them also holds at least three
    // other calls and the grep share stays under three quarters
    // in all of them.
    let events = vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(1, "read", "read(path=src/x.py)", false),
        ev(2, "grep", "grep(pattern=b, path=src)", false),
        ev(3, "read", "read(path=src/y.py)", false),
        ev(4, "grep", "grep(pattern=c, path=src)", false),
        ev(5, "read", "read(path=src/z.py)", false),
        ev(6, "grep", "grep(pattern=d, path=src)", false),
        ev(7, "read", "read(path=src/w.py)", false),
    ];
    assert!(fixation_in(&common::make_trace(&events)).is_empty());
}

#[test]
fn fixation_needs_an_unused_available_tool() {
    let trace = make_trace_full(&fixation_events(), "trace-test", &["grep"], GOAL);
    let found = detect::detect(&trace, 0);
    assert!(detections_of(&found, "tool_fixation").is_empty());
}

#[test]
fn fixation_never_fires_with_no_available_tools() {
    let events = vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(1, "grep", "grep(pattern=b, path=src)", false),
        ev(2, "grep", "grep(pattern=c, path=src)", false),
        ev(3, "grep", "grep(pattern=d, path=src)", false),
    ];
    let trace = make_trace_full(&events, "trace-test", &[], GOAL);
    assert!(fixation_in(&trace).is_empty());
}

#[test]
fn short_no_progress_span_is_not_fixation() {
    let events = fixation_events()[..3].to_vec();
    assert!(fixation_in(&common::make_trace(&events)).is_empty());
}
