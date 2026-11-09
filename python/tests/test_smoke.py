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


def test_client_needs_a_contact_email():
    with pytest.raises(ValueError):
        pe.Client("Example Corp")


def run(*args, env=None):
    return subprocess.run(args, capture_output=True, text=True, env=env, timeout=60)


def test_command_line():
    exe = shutil.which("parsec-edgar")
    assert exe, "pip install should put the parsec-edgar command on PATH"
    out = run(exe, "--version")
    assert out.returncode == 0
    assert out.stdout.strip() == f"parsec-edgar {pe.__version__}"
    assert "sections" in run(exe, "--help").stdout
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
