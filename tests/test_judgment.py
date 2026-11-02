"""PR-20 to PR-24. The fixture player and the judgment wiring."""

import copy
import json
import pathlib

import pytest

from prod import (
    FixturePlayer,
    JudgmentError,
    build_intervention,
    detect,
    run_session,
    trusted,
    validate_fixture,
)
from trace_helpers import ev, make_trace

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"


def committed_player():
    return FixturePlayer(FIXTURES)


def committed_pair(name):
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


def write_fixture(directory, name, pair):
    path = directory / name
    path.write_text(json.dumps(pair), encoding="utf-8")
    return path


def test_player_answers_a_recorded_request_exactly():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    player = committed_player()
    answer = player.judge("severity", pair["request"])
    assert answer == pair["response"]


def test_player_abstains_where_no_fixture_matches():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    request = copy.deepcopy(pair["request"])
    request["evidence_span"]["end_step"] += 1
    assert committed_player().judge("severity", request) is None


def test_player_returns_an_abstention_response_where_one_is_recorded():
    pair = committed_pair("severity-loop-clears-on-nudge.json")
    answer = committed_player().judge("severity", pair["request"])
    assert answer == {"kind": "abstention"}
    assert not trusted(answer)


def test_player_refuses_a_fixture_that_breaks_the_shape(tmp_path):
    pair = committed_pair("recovery-loop-needs-sharper-nudge.json")
    del pair["response"]["probabilities"]
    write_fixture(tmp_path, "broken.json", pair)
    with pytest.raises(JudgmentError) as caught:
        FixturePlayer(tmp_path)
    assert "broken.json" in str(caught.value)


def test_player_refuses_two_fixtures_for_the_same_request(tmp_path):
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    write_fixture(tmp_path, "one.json", pair)
    write_fixture(tmp_path, "two.json", pair)
    with pytest.raises(JudgmentError):
        FixturePlayer(tmp_path)


def test_player_refuses_a_missing_directory(tmp_path):
    with pytest.raises(JudgmentError):
        FixturePlayer(tmp_path / "nope")


def test_validator_refuses_a_choice_with_a_missing_label_probability():
    pair = committed_pair("recovery-loop-needs-sharper-nudge.json")
    del pair["response"]["probabilities"]["getting_worse"]
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_response_for_the_wrong_point():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["judgment_point"] = "recovery"
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_confidence_outside_the_unit_interval():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"]["confidence"] = 1.4
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_score_response_carrying_a_choice_field():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"]["choice"] = "recovering"
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_an_abstention_carrying_a_choice_and_a_score():
    pair = committed_pair("severity-fixation-clears-on-nudge.json")
    pair["response"] = {
        "kind": "abstention",
        "choice": "recovering",
        "score": 0.9,
        "confidence": 0.99,
    }
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


class _StubSource:
    def __init__(self, answers):
        self._answers = answers

    def judge(self, point, request):
        return self._answers.get(point)


def test_a_malformed_foreign_answer_counts_as_no_judgment():
    stub = _StubSource(
        {
            "severity": {"kind": "score", "confidence": 0.9},
            "recovery": {"kind": "choice", "confidence": 0.95},
        }
    )
    judged = run_session("loop-needs-sharper", judgments=stub)
    plain = run_session("loop-needs-sharper")
    assert judged == plain


def test_a_wrong_kind_foreign_answer_counts_as_no_judgment():
    stub = _StubSource(
        {
            "severity": {
                "kind": "choice",
                "choice": "recovering",
                "confidence": 1.0,
                "probabilities": {
                    "recovering": 1.0,
                    "still_stalled": 0.0,
                    "getting_worse": 0.0,
                },
            },
            "recovery": {"kind": "score", "score": 0.5, "confidence": 1.0},
        }
    )
    judged = run_session("loop-needs-sharper", judgments=stub)
    plain = run_session("loop-needs-sharper")
    assert judged == plain


