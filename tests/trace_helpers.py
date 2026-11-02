"""Shared event and trace builders for the detector and record tests."""

from prod import Trace

TOOLS = ["grep", "read", "edit", "test", "finish"]
GOAL = "Find the config file and report its path"


def ev(step, tool, call, progress=False):
    return {"step": step, "tool": tool, "call": call, "progress": progress}


def make_trace(events, trace_id="trace-test", tools=TOOLS, goal=GOAL):
    return Trace.from_dict(
        {
            "trace_id": trace_id,
            "goal": goal,
            "tools_available": tools,
            "events": events,
        }
    )
