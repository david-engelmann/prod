"""PR-2 and PR-12. The tool fixation detector and its subspan search."""

from prod import detect
from trace_helpers import ev, make_trace

FIXATION_EVENTS = [
    ev(0, "grep", "grep(pattern=config, path=src)"),
    ev(1, "grep", "grep(pattern=settings, path=src)"),
    ev(2, "grep", "grep(pattern=config, path=docs)"),
    ev(3, "grep", "grep(pattern=conf, path=src)"),
    ev(4, "grep", "grep(pattern=config, path=tests)"),
]


def fixation_in(trace):
    return [d for d in detect(trace) if d["shape"] == "tool_fixation"]


def test_fixation_trace_is_detected_with_its_span():
    fixation = fixation_in(make_trace(FIXATION_EVENTS, trace_id="trace-fixation"))
    assert len(fixation) == 1
    assert fixation[0]["pattern"] == ["grep"]
    # PR-12. The reported span is the shortest qualifying subspan, the
    # four calls at steps 0 to 3, not the whole five call window.
    assert fixation[0]["evidence_span"] == {"start_step": 0, "end_step": 3}
    assert "4 of 4" in fixation[0]["detail"]


def test_identical_calls_to_one_tool_are_looping_not_fixation():
    events = [ev(i, "grep", "grep(pattern=config, path=src)") for i in range(5)]
    shapes = {d["shape"] for d in detect(make_trace(events))}
    assert "tool_fixation" not in shapes
    assert "looping" in shapes


def test_fixation_span_at_the_head_of_a_longer_window_is_detected():
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(1, "grep", "grep(pattern=b, path=src)"),
        ev(2, "grep", "grep(pattern=c, path=src)"),
        ev(3, "grep", "grep(pattern=d, path=src)"),
        ev(4, "read", "read(path=src/x.py)"),
        ev(5, "edit", "edit(path=src/x.py)"),
        ev(6, "test", "test(target=x)"),
        ev(7, "read", "read(path=src/y.py)"),
    ]
    fixation = fixation_in(make_trace(events))
    assert len(fixation) == 1
    assert fixation[0]["pattern"] == ["grep"]
    assert fixation[0]["evidence_span"] == {"start_step": 0, "end_step": 3}


def test_fixation_span_at_the_tail_of_a_longer_window_is_detected():
    events = [
        ev(0, "read", "read(path=src/x.py)"),
        ev(1, "edit", "edit(path=src/x.py)"),
        ev(2, "test", "test(target=x)"),
        ev(3, "read", "read(path=src/y.py)"),
        ev(4, "grep", "grep(pattern=a, path=src)"),
        ev(5, "grep", "grep(pattern=b, path=src)"),
        ev(6, "grep", "grep(pattern=c, path=src)"),
        ev(7, "grep", "grep(pattern=d, path=src)"),
    ]
    fixation = fixation_in(make_trace(events))
    assert len(fixation) == 1
    assert fixation[0]["pattern"] == ["grep"]
    assert fixation[0]["evidence_span"] == {"start_step": 4, "end_step": 7}


def test_dominant_tool_below_the_share_threshold_is_not_fixation():
    # The grep calls are spread through the window, so every subspan
    # holding four of them also holds at least three other calls and
    # the grep share stays under three quarters in all of them.
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(1, "read", "read(path=src/x.py)"),
        ev(2, "grep", "grep(pattern=b, path=src)"),
        ev(3, "read", "read(path=src/y.py)"),
        ev(4, "grep", "grep(pattern=c, path=src)"),
        ev(5, "read", "read(path=src/z.py)"),
        ev(6, "grep", "grep(pattern=d, path=src)"),
        ev(7, "read", "read(path=src/w.py)"),
    ]
    assert fixation_in(make_trace(events)) == []


def test_fixation_needs_an_unused_available_tool():
    events = list(FIXATION_EVENTS)
    found = detect(make_trace(events, tools=["grep"]))
    assert [d for d in found if d["shape"] == "tool_fixation"] == []


def test_fixation_never_fires_with_no_available_tools():
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(1, "grep", "grep(pattern=b, path=src)"),
        ev(2, "grep", "grep(pattern=c, path=src)"),
        ev(3, "grep", "grep(pattern=d, path=src)"),
    ]
    assert fixation_in(make_trace(events, tools=[])) == []


def test_short_no_progress_span_is_not_fixation():
    events = FIXATION_EVENTS[:3]
    assert fixation_in(make_trace(events)) == []
