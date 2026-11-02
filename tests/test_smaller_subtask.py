"""PR-30 to PR-34. The smaller subtask rung, its order, and its seam."""

import json
import pathlib

import pytest

from prod import (
    SCENARIOS,
    FixturePlayer,
    LadderError,
    apply_rung,
    build_intervention,
    next_rung,
    run_session,
)
from test_cli_e2e import run_cli

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"

GOAL = "Find the config file and report its path"

DETECTION = {
    "trace_id": "trace-ladder-001",
    "shape": "looping",
    "pattern": ["grep(pattern=config, path=src)"],
    "evidence_span": {"start_step": 1, "end_step": 4},
    "detail": "call pattern of length 1 repeated 4 times with no progress",
}


def history_through(*rungs):
    history = []
    step = 4
    for rung in rungs:
        history.append(apply_rung(rung, DETECTION, GOAL, at_step=step, history=history))
        step += 5
    return history


def test_after_both_lower_rungs_the_next_rung_is_smaller_subtask():
    history = history_through("nudge", "sharper_prompt")
    assert next_rung(DETECTION, history) == "smaller_subtask"


def test_subtask_record_carries_the_same_fields_as_the_earlier_rungs():
    history = history_through("nudge", "sharper_prompt")
    record = apply_rung("smaller_subtask", DETECTION, GOAL, at_step=14, history=history)
    assert record["rung"] == "smaller_subtask"
    assert record["at_step"] == 14
    assert record["fired_by"]["trace_id"] == "trace-ladder-001"
    assert record["fired_by"]["shape"] == "looping"
    assert record["fired_by"]["pattern"] == ["grep(pattern=config, path=src)"]
    assert record["fired_by"]["evidence_span"] == {"start_step": 1, "end_step": 4}
    assert set(record) == {"rung", "fired_by", "message", "at_step"}


def test_subtask_message_names_the_pattern_and_the_shape_and_restates_the_goal():
    history = history_through("nudge", "sharper_prompt")
    record = apply_rung("smaller_subtask", DETECTION, GOAL, at_step=14, history=history)
    assert "grep(pattern=config, path=src)" in record["message"]
    assert "looping" in record["message"]
    assert GOAL in record["message"]


def test_subtask_message_bounds_the_next_step_to_the_evidence_span():
    history = history_through("nudge", "sharper_prompt")
    record = apply_rung("smaller_subtask", DETECTION, GOAL, at_step=14, history=history)
    assert "steps 1 to 4" in record["message"]


def test_subtask_bound_follows_the_evidence_span_of_the_detection():
    other = dict(DETECTION, evidence_span={"start_step": 10, "end_step": 17})
    history = history_through("nudge", "sharper_prompt")
    record = apply_rung("smaller_subtask", other, GOAL, at_step=20, history=history)
    assert "steps 10 to 17" in record["message"]
    assert "steps 1 to 4" not in record["message"]


def test_subtask_record_refuses_a_detection_missing_a_field():
    broken = {key: value for key, value in DETECTION.items() if key != "evidence_span"}
    history = history_through("nudge", "sharper_prompt")
    with pytest.raises(LadderError) as excinfo:
        apply_rung("smaller_subtask", broken, GOAL, at_step=14, history=history)
    assert "evidence_span" in str(excinfo.value)


def test_subtask_cannot_fire_with_no_lower_rung_tried():
    with pytest.raises(LadderError):
        apply_rung("smaller_subtask", DETECTION, GOAL, at_step=4, history=[])
    with pytest.raises(LadderError):
        build_intervention("smaller_subtask", DETECTION, GOAL, at_step=4)


def test_subtask_cannot_fire_with_only_the_nudge_tried():
    history = history_through("nudge")
    with pytest.raises(LadderError):
        apply_rung("smaller_subtask", DETECTION, GOAL, at_step=9, history=history)
    with pytest.raises(LadderError):
        build_intervention("smaller_subtask", DETECTION, GOAL, at_step=9, history=history)


def test_subtask_cannot_fire_with_only_the_sharper_prompt_tried():
    # A sharper prompt with no nudge before it cannot be built through
    # the ladder, so the history record is written out by hand in the
    # shape the ladder itself writes.
    history = [
        {
            "rung": "sharper_prompt",
            "fired_by": {
                "trace_id": DETECTION["trace_id"],
                "shape": DETECTION["shape"],
                "pattern": list(DETECTION["pattern"]),
                "evidence_span": dict(DETECTION["evidence_span"]),
            },
            "message": "recorded elsewhere",
            "at_step": 9,
        }
    ]
    assert next_rung(DETECTION, history) == "nudge"
    with pytest.raises(LadderError):
        apply_rung("smaller_subtask", DETECTION, GOAL, at_step=14, history=history)


