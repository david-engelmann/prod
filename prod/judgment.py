"""Recorded decision model judgments, played back from fixtures.

The roadmap gives prod two judgment points after a detector fires.
Severity rates how deep a detected stall runs and answers with a
score. Recovery judges the trace segment recorded after a rung
landed and answers with one choice from recovering, still_stalled,
and getting_worse. No judgment decides whether a stall happened,
and no judgment fires, skips, or reorders a rung.

A judgment is recorded as a fixture that pairs one request with one
response in the shape a decision model returns. A choice response
carries the chosen label, a confidence, and a probability for every
label. A score response carries a numeric score and a confidence.
An abstention response carries neither. The fixture player answers
a request only from a recorded fixture whose request matches it
exactly, and abstains where no fixture matches. Everything here is
offline. The player reads files from one directory and nothing else.
"""

import json
import math
from pathlib import Path

from .ladder import RUNGS

CONFIDENCE_THRESHOLD = 0.6

JUDGMENT_POINTS = ("severity", "recovery")
RECOVERY_LABELS = ("recovering", "still_stalled", "getting_worse")

_KIND_FOR_POINT = {"severity": "score", "recovery": "choice"}


class JudgmentError(Exception):
    """A fixture file or fixtures directory does not match the format."""


def _fail(source, defect):
    raise JudgmentError(source + " " + defect)


def _is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _finite_float(value):
    """The finite float a numeric value converts to, or None.

    A value that cannot convert to a finite float, including an
    integer too large for a float and a non finite float, is not a
    number for the checks in this module, and asking never raises.
    """
    if not _is_number(value):
        return None
    try:
        converted = float(value)
    except (OverflowError, ValueError):
        return None
    if not math.isfinite(converted):
        return None
    return converted


def _check_unit_interval(source, field, value):
    converted = _finite_float(value)
    if converted is None or not 0.0 <= converted <= 1.0:
        _fail(source, "field '" + field + "' is not a number between 0 and 1")
    return converted


def _check_pattern(source, request):
    pattern = request.get("pattern")
    if not isinstance(pattern, list) or not all(isinstance(item, str) for item in pattern):
        _fail(source, "request field 'pattern' is not a list of strings")


def _check_trace_id(source, request):
    trace_id = request.get("trace_id")
    if not isinstance(trace_id, str) or not trace_id:
        _fail(source, "request field 'trace_id' is not a non-empty string")


def _validate_severity_request(source, request):
    _check_trace_id(source, request)
    if not isinstance(request.get("shape"), str) or not request.get("shape"):
        _fail(source, "request field 'shape' is not a non-empty string")
    _check_pattern(source, request)
    span = request.get("evidence_span")
    if not isinstance(span, dict):
        _fail(source, "request field 'evidence_span' is not an object")
    start = span.get("start_step")
    end = span.get("end_step")
    if not isinstance(start, int) or isinstance(start, bool):
        _fail(source, "request evidence span field 'start_step' is not an integer")
    if not isinstance(end, int) or isinstance(end, bool):
        _fail(source, "request evidence span field 'end_step' is not an integer")
    if end < start:
        _fail(source, "request evidence span runs backwards")


def _validate_recovery_request(source, request):
    _check_trace_id(source, request)
    if request.get("rung") not in RUNGS:
        _fail(source, "request field 'rung' is not a ladder rung")
    if not isinstance(request.get("shape"), str) or not request.get("shape"):
        _fail(source, "request field 'shape' is not a non-empty string")
    _check_pattern(source, request)
    events = request.get("events")
    if not isinstance(events, list) or not events:
        _fail(source, "request field 'events' is not a non-empty list")
    previous = None
    for raw in events:
        if not isinstance(raw, dict):
            _fail(source, "request event is not an object")
        step = raw.get("step")
        if not isinstance(step, int) or isinstance(step, bool):
            _fail(source, "request event field 'step' is not an integer")
        if not isinstance(raw.get("tool"), str) or not isinstance(raw.get("call"), str):
            _fail(source, "request event fields 'tool' and 'call' must be strings")
        if not isinstance(raw.get("progress"), bool):
            _fail(source, "request event field 'progress' is not a boolean")
        if previous is not None and step <= previous:
            _fail(source, "request event steps are not strictly increasing")
        previous = step


_FIELDS_FOR_KIND = {
    "abstention": {"kind"},
    "score": {"kind", "score", "confidence"},
    "choice": {"kind", "choice", "confidence", "probabilities"},
}


