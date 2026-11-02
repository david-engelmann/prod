"""Stall detectors over recorded traces.

Two failure shapes are detected in Round 1. Looping is the same call,
or a short cycle of calls, repeating without progress. Tool fixation
is one tool called over and over with varying calls while the other
available tools go unused and the goal does not advance.

A stall window is a maximal run of consecutive events in which no
event reports progress. Detectors only ever look inside stall
windows, so a span containing a progress call is never reported.
Both detectors search subspans inside a window, and a fixation
detection reports the shortest qualifying subspan for its tool
rather than the whole window.
"""

from bisect import bisect_left

from .trace import Trace

LOOP_MIN_RUN = 3
CYCLE_MIN_REPEATS = 2
CYCLE_MAX_LENGTH = 4
FIXATION_MIN_DOMINANT_CALLS = 4
FIXATION_MIN_SHARE_NUM = 3
FIXATION_MIN_SHARE_DEN = 4


def _stall_windows(events):
    """Maximal index ranges of consecutive events with no progress."""
    windows = []
    start = None
    for idx, event in enumerate(events):
        if event.progress:
            if start is not None:
                windows.append((start, idx - 1))
                start = None
        elif start is None:
            start = idx
    if start is not None:
        windows.append((start, len(events) - 1))
    return windows


def _detection(trace, shape, pattern, first, last, detail):
    return {
        "trace_id": trace.trace_id,
        "shape": shape,
        "pattern": list(pattern),
        "evidence_span": {
            "start_step": trace.events[first].step,
            "end_step": trace.events[last].step,
        },
        "detail": detail,
    }


def _first_loop_match(trace, first, last):
    calls = [event.call for event in trace.events[first : last + 1]]
    for offset in range(len(calls)):
        for length in range(1, CYCLE_MAX_LENGTH + 1):
            pattern = calls[offset : offset + length]
            if len(pattern) < length:
                break
            needed = LOOP_MIN_RUN if length == 1 else CYCLE_MIN_REPEATS
            repeats = 0
            pos = offset
            while calls[pos : pos + length] == pattern:
                repeats += 1
                pos += length
            if repeats >= needed:
                end = first + offset + repeats * length - 1
                detail = (
                    "call pattern of length "
                    + str(length)
                    + " repeated "
                    + str(repeats)
                    + " times with no progress"
                )
                return _detection(trace, "looping", pattern, first + offset, end, detail), end
    return None


def _looping_in_window(trace, first, last):
    found = []
    start = first
    while start <= last:
        match = _first_loop_match(trace, start, last)
        if match is None:
            break
        detection, end = match
        found.append(detection)
        start = end + 1
    return found


def _unused_available(trace, positions_by_tool, start, end):
    """Available tools that make no call inside the index range."""
    unused = []
    for tool in trace.tools_available:
        positions = positions_by_tool.get(tool)
        if not positions:
            unused.append(tool)
            continue
        idx = bisect_left(positions, start)
        if idx >= len(positions) or positions[idx] > end:
            unused.append(tool)
    return unused


def _fixation_span_for_tool(trace, positions_by_tool, tool):
    """The shortest span in the window where the tool fixates, or None.

    A qualifying span holds at least FIXATION_MIN_DOMINANT_CALLS calls
    to the tool, the tool makes at least three quarters of the calls
    in the span, the calls to the tool are not all identical, and at
    least one available tool goes unused in the span. The shortest
    qualifying span starts and ends on a call to the tool, so the
    search walks the tool's own call positions inside the window.
    Returns (length, start, end, dominant_calls, unused).
    """
    positions = positions_by_tool[tool]
    if len(positions) < FIXATION_MIN_DOMINANT_CALLS:
        return None
    events = trace.events
    calls = [events[idx].call for idx in positions]
    total = len(positions)
    # differs_at[i] is the first occurrence after i whose call differs
    # from the call at occurrence i, or None when there is none.
    differs_at = [None] * total
    for i in range(total - 2, -1, -1):
        if calls[i + 1] != calls[i]:
            differs_at[i] = i + 1
        else:
            differs_at[i] = differs_at[i + 1]
    best = None
    for i in range(total - FIXATION_MIN_DOMINANT_CALLS + 1):
        j = i + FIXATION_MIN_DOMINANT_CALLS - 1
        while j < total:
            start = positions[i]
            end = positions[j]
            length = end - start + 1
            if best is not None and length >= best[0]:
                break
            count = j - i + 1
            deficit = length * FIXATION_MIN_SHARE_NUM - count * FIXATION_MIN_SHARE_DEN
            if deficit > 0:
                # One more call to this tool can cut the deficit by at
                # most FIXATION_MIN_SHARE_DEN - FIXATION_MIN_SHARE_NUM,
                # which is one, so no occurrence before j + deficit
                # can reach the share threshold from this start.
                j += deficit
                continue
            if differs_at[i] is None:
                break
            if differs_at[i] > j:
                j = differs_at[i]
                continue
            unused = _unused_available(trace, positions_by_tool, start, end)
            if not unused:
                # A longer span from this start only uses more tools.
                break
            best = (length, start, end, count, unused)
            break
    return best


def _fixation_in_window(trace, first, last):
    positions_by_tool = {}
    for idx in range(first, last + 1):
        positions_by_tool.setdefault(trace.events[idx].tool, []).append(idx)
    best = None
    for tool in sorted(positions_by_tool):
        candidate = _fixation_span_for_tool(trace, positions_by_tool, tool)
        if candidate is None:
            continue
        length, start, end, count, unused = candidate
        key = (length, start, tool)
        if best is None or key < best[0]:
            best = (key, tool, start, end, count, unused)
    if best is None:
        return None
    _, tool, start, end, count, unused = best
    length = end - start + 1
    detail = (
        "tool "
        + tool
        + " made "
        + str(count)
        + " of "
        + str(length)
        + " calls with no progress while "
        + ", ".join(unused)
        + " went unused"
    )
    return _detection(trace, "tool_fixation", [tool], start, end, detail)


def detect(trace, from_step=0):
    """Every detection in the trace at or after from_step, in span order."""
    events = tuple(event for event in trace.events if event.step >= from_step)
    view = Trace(
        trace_id=trace.trace_id,
        goal=trace.goal,
        tools_available=trace.tools_available,
        events=events,
    )
    found = []
    for first, last in _stall_windows(view.events):
        found.extend(_looping_in_window(view, first, last))
        fixation = _fixation_in_window(view, first, last)
        if fixation is not None:
            found.append(fixation)
    found.sort(key=lambda d: (d["evidence_span"]["start_step"], d["shape"]))
    return found
