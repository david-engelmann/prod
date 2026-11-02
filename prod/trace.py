"""Recorded synthetic session traces.

A trace is a JSON object with a trace_id, a goal, a list of the tools
available to the agent, and a list of events. Every event carries the
step number, the tool that was called, the call signature, and whether
the call advanced the goal. Event steps must be strictly increasing,
so an evidence span over a loaded trace can never run backwards.
"""

import json
from dataclasses import dataclass
from pathlib import Path


class TraceError(Exception):
    """A trace file or trace object does not match the trace format."""


@dataclass(frozen=True)
class Event:
    step: int
    tool: str
    call: str
    progress: bool


@dataclass(frozen=True)
class Trace:
    trace_id: str
    goal: str
    tools_available: tuple
    events: tuple

    @classmethod
    def from_dict(cls, data):
        if not isinstance(data, dict):
            raise TraceError("trace is not an object")
        trace_id = data.get("trace_id")
        goal = data.get("goal")
        tools = data.get("tools_available")
        raw_events = data.get("events")
        if not isinstance(trace_id, str) or not trace_id:
            raise TraceError("trace_id is not a non-empty string")
        if not isinstance(goal, str):
            raise TraceError("goal is not a string")
        if not isinstance(tools, list) or not all(isinstance(t, str) for t in tools):
            raise TraceError("tools_available is not a list of strings")
        if not isinstance(raw_events, list):
            raise TraceError("events is not a list")
        events = []
        for raw in raw_events:
            if not isinstance(raw, dict):
                raise TraceError("trace event is not an object")
            step = raw.get("step")
            tool = raw.get("tool")
            call = raw.get("call")
            progress = raw.get("progress")
            if not isinstance(step, int) or isinstance(step, bool):
                raise TraceError("event step is not an integer")
            if not isinstance(tool, str) or not isinstance(call, str):
                raise TraceError("event tool and call must be strings")
            if not isinstance(progress, bool):
                raise TraceError("event progress is not a boolean")
            events.append(Event(step=step, tool=tool, call=call, progress=progress))
        for earlier, later in zip(events, events[1:]):
            if later.step <= earlier.step:
                raise TraceError("event steps are not strictly increasing")
        return cls(
            trace_id=trace_id,
            goal=goal,
            tools_available=tuple(tools),
            events=tuple(events),
        )


def load_trace(path):
    target = Path(path)
    try:
        text = target.read_text(encoding="utf-8")
    except OSError as exc:
        raise TraceError("trace file cannot be read, " + str(path)) from exc
    except UnicodeDecodeError as exc:
        raise TraceError("trace file is not valid UTF-8, " + str(path)) from exc
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise TraceError("trace file is not valid JSON, " + str(path)) from exc
    return Trace.from_dict(data)
