"""PR-19. One fixation detection per dominant tool in a window."""

from prod import detect
from trace_helpers import ev, make_trace

TWO_TOOL_EVENTS = [
    ev(0, "grep", "grep(pattern=a, path=src)"),
    ev(1, "grep", "grep(pattern=b, path=src)"),
    ev(2, "grep", "grep(pattern=c, path=src)"),
    ev(3, "grep", "grep(pattern=d, path=src)"),
    ev(4, "read", "read(path=src/a.py)"),
    ev(5, "read", "read(path=src/b.py)"),
    ev(6, "read", "read(path=src/c.py)"),
    ev(7, "read", "read(path=src/d.py)"),
]


def fixation_in(trace):
    return [d for d in detect(trace) if d["shape"] == "tool_fixation"]


def test_two_dominant_tools_in_one_window_yield_two_detections():
    fixation = fixation_in(make_trace(TWO_TOOL_EVENTS))
    assert len(fixation) == 2
    assert fixation[0]["pattern"] == ["grep"]
    assert fixation[0]["evidence_span"] == {"start_step": 0, "end_step": 3}
    assert fixation[1]["pattern"] == ["read"]
    assert fixation[1]["evidence_span"] == {"start_step": 4, "end_step": 7}


def test_detections_from_one_window_come_in_span_start_order():
    events = list(reversed(TWO_TOOL_EVENTS))
    events = [ev(i, e["tool"], e["call"]) for i, e in enumerate(events)]
    fixation = fixation_in(make_trace(events))
    assert [d["pattern"] for d in fixation] == [["read"], ["grep"]]
    assert fixation[0]["evidence_span"] == {"start_step": 0, "end_step": 3}
    assert fixation[1]["evidence_span"] == {"start_step": 4, "end_step": 7}


def test_a_second_tool_below_the_call_count_yields_one_detection():
    events = TWO_TOOL_EVENTS[:7] + [ev(7, "edit", "edit(path=src/a.py)")]
    fixation = fixation_in(make_trace(events))
    assert len(fixation) == 1
    assert fixation[0]["pattern"] == ["grep"]
    assert fixation[0]["evidence_span"] == {"start_step": 0, "end_step": 3}


def test_a_second_tool_with_identical_calls_is_not_fixation():
    events = TWO_TOOL_EVENTS[:4] + [
        ev(4, "read", "read(path=src/a.py)"),
        ev(5, "read", "read(path=src/a.py)"),
        ev(6, "read", "read(path=src/a.py)"),
        ev(7, "read", "read(path=src/a.py)"),
    ]
    fixation = fixation_in(make_trace(events))
    assert len(fixation) == 1
    assert fixation[0]["pattern"] == ["grep"]


def test_detections_never_merge_spans_across_tools():
    fixation = fixation_in(make_trace(TWO_TOOL_EVENTS))
    assert len(fixation) == 2
    for detection in fixation:
        assert len(detection["pattern"]) == 1
    spans = {
        (d["pattern"][0], d["evidence_span"]["start_step"], d["evidence_span"]["end_step"])
        for d in fixation
    }
    assert spans == {("grep", 0, 3), ("read", 4, 7)}
    assert all(
        d["evidence_span"] != {"start_step": 0, "end_step": 7} for d in fixation
    )
