"""Offline checks of the installed package and its `parsec-edgar` command. No SEC requests."""

import os
import shutil
import subprocess
import sys

import pytest

import parsec_edgar as pe

BODY = "We make and sell things in many markets around the world. " * 60


def test_version():
    assert pe.__version__.count(".") == 2


def test_extract_sections():
    text = f"Item 1. Business\n{BODY}\nItem 1A. Risk Factors\n{BODY}\nItem 7. Management's Discussion\n{BODY}\n"
    sections = pe.extract_sections(text, "10-K")
    assert [s["id"] for s in sections] == ["item_1", "item_1a", "item_7"]
    assert sections[1]["text"].startswith("Item 1A. Risk Factors\nWe make")


def test_html_to_text():
    html = "<table><tr><td>Net sales</td><td>$</td><td>391,035</td></tr></table><p>Second <b>line</b>.</p>"
    assert pe.html_to_text(html) == "Net sales\t$391,035\nSecond line.\n"


def test_html_to_text_and_tables():
    html = (
        "<p>Résultats</p><table><tr><th></th><th>2024</th></tr>"
        "<tr><td>Net sales</td><td>$</td><td>391,035</td></tr></table><p>After the table.</p>"
    )
    text, tables = pe.html_to_text_and_tables(html)
    assert text == pe.html_to_text(html)
    [t] = tables
    assert t["rows"] == [["2024"], ["Net sales", "$391,035"]]
    assert t["header_rows"] == [0]
    # Offsets count bytes of the UTF-8 text, like section offsets.
    assert text.encode()[t["start"] : t["end"]].decode() == "2024\nNet sales\t$391,035\n"
    assert pe.html_to_text_and_tables("A plain-text filing.\n") == ("A plain-text filing.\n", [])


def test_client_needs_a_contact_email():
    with pytest.raises(ValueError):
        pe.Client("Example Corp")


def test_client_tickers_max_age():
    ua = "Example Corp admin@example.com"
    for seconds in (0, 3600, 1.5, 86400 * 365, float("inf"), 1e300):
        pe.Client(ua, tickers_max_age=seconds)
    for bad in (-1, float("nan")):
        with pytest.raises(ValueError):
            pe.Client(ua, tickers_max_age=bad)
    with pytest.raises(TypeError):
        pe.Client(ua, tickers_max_age="1 day")


def run(*args, env=None):
    return subprocess.run(args, capture_output=True, text=True, env=env, timeout=60)


def test_command_line():
    exe = shutil.which("parsec-edgar")
    assert exe, "pip install should put the parsec-edgar command on PATH"
    out = run(exe, "--version")
    assert out.returncode == 0
    assert out.stdout.strip() == f"parsec-edgar {pe.__version__}"
    help_text = run(exe, "--help").stdout
    assert "sections" in help_text and "tables" in help_text
    assert run(exe, "--no-such-option").returncode == 2


def test_python_dash_m():
    out = run(sys.executable, "-m", "parsec_edgar", "--version")
    assert out.returncode == 0
    assert out.stdout.strip() == f"parsec-edgar {pe.__version__}"


def test_missing_user_agent_is_a_clear_error():
    env = {k: v for k, v in os.environ.items() if k not in ("PARSEC_EDGAR_USER_AGENT", "EDGAR_IDENTITY")}
    out = run(sys.executable, "-m", "parsec_edgar", "company", "AAPL", env=env)
    assert out.returncode == 1
    assert "PARSEC_EDGAR_USER_AGENT" in out.stderr
