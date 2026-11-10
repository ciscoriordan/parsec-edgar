#!/usr/bin/env bash
# Score edgar-crawler and edgartools on the benchmark's label sets.
#
#   PARSEC_EDGAR_USER_AGENT="Name email@example.com" bench/external/run.sh [dev] [holdout] [blind] [fresh] [fresh2]
#
# Needs git, Python 3.11 for edgar-crawler (it pins numpy 1.24 and pandas 1.5,
# which have no wheels for later Pythons) and Python 3.10 or later for
# edgartools. PYTHON311 and PYTHON choose the interpreters. Clones, virtual
# environments, exported filings and results stay in this directory and are
# ignored by git. Filings come through the benchmark's cache at --rps 3.
set -euo pipefail
: "${PARSEC_EDGAR_USER_AGENT:?set PARSEC_EDGAR_USER_AGENT to your name and contact email}"

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
crawler_commit=84a8d0c5dd7dd6769526e5ccec534c4e0880d56f
edgartools_version=5.62.1

sets=("$@")
if [ ${#sets[@]} -eq 0 ]; then
  sets=(dev holdout blind)
fi

cargo build --quiet --release -p parsec-edgar-bench --manifest-path "$root/Cargo.toml"
bench="$root/target/release/parsec-edgar-bench"

if [ ! -d "$here/edgar-crawler" ]; then
  git clone --quiet https://github.com/nlpaueb/edgar-crawler.git "$here/edgar-crawler"
fi
git -C "$here/edgar-crawler" checkout --quiet "$crawler_commit"

if [ ! -x "$here/.venv-crawler/bin/python" ]; then
  "${PYTHON311:-python3.11}" -m venv "$here/.venv-crawler"
  "$here/.venv-crawler/bin/python" -m pip install --quiet -r "$here/edgar-crawler/requirements.txt"
fi
if [ ! -x "$here/.venv-edgartools/bin/python" ]; then
  "${PYTHON:-python3}" -m venv "$here/.venv-edgartools"
  "$here/.venv-edgartools/bin/python" -m pip install --quiet "edgartools==$edgartools_version"
fi

for set in "${sets[@]}"; do
  case "$set" in
    dev) labels="$root/bench/labels" ;;
    holdout | blind | fresh | fresh2) labels="$root/bench/$set" ;;
    *) echo "unknown label set: $set" >&2; exit 2 ;;
  esac
  raw="$here/raw/$set"
  "$bench" --rps 3 --labels "$labels" export --full-submission "$raw"

  echo "== edgar-crawler, $set"
  "$here/.venv-crawler/bin/python" -I "$here/crawler_run.py" "$here/edgar-crawler" \
    "$here/.venv-crawler/bin/python" "$raw" "$here/out/edgar-crawler/$set" "$set"
  echo "== edgartools, $set"
  "$here/.venv-edgartools/bin/python" -I "$here/edgartools_run.py" "$raw" "$here/out/edgartools/$set"

  for tool in edgar-crawler edgartools; do
    "$bench" --rps 3 --labels "$labels" score-external "$here/out/$tool/$set" --name "$tool" \
      --json "$here/out/$tool/$set.scores.json" > "$here/out/$tool/$set.txt"
    echo "== $tool, $set: $here/out/$tool/$set.txt"
    awk '/^$/ { if (++blank == 2) exit } { print }' "$here/out/$tool/$set.txt"
  done
done
