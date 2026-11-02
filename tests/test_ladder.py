"""PR-4, PR-5, PR-6, PR-13, PR-14, PR-15. The first two ladder rungs and their order."""

import pytest

from prod import LadderError, apply_rung, build_intervention, next_rung

GOAL = "Find the config file and report its path"

DETECTION = {
    "trace_id": "trace-ladder-001",
    "shape": "looping",
    "pattern": ["grep(pattern=config, path=src)"],
    "evidence_span": {"start_step": 1, "end_step": 4},
    "detail": "call pattern of length 1 repeated 4 times with no progress",
}


def test_nudge_is_the_first_rung_offered():
    assert next_rung(DETECTION, []) == "nudge"


def test_nudge_record_names_the_rung_and_the_firing_detection():
    record = apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])
    assert record["rung"] == "nudge"
    assert record["at_step"] == 4
    assert record["fired_by"]["trace_id"] == "trace-ladder-001"
    assert record["fired_by"]["shape"] == "looping"
    assert record["fired_by"]["pattern"] == ["grep(pattern=config, path=src)"]
    assert record["fired_by"]["evidence_span"] == {"start_step": 1, "end_step": 4}
    assert "looping" in record["message"]
    assert GOAL in record["message"]


def test_sharper_prompt_cannot_fire_before_nudge():
    with pytest.raises(LadderError):
        apply_rung("sharper_prompt", DETECTION, GOAL, at_step=4, history=[])


def test_after_nudge_the_next_rung_is_sharper_prompt():
    history = [apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])]
    assert next_rung(DETECTION, history) == "sharper_prompt"


def test_sharper_prompt_record_names_the_pattern_and_restates_the_goal():
    history = [apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])]
    record = apply_rung("sharper_prompt", DETECTION, GOAL, at_step=9, history=history)
    assert record["rung"] == "sharper_prompt"
    assert record["fired_by"]["pattern"] == ["grep(pattern=config, path=src)"]
    assert "grep(pattern=config, path=src)" in record["message"]
    assert GOAL in record["message"]


def test_ladder_is_spent_after_both_rungs():
    history = [apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])]
    history.append(apply_rung("sharper_prompt", DETECTION, GOAL, at_step=9, history=history))
    assert next_rung(DETECTION, history) is None


def test_nudge_cannot_fire_twice_for_the_same_stall():
    history = [apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])]
    with pytest.raises(LadderError):
        apply_rung("nudge", DETECTION, GOAL, at_step=6, history=history)


def test_a_different_stall_starts_at_nudge_again():
    other = dict(DETECTION, shape="tool_fixation", pattern=["grep"])
    history = [apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])]
    assert next_rung(other, history) == "nudge"


def test_build_intervention_rejects_an_unknown_rung():
    with pytest.raises(LadderError):
        build_intervention("kill", DETECTION, GOAL, at_step=0)


def test_build_intervention_builds_a_nudge_with_no_history():
    record = build_intervention("nudge", DETECTION, GOAL, at_step=4)
    assert record["rung"] == "nudge"
    assert record["fired_by"]["trace_id"] == "trace-ladder-001"


def test_build_intervention_refuses_sharper_prompt_without_a_nudge_on_record():
    with pytest.raises(LadderError):
        build_intervention("sharper_prompt", DETECTION, GOAL, at_step=9)
    with pytest.raises(LadderError):
        build_intervention("sharper_prompt", DETECTION, GOAL, at_step=9, history=[])


def test_build_intervention_allows_sharper_prompt_after_a_nudge():
    history = [build_intervention("nudge", DETECTION, GOAL, at_step=4)]
    record = build_intervention("sharper_prompt", DETECTION, GOAL, at_step=9, history=history)
    assert record["rung"] == "sharper_prompt"
    assert GOAL in record["message"]


def test_build_intervention_refuses_a_rung_already_tried_for_the_stall():
    history = [build_intervention("nudge", DETECTION, GOAL, at_step=4)]
    with pytest.raises(LadderError):
        build_intervention("nudge", DETECTION, GOAL, at_step=6, history=history)


def test_second_sharper_prompt_is_refused_by_apply_rung():
    history = [apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])]
    history.append(apply_rung("sharper_prompt", DETECTION, GOAL, at_step=9, history=history))
    assert next_rung(DETECTION, history) is None
    with pytest.raises(LadderError):
        apply_rung("sharper_prompt", DETECTION, GOAL, at_step=12, history=history)


def test_second_sharper_prompt_is_refused_by_build_intervention():
    history = [apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])]
    history.append(apply_rung("sharper_prompt", DETECTION, GOAL, at_step=9, history=history))
    with pytest.raises(LadderError):
        build_intervention("sharper_prompt", DETECTION, GOAL, at_step=12, history=history)


@pytest.mark.parametrize(
    "field", ["trace_id", "shape", "pattern", "evidence_span"]
)
def test_detection_missing_a_field_is_refused_with_the_field_named(field):
    broken = {key: value for key, value in DETECTION.items() if key != field}
    with pytest.raises(LadderError) as excinfo:
        apply_rung("nudge", broken, GOAL, at_step=4, history=[])
    assert field in str(excinfo.value)
    with pytest.raises(LadderError) as excinfo:
        build_intervention("nudge", broken, GOAL, at_step=4)
    assert field in str(excinfo.value)


def test_history_record_missing_its_rung_is_refused_with_the_field_named():
    record = apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])
    broken = {key: value for key, value in record.items() if key != "rung"}
    with pytest.raises(LadderError) as excinfo:
        apply_rung("sharper_prompt", DETECTION, GOAL, at_step=9, history=[broken])
    assert "rung" in str(excinfo.value)


def test_history_record_missing_fired_by_is_refused_with_the_field_named():
    record = apply_rung("nudge", DETECTION, GOAL, at_step=4, history=[])
    broken = {key: value for key, value in record.items() if key != "fired_by"}
    with pytest.raises(LadderError) as excinfo:
        build_intervention("sharper_prompt", DETECTION, GOAL, at_step=9, history=[broken])
    assert "fired_by" in str(excinfo.value)


def test_malformed_records_never_surface_as_a_bare_key_error():
    with pytest.raises(LadderError):
        apply_rung("nudge", {"shape": "looping"}, GOAL, at_step=4, history=[])
    with pytest.raises(LadderError):
        next_rung(DETECTION, [{"rung": "nudge"}])
