"""Run edgar-crawler on exported filings and write its Items for score-external.

Usage: python crawler_run.py CRAWLER_DIR PYTHON RAW_DIR OUT_DIR NAME

CRAWLER_DIR is a clone of https://github.com/nlpaueb/edgar-crawler and PYTHON
the interpreter of a virtual environment with its requirements. edgar-crawler
is GPL-3.0, so this script only prepares its input and runs its own
extract_items.py as a separate program; nothing of it is copied here.

The filings come from `parsec-edgar-bench export`: the primary document, or
the complete submission for filings without one, which is what edgar-crawler's
downloader saves. OUT_DIR gets one `<accession>.json` per filing mapping our
section ids to text, plus `timing.json`.
"""

import csv
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

COLUMNS = [
    "CIK", "Company", "Type", "Date", "complete_text_file_link", "html_index",
    "Filing Date", "Period of Report", "SIC", "htm_file_link", "State of Inc",
    "State location", "Fiscal Year End", "filename",
]
ITEM_KEY = re.compile(r"^(part_[12]_)?item_[0-9a-z.]+$", re.IGNORECASE)


def base_form(form: str) -> str:
    for f in ("10-K", "10-Q", "8-K"):
        if form.startswith(f):
            return f
    return form


def write_inputs(crawler: Path, raw: Path, name: str, entries: list[dict]) -> dict:
    datasets = crawler / "datasets"
    raw_folder, out_folder = f"bench_{name}_raw", f"bench_{name}_out"
    for folder in (raw_folder, out_folder):
        shutil.rmtree(datasets / folder, ignore_errors=True)
    rows = []
    for e in entries:
        form = base_form(e["form"])
        ext = e["file"].rsplit(".", 1)[1]
        filename = f"{e['accession']}.{ext}"
        dest = datasets / raw_folder / form / filename
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(raw / e["file"], dest)
        row = dict.fromkeys(COLUMNS, "")
        row.update({
            "CIK": str(e["cik"]), "Company": e["company"], "Type": form,
            "Date": e["filing_date"], "Filing Date": e["filing_date"],
            "htm_file_link": e["url"], "filename": filename,
        })
        rows.append(row)
    with open(datasets / f"bench_{name}.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=COLUMNS)
        w.writeheader()
        w.writerows(rows)
    return {
        "raw_filings_folder": raw_folder,
        "extracted_filings_folder": out_folder,
        "filings_metadata_file": f"bench_{name}.csv",
        "filing_types": ["10-K", "10-Q", "8-K"],
        "include_signature": False,
        "items_to_extract": [],
        "remove_tables": True,
        "skip_extracted_filings": False,
    }


def run_crawler(crawler: Path, python: str, work: Path, config: dict) -> float:
    work.mkdir(parents=True, exist_ok=True)
    # extract_items.py reads config.json from the working directory.
    (work / "config.json").write_text(json.dumps({"extract_items": config}, indent=1))
    start = time.perf_counter()
    subprocess.run([python, str(crawler / "extract_items.py")], cwd=work, check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return time.perf_counter() - start


def main() -> None:
    crawler, python, raw, out_dir, name = (Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3]),
                                           Path(sys.argv[4]), sys.argv[5])
    out_dir.mkdir(parents=True, exist_ok=True)
    entries = json.loads((raw / "manifest.json").read_text())
    work = out_dir / "work"
    # Startup alone (imports, reading the metadata file), to subtract from the
    # run. The first start after installing compiles the packages, so the
    # empty run goes once before and once after, and the second one counts.
    empty = write_inputs(crawler, raw, f"{name}_empty", [])
    run_crawler(crawler, python, work, empty)
    config = write_inputs(crawler, raw, name, entries)
    total = run_crawler(crawler, python, work, config)
    startup = run_crawler(crawler, python, work, empty)
    extracted = crawler / "datasets" / config["extracted_filings_folder"]
    found = 0
    for e in entries:
        path = extracted / base_form(e["form"]) / f"{e['accession']}.json"
        sections = {}
        if path.exists():
            found += 1
            data = json.loads(path.read_text())
            sections = {k.lower(): v for k, v in data.items()
                        if ITEM_KEY.match(k) and isinstance(v, str) and v.strip()}
        (out_dir / f"{e['accession']}.json").write_text(json.dumps(sections, ensure_ascii=False))
    timing = {"filings": len(entries), "with_output": found, "startup_seconds": startup,
              "total_seconds": total,
              "seconds_per_filing": (total - startup) / max(len(entries), 1)}
    (out_dir / "timing.json").write_text(json.dumps(timing, indent=1))
    print(json.dumps(timing))


if __name__ == "__main__":
    main()
