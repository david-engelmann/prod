"""End to end CLI tests for detect and intervene."""

import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]


def run_cli(*args):
    env = dict(os.environ)
    env["PYTHONPATH"] = str(ROOT)
    return subprocess.run(
        [sys.executable, "-m", "prod", *args],
        capture_output=True,
        text=True,
        cwd=str(ROOT),
        env=env,
    )


def test_detect_looping_example_matches_the_committed_output():
    result = run_cli("detect", "--trace", "examples/looping_trace.json")
    assert result.returncode == 0, result.stderr
    committed = (ROOT / "examples" / "looping_detections.json").read_text(encoding="utf-8")
    assert result.stdout == committed


def test_detect_fixation_example_matches_the_committed_output():
    result = run_cli("detect", "--trace", "examples/fixation_trace.json")
    assert result.returncode == 0, result.stderr
    committed = (ROOT / "examples" / "fixation_detections.json").read_text(encoding="utf-8")
    assert result.stdout == committed


def test_detect_output_is_byte_identical_across_two_runs():
    first = run_cli("detect", "--trace", "examples/looping_trace.json")
    second = run_cli("detect", "--trace", "examples/looping_trace.json")
    assert first.returncode == 0 and second.returncode == 0
    assert first.stdout == second.stdout


def test_intervene_runs_a_sandbox_session_end_to_end():
    result = run_cli("intervene", "--scenario", "loop-needs-sharper")
    assert result.returncode == 0, result.stderr
    record = json.loads(result.stdout)
    assert record["outcome"] == "cleared"
    assert [i["rung"] for i in record["interventions"]] == ["nudge", "sharper_prompt"]


def test_detect_on_a_missing_trace_exits_2_without_a_traceback(tmp_path):
    result = run_cli("detect", "--trace", str(tmp_path / "nope.json"))
    assert result.returncode == 2
    assert "Traceback" not in result.stderr


def test_detect_on_invalid_json_exits_2_without_a_traceback(tmp_path):
    bad = tmp_path / "bad.json"
    bad.write_text("{not json", encoding="utf-8")
    result = run_cli("detect", "--trace", str(bad))
    assert result.returncode == 2
    assert "Traceback" not in result.stderr


def test_detect_on_non_monotonic_steps_exits_2_without_a_traceback(tmp_path):
    bad = tmp_path / "unordered.json"
    payload = {
        "trace_id": "trace-unordered",
        "goal": "Find the config file and report its path",
        "tools_available": ["grep", "read"],
        "events": [
            {"step": 0, "tool": "grep", "call": "grep(pattern=a, path=src)", "progress": False},
            {"step": 2, "tool": "grep", "call": "grep(pattern=b, path=src)", "progress": False},
            {"step": 1, "tool": "grep", "call": "grep(pattern=c, path=src)", "progress": False},
        ],
    }
    bad.write_text(json.dumps(payload), encoding="utf-8")
    result = run_cli("detect", "--trace", str(bad))
    assert result.returncode == 2
    assert "Traceback" not in result.stderr
