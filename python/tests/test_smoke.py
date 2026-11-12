"""Offline checks of the installed package and its `parsec-edgar` command. No SEC requests."""

import datetime
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


def test_extract_sections_of_a_10ksb():
    # Form 10-KSB numbers its Items differently from Form 10-K: Item 7 is the
    # financial statements, and Item 8A controls and procedures.
    text = (
        f"Item 6. Management's Discussion and Analysis or Plan of Operation\n{BODY}\n"
        f"Item 7. Financial Statements\n{BODY}\nItem 8A. Controls and Procedures\n{BODY}\n"
    )
    sections = pe.extract_sections(text, "10KSB")
    assert [(s["id"], s["title"]) for s in sections] == [
        ("item_6", "Management's Discussion and Analysis or Plan of Operation"),
        ("item_7", "Financial Statements"),
        ("item_8a", "Controls and Procedures"),
    ]


def test_extract_sections_titles_by_filing_date():
    # A bare "Item 6." in a 10-K: "Selected Financial Data" for fiscal years
    # ending before August 9, 2021, "[Reserved]" since, and the current title
    # without a date.
    text = f"Item 5. Market\n{BODY}\nItem 6.\n{BODY}\nItem 7. Management's Discussion\n{BODY}\n"

    def title(**dates):
        return pe.extract_sections(text, "10-K", **dates)[1]["title"]

    assert title() == "[Reserved]"
    assert title(filing_date="2019-03-01") == "Selected Financial Data"
    assert title(filing_date=datetime.date(2019, 3, 1)) == "Selected Financial Data"
    assert title(filing_date=datetime.date(2022, 3, 1)) == "[Reserved]"
    assert title(filing_date=" 2022-03-01 ") == "[Reserved]"
    # A 10-K/A for 2019 filed in 2022.
    assert title(filing_date="2022-03-01", report_date="2019-12-31") == "Selected Financial Data"
    assert title(filing_date="2022-03-01", report_date=datetime.date(2021, 12, 31)) == "[Reserved]"
    # The library checks the strings, the same on every Python version.
    for bad in ["2019-02-30", "2019-3-1", "20190301", "2019-W09-5", "03/01/2019", "2019-03-01T00:00:00", ""]:
        with pytest.raises(ValueError, match="filing_date must be a day written YYYY-MM-DD"):
            pe.extract_sections(text, "10-K", filing_date=bad)
        with pytest.raises(ValueError, match="report_date must be"):
            pe.extract_sections(text, "10-K", filing_date="2022-03-01", report_date=bad)
    with pytest.raises(ValueError, match="needs a filing_date"):
        pe.extract_sections(text, "10-K", report_date="2019-12-31")
    with pytest.raises(TypeError):
        pe.extract_sections(text, "10-K", filing_date=20190301)


def test_client_sections_checks_the_filing_dates():
    # A filing dict with a date that is not a day raises ValueError before
    # anything is downloaded.
    client = pe.Client("Example Corp admin@example.com")
    filing = {"cik": 1, "company": "A", "accession_number": "0000000001-24-000001", "form": "10-K"}
    with pytest.raises(ValueError, match="filing_date must be"):
        client.sections({**filing, "filing_date": "03/01/2024"})
    with pytest.raises(ValueError, match="report_date must be"):
        client.sections({**filing, "filing_date": "2024-03-01", "report_date": "2023-02-30"})


def test_extract_sections_names_the_forms_with_items():
    with pytest.raises(ValueError, match="10-K, 10-Q, 8-K, 10-KSB or 10-QSB.*not S-1"):
        pe.extract_sections("Item 1. Business\n", "S-1")
    assert "10-KSB" in pe.Client.sections.__doc__


def test_html_to_text():
    html = "<table><tr><td>Net sales</td><td>$</td><td>391,035</td></tr></table><p>Second <b>line</b>.</p>"
    assert pe.html_to_text(html) == "Net sales\t$391,035\nSecond line.\n"


def test_html_to_text_and_tables():
    html = (
        "<p>Résultats</p><table><tr><th></th><th colspan=2>2024</th></tr>"
        "<tr><td>Net sales</td><td>$</td><td>391,035</td></tr></table><p>After the table.</p>"
    )
    text, tables = pe.html_to_text_and_tables(html)
    assert text == pe.html_to_text(html)
    [t] = tables
    assert t["rows"] == [["2024"], ["Net sales", "$391,035"]]
    assert t["header_rows"] == [0]
    # "2024" covers the "$" and number columns; "$391,035" sits in the number's column.
    assert t["columns"] == [[1], [0, 2]]
    assert t["spans"] == [[2], [1, 1]]
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
