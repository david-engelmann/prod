"""The graded rescue ladder, first two rungs.

Escalation order is fixed. Nudge fires before sharper prompt. A rung
fires only when every lower rung has been tried for that stall, where
a lower rung tried on the same pattern earlier in the session counts
as tried, and a rung that has already been tried for a stall never
fires again for it. Every public entry point that builds an
intervention enforces the same order against the session history.
The later rungs named by the contract, smaller subtask, fork, and
kill, are not implemented in this round, and kill is the last rung
that belongs to deadman and is never reimplemented here.

Records handed to the ladder are validated before they are read. A
detection record or a history record that is missing a field the
ladder needs is refused with LadderError naming the missing field.
"""

RUNGS = ("nudge", "sharper_prompt")

_DETECTION_FIELDS = ("trace_id", "shape", "pattern", "evidence_span")


class LadderError(Exception):
    """A rung was asked to fire out of order, or a record was incomplete."""


def _check_pattern(owner, record):
    pattern = record["pattern"]
    if not isinstance(pattern, (list, tuple)) or not all(
        isinstance(item, str) for item in pattern
    ):
        raise LadderError(owner + " field 'pattern' is not a list of strings")


def _validate_detection(detection):
    if not isinstance(detection, dict):
        raise LadderError("detection record is not an object")
    for field in _DETECTION_FIELDS:
        if field not in detection:
            raise LadderError("detection record is missing field '" + field + "'")
    _check_pattern("detection record", detection)
    span = detection["evidence_span"]
    if not isinstance(span, dict):
        raise LadderError("detection record field 'evidence_span' is not an object")
    for field in ("start_step", "end_step"):
        if field not in span:
            raise LadderError(
                "detection record evidence span is missing field '" + field + "'"
            )


def _validate_history_record(record):
    if not isinstance(record, dict):
        raise LadderError("history record is not an object")
    if "rung" not in record:
        raise LadderError("history record is missing field 'rung'")
    if "fired_by" not in record:
        raise LadderError("history record is missing field 'fired_by'")
    fired_by = record["fired_by"]
    if not isinstance(fired_by, dict):
        raise LadderError("history record field 'fired_by' is not an object")
    for field in ("shape", "pattern"):
        if field not in fired_by:
            raise LadderError(
                "history record fired_by is missing field '" + field + "'"
            )
    _check_pattern("history record fired_by", fired_by)


def _pattern_key(detection):
    return (detection["shape"], tuple(detection["pattern"]))


def _tried_rungs(detection, history):
    _validate_detection(detection)
    key = _pattern_key(detection)
    tried = set()
    for record in history:
        _validate_history_record(record)
        fired_by = record["fired_by"]
        if (fired_by["shape"], tuple(fired_by["pattern"])) == key:
            tried.add(record["rung"])
    return tried


def next_rung(detection, history):
    """The next rung that may fire for this stall, or None when spent."""
    tried = _tried_rungs(detection, history)
    for rung in RUNGS:
        if rung not in tried:
            return rung
    return None


def _enforce_order(rung, detection, history):
    if rung not in RUNGS:
        raise LadderError("unknown rung " + str(rung))
    tried = _tried_rungs(detection, history)
    if rung in tried:
        raise LadderError(rung + " has already been tried for this stall")
    lower = RUNGS[: RUNGS.index(rung)]
    missing = [name for name in lower if name not in tried]
    if missing:
        raise LadderError(
            rung + " cannot fire before " + " and ".join(missing) + " for the same stall"
        )


def build_intervention(rung, detection, goal, at_step, history=()):
    """The recorded intervention for one rung firing on one detection.

    The escalation order is enforced against history, which defaults
    to an empty session, so a nudge can be built with no history and
    a sharper prompt built with no nudge on record is refused.
    """
    _enforce_order(rung, detection, history)
    if rung == "nudge":
        message = (
            "Stop. Your recent work stalled in the shape "
            + detection["shape"]
            + ". Change approach and work toward the goal. The goal is "
            + goal
            + "."
        )
    else:
        message = (
            "New instruction. Your previous approach stalled. The pattern was "
            + " then ".join(detection["pattern"])
            + " in the shape "
            + detection["shape"]
            + ", and it made no progress. Do not repeat that pattern. The goal is "
            + goal
            + ". Take a different first step toward it now."
        )
    return {
        "rung": rung,
        "fired_by": {
            "trace_id": detection["trace_id"],
            "shape": detection["shape"],
            "pattern": list(detection["pattern"]),
            "evidence_span": dict(detection["evidence_span"]),
        },
        "message": message,
        "at_step": at_step,
    }


def apply_rung(rung, detection, goal, at_step, history):
    """Build an intervention while enforcing the escalation order."""
    return build_intervention(rung, detection, goal, at_step, history)
