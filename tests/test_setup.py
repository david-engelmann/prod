import hashlib
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
CANONICAL_PREFIX_SHA256 = "63d2f04005dc8eb111d3efca1be55c0911f9ad46bc7cf3c74d9f9908f4a86f11"


def test_contract_stub_obligations_exist():
    text = (ROOT / "CONTRACT.md").read_text()
    lines = [ln for ln in text.splitlines() if re.match(r"^[A-Z]{2}-S\d\.", ln)]
    assert len(lines) >= 5, f"expected at least 5 stub obligations, found {len(lines)}"
    for ln in lines:
        words = len(ln.split())
        assert words >= 20, f"obligation too thin at {words} words, {ln[:40]}"


def test_agents_canonical_block_present():
    text = (ROOT / "AGENTS.md").read_text()
    assert "repo annex below this line" in text
    prefix = text.split("## Repo annex", 1)[0]
    digest = hashlib.sha256(prefix.encode()).hexdigest()
    assert digest == CANONICAL_PREFIX_SHA256, "canonical block above the annex line has drifted"
    annex = text.split("## Repo annex", 1)[1]
    assert "CONTRACT.md" in annex


def test_readme_status_reports_rounds():
    text = (ROOT / "README.md").read_text()
    assert "0 of 100" in text


def test_no_em_or_en_dash_in_docs():
    bad = []
    for p in ROOT.rglob("*.md"):
        if any(part.startswith(".") for part in p.relative_to(ROOT).parts):
            continue
        t = p.read_text()
        if "\u2014" in t or "\u2013" in t:
            bad.append(str(p.relative_to(ROOT)))
    assert not bad, f"em or en dash found in {bad}"
