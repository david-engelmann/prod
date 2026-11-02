"""Command line entry point. python -m prod detect or intervene."""

import argparse
import json
import sys

from .detect import detect
from .judgment import FixturePlayer, JudgmentError
from .sandbox import SCENARIOS, run_session
from .trace import TraceError, load_trace


def _print_json(payload):
    sys.stdout.write(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def main(argv=None):
    parser = argparse.ArgumentParser(prog="prod")
    sub = parser.add_subparsers(dest="command", required=True)

    detect_parser = sub.add_parser("detect", help="run the stall detectors over a trace file")
    detect_parser.add_argument("--trace", required=True, help="path to a synthetic trace JSON file")

    intervene_parser = sub.add_parser("intervene", help="run a scripted sandbox session")
    intervene_parser.add_argument(
        "--scenario",
        required=True,
        choices=sorted(SCENARIOS),
        help="the scripted scenario to run",
    )
    intervene_parser.add_argument(
        "--fixtures",
        default=None,
        help="path to a directory of recorded judgment fixtures",
    )

    args = parser.parse_args(argv)
    try:
        if args.command == "detect":
            trace = load_trace(args.trace)
            _print_json({"trace_id": trace.trace_id, "detections": detect(trace)})
            return 0
        judgments = None
        if args.fixtures is not None:
            judgments = FixturePlayer(args.fixtures)
        record = run_session(args.scenario, judgments=judgments)
        _print_json(record)
        return 0
    except (TraceError, ValueError, JudgmentError) as exc:
        sys.stderr.write("error: " + str(exc) + "\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
