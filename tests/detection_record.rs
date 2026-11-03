//! PR-3. Every detection names the trace, the pattern, and the
//! span.

mod common;

use common::{detections_of, ev, make_trace_full, pattern_of, span_of, text_of};
use prod::detect;
use prod::pyjson::Value;

#[test]
fn detection_carries_trace_pattern_and_evidence_span() {
    let events = vec![
        ev(10, "grep", "grep(pattern=config, path=src)", false),
        ev(11, "grep", "grep(pattern=config, path=src)", false),
        ev(12, "grep", "grep(pattern=config, path=src)", false),
    ];
    let trace = make_trace_full(&events, "trace-named-007", &common::TOOLS, common::GOAL);
    let found = detect::detect(&trace, 0);
    assert_eq!(found.len(), 1);
    let detection = &found[0];
    assert_eq!(text_of(detection, "trace_id"), "trace-named-007");
    assert_eq!(
        pattern_of(detection),
        vec!["grep(pattern=config, path=src)"]
    );
    assert_eq!(span_of(detection), (10, 12));
}

#[test]
fn fixation_detection_names_the_tool_as_its_pattern() {
    let calls = [
        "grep(pattern=a, path=src)",
        "grep(pattern=b, path=src)",
        "grep(pattern=c, path=src)",
        "grep(pattern=d, path=src)",
    ];
    let events: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(step, call)| ev(step as i64, "grep", call, false))
        .collect();
    let trace = make_trace_full(&events, "trace-fixation-003", &common::TOOLS, common::GOAL);
    let found = detections_of(&detect::detect(&trace, 0), "tool_fixation");
    assert_eq!(found.len(), 1);
    assert_eq!(text_of(&found[0], "trace_id"), "trace-fixation-003");
    assert_eq!(pattern_of(&found[0]), vec!["grep"]);
    assert_eq!(span_of(&found[0]), (0, 3));
}
