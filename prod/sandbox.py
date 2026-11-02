"""The local sandbox harness.

A scripted test agent is driven by a fixed script held here. It
emits stall events until it receives an intervention it responds
to, and then it emits recovery events that make progress and finish.
The harness runs the detectors over the session as it grows, fires
the ladder in order, and records the whole session as one timeline
of events and interventions. Nothing here touches a network, a wall
clock, or any agent outside this module, and nothing is left to chance.

A session may run with a judgment source, any object that answers
judge(point, request) with a recorded response or with no answer.
The harness then asks for a severity score before a rung fires and
records a trusted score on the intervention record, and asks for a
recovery choice over the segment that follows each rung and records
a trusted choice on that record. With no judgment source, or where
an answer is missing or untrusted, the session record is exactly
the record the harness produced before the seam existed.
"""

from .detect import detect
from .judgment import check_answer, recovery_request, severity_request, trusted
from .ladder import apply_rung, next_rung
from .trace import Event, Trace

MAX_STEPS = 200


class ScriptedAgent:
    """A test agent whose behaviour is fixed by its scenario script."""

    def __init__(self, scenario):
        self.scenario = scenario
        self.goal = scenario["goal"]
        self.tools_available = tuple(scenario["tools_available"])
        self._stall = list(scenario["stall_events"])
        self._recovery = list(scenario["recovery_events"])
        self._responds_to = set(scenario["responds_to"])
        self._mode = "stall"
        self._index = 0
        self.finished = False

    def receive(self, intervention):
        if intervention["rung"] in self._responds_to and self._mode == "stall":
            self._mode = "recover"
            self._index = 0

    def next_event(self):
        if self._mode == "recover":
            if self._index >= len(self._recovery):
                self.finished = True
                return None
            raw = self._recovery[self._index]
        else:
            raw = self._stall[self._index % len(self._stall)]
        self._index += 1
        if raw["tool"] == "finish":
            self.finished = True
        return raw


def _scenario(name, goal, stall_events, recovery_events, responds_to):
    return {
        "name": name,
        "goal": goal,
        "tools_available": ["grep", "read", "edit", "test", "finish"],
        "stall_events": stall_events,
        "recovery_events": recovery_events,
        "responds_to": responds_to,
    }


_LOOP_STALL = [
    {"tool": "grep", "call": "grep(pattern=config, path=src)", "progress": False},
]
_FIXATION_STALL = [
    {"tool": "grep", "call": "grep(pattern=config, path=src)", "progress": False},
    {"tool": "grep", "call": "grep(pattern=settings, path=src)", "progress": False},
    {"tool": "grep", "call": "grep(pattern=config, path=docs)", "progress": False},
    {"tool": "grep", "call": "grep(pattern=conf, path=src)", "progress": False},
]
_RECOVERY = [
    {"tool": "read", "call": "read(path=src/config.py)", "progress": True},
    {"tool": "test", "call": "test(target=config)", "progress": True},
    {"tool": "finish", "call": "finish(summary=config located)", "progress": True},
]

SCENARIOS = {
    "loop-clears-on-nudge": _scenario(
        "loop-clears-on-nudge",
        "Find the config file and report its path",
        _LOOP_STALL,
        _RECOVERY,
        ("nudge",),
    ),
    "loop-needs-sharper": _scenario(
        "loop-needs-sharper",
        "Find the config file and report its path",
        _LOOP_STALL,
        _RECOVERY,
        ("sharper_prompt",),
    ),
    "fixation-clears-on-nudge": _scenario(
        "fixation-clears-on-nudge",
        "Find the config file and report its path",
        _FIXATION_STALL,
        _RECOVERY,
        ("nudge",),
    ),
    "loop-never-clears": _scenario(
        "loop-never-clears",
        "Find the config file and report its path",
        _LOOP_STALL,
        _RECOVERY,
        (),
    ),
}


def run_session(scenario_name, judgments=None):
    """Run one scripted sandbox session and return its full record.

    The optional judgments source is asked at the two judgment
    points the contract states. It never changes which rung fires
    or when, only what the intervention records carry.
    """
    if scenario_name not in SCENARIOS:
        raise ValueError("unknown scenario " + str(scenario_name))
    scenario = SCENARIOS[scenario_name]
    agent = ScriptedAgent(scenario)
    events = []
    interventions = []
    detections = []
    last_intervention_step = -1
    stalled_out = False
    pending = None

    def assess_recovery(upto_step):
        nonlocal pending
        if judgments is None or pending is None:
            pending = None
            return
        record = pending
        pending = None
        segment = [
            event
            for event in events
            if record["at_step"] < event["step"] <= upto_step
        ]
        if not segment:
            return
        response = check_answer(
            "recovery", judgments.judge("recovery", recovery_request(record, segment))
        )
        if trusted(response):
            record["recovery"] = response["choice"]

    while not agent.finished and len(events) < MAX_STEPS:
        raw = agent.next_event()
        if raw is None:
            break
        step = len(events)
        events.append(
            {"step": step, "tool": raw["tool"], "call": raw["call"], "progress": raw["progress"]}
        )
        if agent.finished:
            break
        trace = Trace(
            trace_id="sandbox-" + scenario_name,
            goal=scenario["goal"],
            tools_available=agent.tools_available,
            events=tuple(
                Event(step=e["step"], tool=e["tool"], call=e["call"], progress=e["progress"])
                for e in events
            ),
        )
        fresh = detect(trace, from_step=last_intervention_step + 1)
        if not fresh:
            continue
        detection = fresh[0]
        rung = next_rung(detection, interventions)
        if rung is None:
            assess_recovery(step)
            stalled_out = True
            detections.append(detection)
            break
        assess_recovery(step)
        severity = None
        if judgments is not None:
            answer = check_answer(
                "severity", judgments.judge("severity", severity_request(detection))
            )
            if trusted(answer):
                severity = answer["score"]
        record = apply_rung(
            rung, detection, scenario["goal"], step, interventions, severity=severity
        )
        interventions.append(record)
        detections.append(detection)
        agent.receive(record)
        last_intervention_step = step
        pending = record
    if events:
        assess_recovery(events[-1]["step"])
    stall_cleared = agent.finished and not stalled_out
    return {
        "session_id": "sandbox-" + scenario_name,
        "goal": scenario["goal"],
        "events": events,
        "detections": detections,
        "interventions": interventions,
        "stall_cleared": stall_cleared,
        "outcome": "cleared" if stall_cleared else "stalled",
        "cleared_by": interventions[-1]["rung"] if stall_cleared and interventions else None,
    }
