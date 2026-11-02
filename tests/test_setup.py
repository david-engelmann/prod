import pathlib, re
ROOT = pathlib.Path(__file__).resolve().parents[1]

def test_contract_stub_obligations_exist():
    text = (ROOT / "CONTRACT.md").read_text()
    ids = re.findall(r"^[A-Z]{2}-S\d\.", text, flags=re.M)
    assert len(ids) >= 3, f"expected at least 3 stub obligations, found {len(ids)}"

def test_agents_canonical_block_present():
    text = (ROOT / "AGENTS.md").read_text()
    assert "repo annex below this line" in text
    annex = text.split("## Repo annex", 1)[1]
    assert "CONTRACT.md" in annex

def test_readme_status_reports_rounds():
    text = (ROOT / "README.md").read_text()
    assert "0 of 100" in text

def test_no_em_or_en_dash_in_docs():
    bad = []
    for p in ROOT.rglob("*.md"):
        if ".git" in p.parts:
            continue
        t = p.read_text()
        if "\u2014" in t or "\u2013" in t:
            bad.append(str(p.relative_to(ROOT)))
    assert not bad, f"em or en dash found in {bad}"
