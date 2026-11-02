"""PR-17. Trace loading rejects steps that are not strictly increasing."""

import pytest

from prod import Trace, TraceError
from trace_helpers import ev, make_trace


def test_equal_steps_are_rejected_at_load():
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(1, "grep", "grep(pattern=b, path=src)"),
        ev(1, "grep", "grep(pattern=c, path=src)"),
    ]
    with pytest.raises(TraceError):
        make_trace(events)


def test_decreasing_steps_are_rejected_at_load():
    events = [
        ev(0, "grep", "grep(pattern=a, path=src)"),
        ev(5, "grep", "grep(pattern=b, path=src)"),
        ev(3, "grep", "grep(pattern=c, path=src)"),
    ]
    with pytest.raises(TraceError):
        make_trace(events)


def test_steps_that_increase_by_more_than_one_are_accepted():
    events = [
        ev(10, "grep", "grep(pattern=a, path=src)"),
        ev(20, "grep", "grep(pattern=b, path=src)"),
        ev(30, "grep", "grep(pattern=c, path=src)"),
    ]
    trace = make_trace(events)
    assert isinstance(trace, Trace)
    assert [event.step for event in trace.events] == [10, 20, 30]
