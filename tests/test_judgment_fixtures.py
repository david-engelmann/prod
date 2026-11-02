"""PR-20 and PR-22. The committed fixtures and their schema pin."""

import json
import pathlib

from prod import CONFIDENCE_THRESHOLD, FixturePlayer, validate_fixture

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"


def committed_pairs():
    pairs = []
    for path in sorted(FIXTURES.glob("*.json")):
        pairs.append((path.name, json.loads(path.read_text(encoding="utf-8"))))
    return pairs


def test_every_committed_fixture_matches_the_recorded_shape():
    pairs = committed_pairs()
    assert pairs, "no committed fixtures found"
    for name, pair in pairs:
        assert validate_fixture(pair, source=name) is pair


def test_every_committed_fixture_is_marked_synthetic():
    for name, pair in committed_pairs():
        assert pair.get("synthetic") is True, f"fixture {name} is not marked synthetic"


def test_the_player_loads_the_committed_directory():
    player = FixturePlayer(FIXTURES)
    for _, pair in committed_pairs():
        assert player.judge(pair["judgment_point"], pair["request"]) is not None


def test_the_committed_set_covers_both_judgment_points():
    points = {pair["judgment_point"] for _, pair in committed_pairs()}
    assert points == {"severity", "recovery"}


def test_the_committed_set_covers_every_recovery_label():
    labels = {
        pair["response"]["choice"]
        for _, pair in committed_pairs()
        if pair["response"].get("kind") == "choice"
    }
    assert labels == {"recovering", "still_stalled", "getting_worse"}


def test_each_point_has_an_abstention_or_below_threshold_case():
    for point in ("severity", "recovery"):
        responses = [
            pair["response"]
            for _, pair in committed_pairs()
            if pair["judgment_point"] == point
        ]
        assert any(
            response.get("kind") == "abstention"
            or response.get("confidence", 1.0) < CONFIDENCE_THRESHOLD
            for response in responses
        ), f"no fallback case recorded for the {point} point"
