"""PR-7 and PR-9. The sandbox harness and the escalation outcomes."""

from prod import run_session


def test_session_record_is_one_timeline_with_steps_on_interventions():
    record = run_session("loop-needs-sharper")
    assert record["session_id"] == "sandbox-loop-needs-sharper"
    steps = [event["step"] for event in record["events"]]
    assert steps == list(range(len(steps)))
    for intervention in record["interventions"]:
        assert 0 <= intervention["at_step"] < len(record["events"])
    first = record["interventions"][0]
    before = [e for e in record["events"] if e["step"] <= first["at_step"]]
    after = [e for e in record["events"] if e["step"] > first["at_step"]]
    assert before and after
    assert all(not e["progress"] for e in before)


def test_stall_that_clears_after_nudge_never_reaches_the_second_rung():
    record = run_session("loop-clears-on-nudge")
    assert record["stall_cleared"] is True
    assert record["outcome"] == "cleared"
    assert [i["rung"] for i in record["interventions"]] == ["nudge"]
    assert record["cleared_by"] == "nudge"


def test_stall_that_survives_nudge_escalates_to_sharper_prompt():
    record = run_session("loop-needs-sharper")
    assert record["stall_cleared"] is True
    assert [i["rung"] for i in record["interventions"]] == ["nudge", "sharper_prompt"]
    assert record["cleared_by"] == "sharper_prompt"


def test_fixation_scenario_is_rescued_by_nudge():
    record = run_session("fixation-clears-on-nudge")
    assert record["stall_cleared"] is True
    assert record["detections"][0]["shape"] == "tool_fixation"
    assert [i["rung"] for i in record["interventions"]] == ["nudge"]


# Round 4 alteration. This pin asserted the ignoring agent ends with
# the two Round 1 rungs on its timeline. PR-32 extends the same
# outcome over the third rung the ladder gained, so the timeline now
# records all three rungs in order. The claim the pin carries, that
# an agent ignoring every rung ends stalled with every rung tried,
# is unchanged.
def test_agent_that_ignores_every_rung_ends_stalled_with_all_recorded():
    record = run_session("loop-never-clears")
    assert record["stall_cleared"] is False
    assert record["outcome"] == "stalled"
    assert [i["rung"] for i in record["interventions"]] == [
        "nudge",
        "sharper_prompt",
        "smaller_subtask",
    ]


def test_intervention_records_name_the_rung_and_the_firing_detection():
    record = run_session("loop-needs-sharper")
    assert len(record["interventions"]) == len(record["detections"])
    for intervention, detection in zip(record["interventions"], record["detections"]):
        assert intervention["fired_by"]["shape"] == detection["shape"]
        assert intervention["fired_by"]["pattern"] == detection["pattern"]
        assert intervention["fired_by"]["evidence_span"] == detection["evidence_span"]
        assert intervention["fired_by"]["trace_id"] == record["session_id"]
