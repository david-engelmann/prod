//! PR-17. Trace loading rejects steps that are not strictly
//! increasing.

mod common;

use common::{GOAL, TOOLS, ev, trace_doc};
use westering::trace::Trace;

#[test]
fn equal_steps_are_rejected_at_load() {
    let events = vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(1, "grep", "grep(pattern=b, path=src)", false),
        ev(1, "grep", "grep(pattern=c, path=src)", false),
    ];
    let doc = trace_doc(&events, "trace-test", &TOOLS, GOAL);
    assert!(Trace::from_dict(&doc).is_err());
}

#[test]
fn decreasing_steps_are_rejected_at_load() {
    let events = vec![
        ev(0, "grep", "grep(pattern=a, path=src)", false),
        ev(5, "grep", "grep(pattern=b, path=src)", false),
        ev(3, "grep", "grep(pattern=c, path=src)", false),
    ];
    let doc = trace_doc(&events, "trace-test", &TOOLS, GOAL);
    assert!(Trace::from_dict(&doc).is_err());
}

#[test]
fn steps_that_increase_by_more_than_one_are_accepted() {
    let events = vec![
        ev(10, "grep", "grep(pattern=a, path=src)", false),
        ev(20, "grep", "grep(pattern=b, path=src)", false),
        ev(30, "grep", "grep(pattern=c, path=src)", false),
    ];
    let doc = trace_doc(&events, "trace-test", &TOOLS, GOAL);
    let trace = Trace::from_dict(&doc).expect("trace loads");
    let steps: Vec<String> = trace
        .events
        .iter()
        .map(|event| event.step.clone())
        .collect();
    assert_eq!(steps, vec!["10", "20", "30"]);
}