def test_subtask_cannot_fire_twice_for_the_same_stall():
    history = history_through("nudge", "sharper_prompt", "smaller_subtask")
    with pytest.raises(LadderError):
        apply_rung("smaller_subtask", DETECTION, GOAL, at_step=20, history=history)
    with pytest.raises(LadderError):
        build_intervention("smaller_subtask", DETECTION, GOAL, at_step=20, history=history)


def test_lower_rungs_tried_on_a_different_stall_do_not_unlock_the_subtask():
    other = dict(DETECTION, shape="tool_fixation", pattern=["grep"])
    history = history_through("nudge", "sharper_prompt")
    assert next_rung(other, history) == "nudge"
    with pytest.raises(LadderError):
        apply_rung("smaller_subtask", other, GOAL, at_step=14, history=history)


def test_fork_and_kill_are_not_rungs_of_this_ladder():
    history = history_through("nudge", "sharper_prompt", "smaller_subtask")
    for rung in ("fork", "kill"):
        with pytest.raises(LadderError):
            build_intervention(rung, DETECTION, GOAL, at_step=20, history=history)


def test_the_subtask_scenario_is_in_the_committed_scenario_set():
    assert "loop-needs-subtask" in SCENARIOS


def test_agent_that_ignores_both_lower_rungs_clears_on_the_subtask():
    record = run_session("loop-needs-subtask")
    assert record["stall_cleared"] is True
    assert record["outcome"] == "cleared"
    assert [i["rung"] for i in record["interventions"]] == [
        "nudge",
        "sharper_prompt",
        "smaller_subtask",
    ]
    assert record["cleared_by"] == "smaller_subtask"
    assert record["interventions"][-1]["at_step"] < len(record["events"])


def test_sessions_that_clear_before_the_subtask_keep_their_round_3_shape():
    record = run_session("loop-clears-on-nudge")
    assert [i["rung"] for i in record["interventions"]] == ["nudge"]
    assert len(record["events"]) == 6
    record = run_session("loop-needs-sharper")
    assert [i["rung"] for i in record["interventions"]] == ["nudge", "sharper_prompt"]
    assert len(record["events"]) == 9
    record = run_session("fixation-clears-on-nudge")
    assert [i["rung"] for i in record["interventions"]] == ["nudge"]
    assert len(record["events"]) == 7


def test_committed_subtask_fixture_request_is_the_builders_output():
    pair = json.loads(
        (FIXTURES / "recovery-loop-needs-subtask-subtask.json").read_text(encoding="utf-8")
    )
    captured = {}

    class Capture:
        def judge(self, point, request):
            if point == "recovery" and request["rung"] == "smaller_subtask":
                captured["request"] = request
            return None

    run_session("loop-needs-subtask", judgments=Capture())
    assert captured["request"] == pair["request"]
    assert pair["request"]["rung"] == "smaller_subtask"
    assert pair["response"]["kind"] == "choice"


def test_trusted_subtask_recovery_is_recorded_on_the_subtask_record():
    player = FixturePlayer(FIXTURES)
    record = run_session("loop-needs-subtask", judgments=player)
    third = record["interventions"][2]
    assert third["rung"] == "smaller_subtask"
    assert third["recovery"] == "recovering"
    plain = run_session("loop-needs-subtask")
    assert {k: v for k, v in third.items() if k != "recovery"} == plain["interventions"][2]


def test_judgments_never_change_the_subtask_session():
    player = FixturePlayer(FIXTURES)
    plain = run_session("loop-needs-subtask")
    judged = run_session("loop-needs-subtask", judgments=player)
    assert [i["rung"] for i in judged["interventions"]] == [
        i["rung"] for i in plain["interventions"]
    ]
    assert judged["events"] == plain["events"]
    assert judged["detections"] == plain["detections"]
    assert judged["outcome"] == plain["outcome"]


def test_intervene_fires_the_subtask_end_to_end():
    result = run_cli("intervene", "--scenario", "loop-needs-subtask")
    assert result.returncode == 0, result.stderr
    record = json.loads(result.stdout)
    assert [i["rung"] for i in record["interventions"]] == [
        "nudge",
        "sharper_prompt",
        "smaller_subtask",
    ]
    assert record["cleared_by"] == "smaller_subtask"
    assert record["outcome"] == "cleared"


def test_intervene_with_fixtures_records_the_subtask_recovery():
    result = run_cli(
        "intervene", "--scenario", "loop-needs-subtask", "--fixtures", str(FIXTURES)
    )
    assert result.returncode == 0, result.stderr
    record = json.loads(result.stdout)
    assert record["interventions"][2]["recovery"] == "recovering"