def test_validator_refuses_a_fixture_not_marked_synthetic():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["synthetic"] = False
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_trust_follows_the_contract_threshold():
    assert trusted({"kind": "score", "score": 0.5, "confidence": 0.6})
    assert not trusted({"kind": "score", "score": 0.5, "confidence": 0.59})
    assert not trusted({"kind": "choice", "choice": "recovering", "confidence": 0.2})
    assert not trusted({"kind": "abstention"})
    assert not trusted(None)


def test_ladder_records_a_handed_in_severity_and_nothing_else_changes():
    events = [ev(i, "grep", "grep(pattern=config, path=src)") for i in range(3)]
    detection = detect(make_trace(events))[0]
    plain = build_intervention("nudge", detection, "goal", 2)
    stamped = build_intervention("nudge", detection, "goal", 2, severity=0.82)
    assert "severity" not in plain
    assert stamped["severity"] == 0.82
    assert {k: v for k, v in stamped.items() if k != "severity"} == plain


def test_trusted_severity_is_recorded_on_the_intervention_record():
    record = run_session("loop-needs-sharper", judgments=committed_player())
    first, second = record["interventions"]
    assert first["severity"] == 0.82
    plain = run_session("loop-needs-sharper")
    assert {k: v for k, v in first.items() if k not in ("severity", "recovery")} == {
        k: v for k, v in plain["interventions"][0].items()
    }
    assert second["rung"] == "sharper_prompt"


def test_below_threshold_severity_leaves_the_record_as_it_was():
    record = run_session("loop-needs-sharper", judgments=committed_player())
    plain = run_session("loop-needs-sharper")
    second = record["interventions"][1]
    assert "severity" not in second
    assert {k: v for k, v in second.items() if k != "recovery"} == plain["interventions"][1]


def test_a_low_score_with_a_trusted_confidence_is_still_recorded():
    record = run_session("loop-never-clears", judgments=committed_player())
    assert record["interventions"][0]["severity"] == 0.35


def test_an_abstaining_source_leaves_the_session_bit_for_bit(tmp_path):
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"] = {"kind": "abstention"}
    write_fixture(tmp_path, "abstain.json", pair)
    record = run_session("loop-needs-sharper", judgments=FixturePlayer(tmp_path))
    assert record == run_session("loop-needs-sharper")


def test_a_below_threshold_source_leaves_the_session_bit_for_bit(tmp_path):
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"] = {"kind": "score", "score": 0.99, "confidence": 0.59}
    write_fixture(tmp_path, "quiet.json", pair)
    record = run_session("loop-needs-sharper", judgments=FixturePlayer(tmp_path))
    assert record == run_session("loop-needs-sharper")


def test_recovery_choices_are_recorded_verbatim():
    record = run_session("loop-needs-sharper", judgments=committed_player())
    first, second = record["interventions"]
    assert first["recovery"] == "still_stalled"
    assert second["recovery"] == "recovering"
    record = run_session("loop-never-clears", judgments=committed_player())
    assert record["interventions"][1]["recovery"] == "getting_worse"


def test_untrusted_and_abstaining_recovery_answers_record_nothing():
    record = run_session("loop-never-clears", judgments=committed_player())
    assert "recovery" not in record["interventions"][0]
    record = run_session("fixation-clears-on-nudge", judgments=committed_player())
    assert "recovery" not in record["interventions"][0]
    assert record["interventions"][0]["severity"] == 0.64


def test_judgments_never_change_rungs_or_outcomes():
    player = committed_player()
    for name in (
        "loop-clears-on-nudge",
        "loop-needs-sharper",
        "fixation-clears-on-nudge",
        "loop-never-clears",
    ):
        plain = run_session(name)
        judged = run_session(name, judgments=player)
        assert [i["rung"] for i in judged["interventions"]] == [
            i["rung"] for i in plain["interventions"]
        ]
        assert judged["outcome"] == plain["outcome"]
        assert judged["events"] == plain["events"]
        assert judged["detections"] == plain["detections"]
