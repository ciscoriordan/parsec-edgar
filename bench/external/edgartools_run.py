"""Run edgartools on exported filings and write its Items for score-external.

Usage: python edgartools_run.py RAW_DIR OUT_DIR

RAW_DIR is the output of `parsec-edgar-bench export --full-submission`. Each
filing is read from its complete SGML submission with Filing.from_sgml_text,
which is what edgartools downloads for a filing, and its Items come from the
report object's public API (`report.items`, then `report[item]`). Network
access is blocked, so nothing reaches the SEC. OUT_DIR gets one
`<accession>.json` per filing mapping our section ids to text, plus
`timing.json` with the seconds each filing took.
"""

import json
import re
import statistics
import socket
import sys
import time
import warnings
from pathlib import Path


def no_network(*args, **kwargs):
    raise OSError("network access is disabled in this benchmark")


socket.socket.connect = no_network
socket.create_connection = no_network
warnings.simplefilter("ignore")

import stamina  # noqa: E402
from edgar import Filing, set_identity  # noqa: E402
from edgar.company_reports import CurrentReport, TenK, TenQ  # noqa: E402

set_identity("Parsec bench@example.com")
# A blocked request fails at once instead of being retried. edgartools makes
# one only for a filing without an HTML document, where it looks for one on
# the filing's index page; with the network on it finds none and returns no
# Items, which is also what a blocked request gives here.
stamina.set_active(False)


def section_id(form: str, name: str) -> str | None:
    """Our id for an edgartools item name, or None for anything else."""
    name = name.strip()
    if form.startswith("10-Q"):
        m = re.match(r"(?i)part\s+(i{1,2})\s*,\s*item\s+(\d+[a-z]?)$", name)
        if m:
            part = 1 if m.group(1).lower() == "i" else 2
            return f"part_{part}_item_{m.group(2).lower()}"
        m = re.match(r"(?i)item\s+(\d+[a-z]?)$", name)
        # edgartools reads an Item without a part as Part I.
        return f"part_1_item_{m.group(1).lower()}" if m else None
    if form.startswith("8-K"):
        m = re.match(r"(?i)item\s+(\d+\.\d+)$", name)
        return f"item_{m.group(1)}" if m else None
    m = re.match(r"(?i)item\s+(\d+[a-z]?)$", name)
    return f"item_{m.group(1).lower()}" if m else None


def report_for(filing, form: str):
    if form.startswith("10-K"):
        # TenK accepts only 10-K and 10-K/A; a 10-K405 is the same report.
        filing.form = "10-K"
        return TenK(filing)
    if form.startswith("10-Q"):
        return TenQ(filing)
    return CurrentReport(filing)


def run_one(path: Path, form: str) -> dict[str, str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    filing = Filing.from_sgml_text(text)
    report = report_for(filing, form)
    out: dict[str, str] = {}
    for name in report.items:
        sid = section_id(form, name)
        if sid is None or sid in out:
            continue
        body = report[name]
        if body and body.strip():
            out[sid] = body
    return out


def main() -> None:
    raw, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((raw / "manifest.json").read_text())
    timing = {}
    for entry in manifest:
        acc = entry["accession"]
        start = time.perf_counter()
        try:
            sections = run_one(raw / entry["full_submission"], entry["form"])
            error = None
        except Exception as e:  # a crash is scored as no output
            sections, error = {}, f"{type(e).__name__}: {e}"
        timing[acc] = {"seconds": time.perf_counter() - start, "error": error}
        (out_dir / f"{acc}.json").write_text(json.dumps(sections, ensure_ascii=False))
        status = error or f"{len(sections)} items"
        print(f"{acc} {entry['form']:<7} {timing[acc]['seconds']:7.2f}s {status}", flush=True)
    (out_dir / "timing.json").write_text(json.dumps(timing, indent=1))
    secs = [t["seconds"] for t in timing.values()]
    errors = sum(t["error"] is not None for t in timing.values())
    print(json.dumps({"filings": len(secs), "errors": errors, "mean_seconds": statistics.mean(secs),
                      "median_seconds": statistics.median(secs), "total_seconds": sum(secs)}))


if __name__ == "__main__":
    main()
