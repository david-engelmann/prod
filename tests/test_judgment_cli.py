"""PR-25. The intervene command with a fixtures directory."""

import json
import pathlib

from test_cli_e2e import run_cli

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"


def test_intervene_with_fixtures_records_the_judgments():
    result = run_cli(
        "intervene", "--scenario", "loop-needs-sharper", "--fixtures", str(FIXTURES)
    )
    assert result.returncode == 0, result.stderr
    record = json.loads(result.stdout)
    first, second = record["interventions"]
    assert first["severity"] == 0.82
    assert first["recovery"] == "still_stalled"
    assert "severity" not in second
    assert second["recovery"] == "recovering"
    assert record["outcome"] == "cleared"


def test_intervene_without_fixtures_carries_no_judgment_fields():
    result = run_cli("intervene", "--scenario", "loop-needs-sharper")
    assert result.returncode == 0, result.stderr
    record = json.loads(result.stdout)
    for intervention in record["interventions"]:
        assert "severity" not in intervention
        assert "recovery" not in intervention


def test_intervene_with_a_missing_fixtures_directory_exits_2(tmp_path):
    result = run_cli(
        "intervene",
        "--scenario",
        "loop-needs-sharper",
        "--fixtures",
        str(tmp_path / "nope"),
    )
    assert result.returncode == 2
    assert "Traceback" not in result.stderr
    assert result.stderr.count("\n") == 1


def test_intervene_with_a_broken_fixture_exits_2(tmp_path):
    pair = json.loads(
        (FIXTURES / "recovery-loop-needs-sharper-nudge.json").read_text(encoding="utf-8")
    )
    pair["response"]["confidence"] = 7
    (tmp_path / "broken.json").write_text(json.dumps(pair), encoding="utf-8")
    result = run_cli(
        "intervene", "--scenario", "loop-needs-sharper", "--fixtures", str(tmp_path)
    )
    assert result.returncode == 2
    assert "Traceback" not in result.stderr
    assert "broken.json" in result.stderr
