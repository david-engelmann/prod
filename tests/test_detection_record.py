"""PR-3. Every detection names the trace, the pattern, and the span."""

from prod import detect
from trace_helpers import ev, make_trace


def test_detection_carries_trace_pattern_and_evidence_span():
    events = [
        ev(10, "grep", "grep(pattern=config, path=src)"),
        ev(11, "grep", "grep(pattern=config, path=src)"),
        ev(12, "grep", "grep(pattern=config, path=src)"),
    ]
    trace = make_trace(events, trace_id="trace-named-007")
    found = detect(trace)
    assert len(found) == 1
    detection = found[0]
    assert detection["trace_id"] == "trace-named-007"
    assert detection["pattern"] == ["grep(pattern=config, path=src)"]
    assert detection["evidence_span"]["start_step"] == 10
    assert detection["evidence_span"]["end_step"] == 12


def test_fixation_detection_names_the_tool_as_its_pattern():
    calls = [
        "grep(pattern=a, path=src)",
        "grep(pattern=b, path=src)",
        "grep(pattern=c, path=src)",
        "grep(pattern=d, path=src)",
    ]
    events = [ev(i, "grep", call) for i, call in enumerate(calls)]
    trace = make_trace(events, trace_id="trace-fixation-003")
    found = [d for d in detect(trace) if d["shape"] == "tool_fixation"]
    assert len(found) == 1
    assert found[0]["trace_id"] == "trace-fixation-003"
    assert found[0]["pattern"] == ["grep"]
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 3}
