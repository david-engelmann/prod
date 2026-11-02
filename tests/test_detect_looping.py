"""PR-1. The looping detector."""

from prod import detect
from trace_helpers import ev, make_trace


def test_identical_call_run_is_detected_with_the_right_evidence_span():
    events = [
        ev(0, "read", "read(path=README)", progress=True),
        ev(1, "grep", "grep(pattern=config, path=src)"),
        ev(2, "grep", "grep(pattern=config, path=src)"),
        ev(3, "grep", "grep(pattern=config, path=src)"),
        ev(4, "grep", "grep(pattern=config, path=src)"),
    ]
    found = detect(make_trace(events))
    looping = [d for d in found if d["shape"] == "looping"]
    assert len(looping) == 1
    assert looping[0]["pattern"] == ["grep(pattern=config, path=src)"]
    assert looping[0]["evidence_span"] == {"start_step": 1, "end_step": 4}


def test_two_call_cycle_repeated_is_detected():
    calls = ["open(path=a)", "open(path=b)"] * 3
    events = [ev(i, "read", call) for i, call in enumerate(calls)]
    found = detect(make_trace(events))
    looping = [d for d in found if d["shape"] == "looping"]
    assert len(looping) == 1
    assert looping[0]["pattern"] == ["open(path=a)", "open(path=b)"]
    assert looping[0]["evidence_span"] == {"start_step": 0, "end_step": 5}


def test_two_looping_spans_in_one_window_are_both_reported():
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(1, "grep", "grep(pattern=a, path=src)"),
        ev(2, "grep", "grep(pattern=a, path=src)"),
        ev(3, "read", "read(path=src/x.py)"),
        ev(4, "read", "read(path=src/x.py)"),
        ev(5, "read", "read(path=src/x.py)"),
    ]
    looping = [x for x in detect(make_trace(events)) if x["shape"] == "looping"]
    assert len(looping) == 2
    assert looping[0]["evidence_span"] == {"start_step": 0, "end_step": 2}
    assert looping[1]["evidence_span"] == {"start_step": 3, "end_step": 5}


def test_three_call_cycle_repeated_is_detected():
    calls = ["open(path=a)", "open(path=b)", "open(path=c)"] * 2
    events = [ev(i, "read", call) for i, call in enumerate(calls)]
    looping = [x for x in detect(make_trace(events)) if x["shape"] == "looping"]
    assert len(looping) == 1
    assert looping[0]["evidence_span"] == {"start_step": 0, "end_step": 5}


def test_four_call_cycle_repeated_is_detected():
    calls = ["open(path=a)", "open(path=b)", "open(path=c)", "open(path=d)"] * 2
    events = [ev(i, "read", call) for i, call in enumerate(calls)]
    looping = [x for x in detect(make_trace(events)) if x["shape"] == "looping"]
    assert len(looping) == 1
    assert looping[0]["evidence_span"] == {"start_step": 0, "end_step": 7}


def test_two_identical_calls_are_not_a_loop():
    events = [
        ev(0, "grep", "grep(pattern=config, path=src)"),
        ev(1, "grep", "grep(pattern=config, path=src)"),
    ]
    assert [d for d in detect(make_trace(events)) if d["shape"] == "looping"] == []


def test_a_progress_call_inside_the_span_breaks_the_loop():
    events = [
        ev(0, "grep", "grep(pattern=config, path=src)"),
        ev(1, "grep", "grep(pattern=config, path=src)"),
        ev(2, "grep", "grep(pattern=config, path=src)", progress=True),
        ev(3, "grep", "grep(pattern=config, path=src)"),
        ev(4, "grep", "grep(pattern=config, path=src)"),
    ]
    assert [d for d in detect(make_trace(events)) if d["shape"] == "looping"] == []


def test_healthy_trace_with_varied_tool_use_produces_no_detection():
    events = [
        ev(0, "grep", "grep(pattern=config, path=src)"),
        ev(1, "read", "read(path=src/config.py)", progress=True),
        ev(2, "edit", "edit(path=src/config.py)"),
        ev(3, "test", "test(target=config)", progress=True),
        ev(4, "grep", "grep(pattern=other, path=docs)"),
        ev(5, "read", "read(path=docs/guide.md)", progress=True),
    ]
    assert detect(make_trace(events)) == []