def _validate_response(source, point, response):
    if not isinstance(response, dict):
        _fail(source, "response is not an object")
    kind = response.get("kind")
    if not isinstance(kind, str):
        _fail(source, "response field 'kind' is not a string")
    if kind in _FIELDS_FOR_KIND:
        extra = set(response) - _FIELDS_FOR_KIND[kind]
        if extra:
            _fail(
                source,
                "response carries the field "
                + repr(sorted(extra)[0])
                + ", which a "
                + kind
                + " response does not carry",
            )
    if kind == "abstention":
        return
    expected = _KIND_FOR_POINT[point]
    if kind != expected:
        _fail(
            source,
            "response kind "
            + repr(kind)
            + " does not answer the "
            + point
            + " point, which takes a "
            + expected
            + " or an abstention",
        )
    _check_unit_interval(source, "confidence", response.get("confidence"))
    if kind == "score":
        _check_unit_interval(source, "score", response.get("score"))
        return
    choice = response.get("choice")
    if choice not in RECOVERY_LABELS:
        _fail(source, "response field 'choice' is not one of the recovery labels")
    probabilities = response.get("probabilities")
    if not isinstance(probabilities, dict):
        _fail(source, "response field 'probabilities' is not an object")
    if set(probabilities) != set(RECOVERY_LABELS):
        _fail(source, "response probabilities do not cover exactly the recovery labels")
    total = 0.0
    for label in RECOVERY_LABELS:
        total += _check_unit_interval(
            source, "probabilities." + label, probabilities[label]
        )
    if abs(total - 1.0) > 0.001:
        _fail(source, "response probabilities do not add up to 1")


_ENVELOPE_FIELDS = {"synthetic", "judgment_point", "request", "response"}


def validate_fixture(data, source="fixture"):
    """One recorded fixture pair, checked against the recorded shape.

    Returns the pair unchanged when it is well formed and raises
    JudgmentError naming the source and the defect when it is not.
    """
    if not isinstance(data, dict):
        _fail(source, "is not an object")
    extra = set(data) - _ENVELOPE_FIELDS
    if extra:
        _fail(
            source,
            "carries the top level field "
            + repr(sorted(extra, key=repr)[0])
            + ", which the fixture envelope does not carry",
        )
    if data.get("synthetic") is not True:
        _fail(source, "is not marked synthetic")
    point = data.get("judgment_point")
    if point not in JUDGMENT_POINTS:
        _fail(source, "field 'judgment_point' is not a judgment point")
    request = data.get("request")
    if not isinstance(request, dict):
        _fail(source, "field 'request' is not an object")
    if point == "severity":
        _validate_severity_request(source, request)
    else:
        _validate_recovery_request(source, request)
    if "response" not in data:
        _fail(source, "carries no response")
    _validate_response(source, point, data["response"])
    return data


def _request_key(point, request):
    return json.dumps({"point": point, "request": request}, sort_keys=True)


def load_fixture_file(path):
    target = Path(path)
    try:
        text = target.read_text(encoding="utf-8")
    except OSError as exc:
        raise JudgmentError("fixture file cannot be read, " + str(path)) from exc
    except UnicodeDecodeError as exc:
        raise JudgmentError("fixture file is not valid UTF-8, " + str(path)) from exc
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise JudgmentError("fixture file is not valid JSON, " + str(path)) from exc
    return validate_fixture(data, source="fixture " + target.name)


class FixturePlayer:
    """Answers judgment requests from the recorded fixtures in a directory.

    Every JSON file in the directory holds one fixture pair. Pairs
    are validated as they load, so a response that changes shape
    fails here, loudly, and never reaches the ladder.
    """

    def __init__(self, directory):
        target = Path(directory)
        if not target.is_dir():
            raise JudgmentError("fixtures directory cannot be read, " + str(directory))
        self._answers = {}
        for path in sorted(target.glob("*.json")):
            pair = load_fixture_file(path)
            key = _request_key(pair["judgment_point"], pair["request"])
            if key in self._answers:
                raise JudgmentError(
                    "fixture "
                    + path.name
                    + " records the same request as an earlier fixture"
                )
            self._answers[key] = pair["response"]

    def judge(self, point, request):
        """The recorded response for this request, or None to abstain."""
        response = self._answers.get(_request_key(point, request))
        if response is None:
            return None
        return json.loads(json.dumps(response))


def check_answer(point, response):
    """The answer itself when it is well formed for its point, else None.

    A judgment source that is not the fixture player can hand back
    anything. An answer outside the recorded shape for its point is
    treated as no judgment at all, never trusted and never recorded,
    so a malformed answer cannot crash the session or reach a record.
    """
    if response is None:
        return None
    try:
        _validate_response("answer", point, response)
    except JudgmentError:
        return None
    return response


def trusted(response):
    """Whether a judgment answer may be used at all.

    An abstention and an answer whose confidence falls below the
    threshold in the contract both count as no judgment at all.
    """
    if not isinstance(response, dict):
        return False
    if response.get("kind") not in ("score", "choice"):
        return False
    confidence = _finite_float(response.get("confidence"))
    return confidence is not None and confidence >= CONFIDENCE_THRESHOLD


def severity_request(detection):
    """The severity request the ladder asks about one detection."""
    return {
        "trace_id": detection["trace_id"],
        "shape": detection["shape"],
        "pattern": list(detection["pattern"]),
        "evidence_span": dict(detection["evidence_span"]),
    }


def recovery_request(record, segment):
    """The recovery request for one rung over its post intervention segment."""
    fired_by = record["fired_by"]
    return {
        "trace_id": fired_by["trace_id"],
        "rung": record["rung"],
        "shape": fired_by["shape"],
        "pattern": list(fired_by["pattern"]),
        "events": [dict(event) for event in segment],
    }
