"""PR-28. The looping match order and the window segmentation."""

from prod import detect
from trace_helpers import ev, make_trace


def looping(events):
    return [d for d in detect(make_trace(events)) if d["shape"] == "looping"]


def test_the_earlier_offset_wins_even_when_the_later_pattern_is_longer():
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(1, "grep", "grep(pattern=a, path=src)"),
        ev(2, "grep", "grep(pattern=a, path=src)"),
        ev(3, "read", "read(path=src/b.py)"),
        ev(4, "read", "read(path=src/c.py)"),
        ev(5, "read", "read(path=src/b.py)"),
        ev(6, "read", "read(path=src/c.py)"),
        ev(7, "read", "read(path=src/b.py)"),
        ev(8, "read", "read(path=src/c.py)"),
    ]
    found = looping(events)
    assert len(found) == 2
    assert found[0]["pattern"] == ["grep(pattern=a, path=src)"]
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 2}
    assert found[1]["pattern"] == ["read(path=src/b.py)", "read(path=src/c.py)"]
    assert found[1]["evidence_span"] == {"start_step": 3, "end_step": 8}


def test_the_shortest_length_wins_at_one_offset():
    events = [ev(i, "grep", "grep(pattern=a, path=src)") for i in range(6)]
    found = looping(events)
    assert len(found) == 1
    assert found[0]["pattern"] == ["grep(pattern=a, path=src)"]
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 5}
    assert found[0]["detail"] == (
        "call pattern of length 1 repeated 6 times with no progress"
    )


def test_the_maximal_repeat_count_sets_the_evidence_span_end():
    events = [ev(i, "grep", "grep(pattern=a, path=src)") for i in range(5)]
    events.append(ev(5, "read", "read(path=src/other.py)"))
    found = looping(events)
    assert len(found) == 1
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 4}
    assert found[0]["detail"] == (
        "call pattern of length 1 repeated 5 times with no progress"
    )


def test_a_maximal_cycle_repeat_sets_the_evidence_span_end():
    calls = ["read(path=src/b.py)", "read(path=src/c.py)"] * 3
    events = [ev(i, "read", call) for i, call in enumerate(calls)]
    events.append(ev(6, "grep", "grep(pattern=other, path=docs)"))
    found = looping(events)
    assert len(found) == 1
    assert found[0]["pattern"] == ["read(path=src/b.py)", "read(path=src/c.py)"]
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 5}
    assert found[0]["detail"] == (
        "call pattern of length 2 repeated 3 times with no progress"
    )


def test_two_loops_in_one_window_are_separated_by_the_resume_rule():
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(1, "grep", "grep(pattern=a, path=src)"),
        ev(2, "grep", "grep(pattern=a, path=src)"),
        ev(3, "read", "read(path=src/x.py)"),
        ev(4, "read", "read(path=src/x.py)"),
        ev(5, "read", "read(path=src/x.py)"),
    ]
    found = looping(events)
    assert len(found) == 2
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 2}
    assert found[1]["evidence_span"] == {"start_step": 3, "end_step": 5}


def test_a_pattern_beginning_inside_a_reported_match_is_never_reported():
    events = [ev(i, "grep", "grep(pattern=a, path=src)") for i in range(4)]
    found = looping(events)
    assert len(found) == 1
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 3}


def test_a_pattern_inside_a_reported_span_is_reported_only_by_its_tail():
    p = "read(path=src/b.py)"
    q = "read(path=src/c.py)"
    calls = [p, q, p, q, q, q, q]
    events = [ev(i, "read", call) for i, call in enumerate(calls)]
    found = looping(events)
    assert len(found) == 2
    assert found[0]["pattern"] == [p, q]
    assert found[0]["evidence_span"] == {"start_step": 0, "end_step": 3}
    assert found[1]["pattern"] == [q]
    assert found[1]["evidence_span"] == {"start_step": 4, "end_step": 6}
