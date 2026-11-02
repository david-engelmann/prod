"""PR-27. The judgment seam hardening against malformed answers."""

import json
import pathlib

import pytest

from prod import (
    FixturePlayer,
    JudgmentError,
    check_answer,
    run_session,
    trusted,
    validate_fixture,
)

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"
HUGE = 10**400


def committed_pair(name):
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


def write_fixture(directory, name, pair):
    path = directory / name
    path.write_text(json.dumps(pair), encoding="utf-8")
    return path


class _StubSource:
    def __init__(self, answers):
        self._answers = answers

    def judge(self, point, request):
        return self._answers.get(point)


def test_a_list_kind_refuses_at_fixture_load_with_the_judgment_error(tmp_path):
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"]["kind"] = ["score"]
    with pytest.raises(JudgmentError) as caught:
        validate_fixture(pair, source="mutant")
    assert "kind" in str(caught.value)
    write_fixture(tmp_path, "listkind.json", pair)
    with pytest.raises(JudgmentError) as caught:
        FixturePlayer(tmp_path)
    assert "listkind.json" in str(caught.value)


def test_a_list_kind_from_a_foreign_source_counts_as_no_judgment():
    answer = {"kind": ["score"], "score": 0.5, "confidence": 0.9}
    assert check_answer("severity", answer) is None
    stub = _StubSource(
        {
            "severity": {"kind": ["score"], "score": 0.5, "confidence": 0.9},
            "recovery": {
                "kind": ["choice"],
                "choice": "recovering",
                "confidence": 0.95,
                "probabilities": {
                    "recovering": 0.95,
                    "still_stalled": 0.04,
                    "getting_worse": 0.01,
                },
            },
        }
    )
    judged = run_session("loop-needs-sharper", judgments=stub)
    plain = run_session("loop-needs-sharper")
    assert json.dumps(judged, sort_keys=True) == json.dumps(plain, sort_keys=True)


def test_a_non_string_kind_of_any_shape_never_raises_out_of_checking():
    for kind in (["score"], {"name": "score"}, 7, None):
        answer = {"kind": kind, "score": 0.5, "confidence": 0.9}
        assert check_answer("severity", answer) is None
        assert not trusted(answer)


def test_a_huge_integer_confidence_refuses_at_load_and_counts_as_no_judgment(tmp_path):
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"]["confidence"] = HUGE
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")
    write_fixture(tmp_path, "huge.json", pair)
    with pytest.raises(JudgmentError):
        FixturePlayer(tmp_path)
    answer = {"kind": "score", "score": 0.5, "confidence": HUGE}
    assert check_answer("severity", answer) is None
    assert not trusted(answer)
    stub = _StubSource({"severity": answer, "recovery": answer})
    judged = run_session("loop-needs-sharper", judgments=stub)
    assert json.dumps(judged, sort_keys=True) == json.dumps(
        run_session("loop-needs-sharper"), sort_keys=True
    )


def test_a_huge_integer_score_refuses_at_load_and_counts_as_no_judgment():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"]["score"] = HUGE
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")
    answer = {"kind": "score", "score": HUGE, "confidence": 0.9}
    assert check_answer("severity", answer) is None


def test_a_huge_integer_probability_refuses_at_load_and_counts_as_no_judgment():
    pair = committed_pair("recovery-loop-clears-on-nudge.json")
    pair["response"]["probabilities"]["recovering"] = HUGE
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")
    assert check_answer("recovery", pair["response"]) is None


def test_a_non_finite_number_is_not_a_number_for_the_seam():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"]["confidence"] = float("inf")
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")
    answer = {"kind": "score", "score": 0.5, "confidence": float("nan")}
    assert check_answer("severity", answer) is None
    assert not trusted(answer)
    infinite = {"kind": "score", "score": 0.5, "confidence": float("inf")}
    assert check_answer("severity", infinite) is None
    assert not trusted(infinite)
    huge = {"kind": "score", "score": 0.5, "confidence": HUGE}
    assert check_answer("severity", huge) is None
    assert not trusted(huge)


def test_extra_fields_of_mixed_types_are_refused_without_a_comparison_error():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["response"][123] = "x"
    pair["response"]["zzz"] = 1
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")
    answer = {"kind": "score", "score": 0.5, "confidence": 0.9, 123: "x", "zzz": 1}
    assert check_answer("severity", answer) is None
    assert check_answer("recovery", answer) is None


def test_an_extra_envelope_field_refuses_at_load_naming_it(tmp_path):
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    pair["mystery_field"] = "surprise"
    with pytest.raises(JudgmentError) as caught:
        validate_fixture(pair, source="mutant")
    assert "mystery_field" in str(caught.value)
    write_fixture(tmp_path, "extra.json", pair)
    with pytest.raises(JudgmentError) as caught:
        FixturePlayer(tmp_path)
    assert "mystery_field" in str(caught.value)
    assert "extra.json" in str(caught.value)


def test_the_exact_envelope_still_loads():
    pair = committed_pair("severity-loop-needs-sharper-opening.json")
    assert set(pair) == {"synthetic", "judgment_point", "request", "response"}
    assert validate_fixture(pair, source="mutant") is pair


def _recovery_pair_with_probabilities(probabilities):
    pair = committed_pair("recovery-loop-clears-on-nudge.json")
    pair["response"]["probabilities"] = probabilities
    return pair


def test_a_probability_sum_outside_the_named_band_is_refused():
    for probabilities in (
        {"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.1},
        {"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.202},
    ):
        pair = _recovery_pair_with_probabilities(probabilities)
        with pytest.raises(JudgmentError):
            validate_fixture(pair, source="mutant")
        assert check_answer("recovery", pair["response"]) is None


def test_a_probability_sum_inside_the_named_band_is_accepted():
    for probabilities in (
        {"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.1995},
        {"recovering": 0.5, "still_stalled": 0.3, "getting_worse": 0.2009},
    ):
        pair = _recovery_pair_with_probabilities(probabilities)
        assert validate_fixture(pair, source="mutant") is pair
        assert check_answer("recovery", pair["response"]) == pair["response"]
