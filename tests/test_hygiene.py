"""PR-8, PR-10, PR-11, PR-18. Sandbox only, deterministic, synthetic."""

import ast
import json
import pathlib

from prod import run_session

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "prod"

# Modules whose import gives the package a network, wall clock, or
# randomness capability, or a route to the process environment.
FORBIDDEN_IMPORTS = {
    "socket",
    "urllib",
    "requests",
    "http",
    "time",
    "datetime",
    "random",
    "subprocess",
}

# Attribute access that reaches a forbidden capability through a
# module the package is otherwise allowed to import.
FORBIDDEN_ATTRIBUTES = {
    ("os", "environ"),
    ("os", "popen"),
    ("os", "system"),
    ("time", "time"),
}


def forbidden_uses(source):
    """Forbidden capabilities used by a module, read from its syntax tree.

    Imports and attribute access are the evidence. Prose in docstrings
    and comments is never matched, so an ordinary word that merely
    contains a forbidden name is not a finding.
    """
    tree = ast.parse(source)
    offenders = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                root = alias.name.split(".")[0]
                if root in FORBIDDEN_IMPORTS:
                    offenders.append("import " + alias.name)
        elif isinstance(node, ast.ImportFrom) and node.module and node.level == 0:
            root = node.module.split(".")[0]
            if root in FORBIDDEN_IMPORTS:
                offenders.append("from " + node.module + " import")
        elif isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name):
            if (node.value.id, node.attr) in FORBIDDEN_ATTRIBUTES:
                offenders.append(node.value.id + "." + node.attr)
    return offenders


def test_package_uses_only_the_standard_library():
    imported = set()
    for path in PACKAGE.glob("*.py"):
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imported.update(alias.name.split(".")[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.module and node.level == 0:
                imported.add(node.module.split(".")[0])
    allowed = set(__import__("sys").stdlib_module_names)
    assert imported <= allowed, f"non standard library imports found, {imported - allowed}"


def test_package_has_no_network_wall_clock_or_randomness_capabilities():
    offenders = []
    for path in PACKAGE.glob("*.py"):
        for use in forbidden_uses(path.read_text(encoding="utf-8")):
            offenders.append((path.name, use))
    assert not offenders, f"forbidden capabilities in package source, {offenders}"


def test_prose_mentioning_randomness_is_not_a_forbidden_use():
    source = '"""A harness where nothing is left to randomness."""\n\nVALUE = 1\n'
    assert forbidden_uses(source) == []


def test_os_system_and_popen_are_forbidden_uses():
    assert forbidden_uses("import os\nos.system('curl example')\n") == ["os.system"]
    assert forbidden_uses("import os\nos.popen('curl example')\n") == ["os.popen"]


def test_a_real_random_import_is_a_forbidden_use():
    assert forbidden_uses("import random\n") == ["import random"]
    assert forbidden_uses("from random import shuffle\n") == ["from random import"]


def test_session_output_is_byte_identical_across_runs():
    first = json.dumps(run_session("loop-needs-sharper"), indent=2, sort_keys=True)
    second = json.dumps(run_session("loop-needs-sharper"), indent=2, sort_keys=True)
    assert first == second


def test_examples_and_fixtures_are_synthetic():
    examples = ROOT / "examples"
    traces = sorted(examples.glob("*_trace.json"))
    assert traces, "no example traces found"
    for path in traces:
        data = json.loads(path.read_text(encoding="utf-8"))
        assert data.get("synthetic") is True, f"example {path.name} is not marked synthetic"
