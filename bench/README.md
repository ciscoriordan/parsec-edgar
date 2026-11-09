# Section-splitting benchmark

This benchmark measures how well `extract_sections` finds the Items of 10-K, 10-Q and 8-K filings, and how long text conversion and splitting take. It compares the output with hand labels for 104 filings filed from 1997 to 2026. The same labels also score [edgar-crawler](https://github.com/nlpaueb/edgar-crawler) and [edgartools](https://github.com/dgunning/edgartools).

## Running it

```sh
export PARSEC_EDGAR_USER_AGENT="Your Name you@example.com"
cargo run --release -p parsec-edgar-bench -- score                           # development set
cargo run --release -p parsec-edgar-bench -- --labels bench/holdout score
cargo run --release -p parsec-edgar-bench -- --labels bench/blind score
```

The first run downloads the labeled documents into the cache, at 3 requests per second. The cache is `~/.cache/parsec-edgar`, or `PARSEC_EDGAR_CACHE` when set. Later runs read only the cache.

`score` takes these options:
- `--filter TEXT` keeps only filings whose accession number, company or form contains TEXT.
- `--repeat N` times each filing N times and keeps the fastest run.
- `--json FILE` writes the per-filing results.
- `--no-misses` leaves out the list of misses.

The report has one row per group: 10-K by era of the filing date, 10-Q, 8-K, all 10-Ks, and all filings. It then has one row per tag. After that it lists every filing with a miss, Item by Item, with the labeled and predicted line numbers and character counts.

## What is measured

- **Item start.** A predicted Item is a hit when it starts on the labeled heading line.
  - Precision is hits divided by predicted Items, recall is hits divided by labeled Items, and F1 is their harmonic mean.
  - A predicted Item that is not labeled counts against precision, for example an Item that a filing lists only in its table of contents.
- **Span accuracy.** For each filing: the characters assigned to the right Item, divided by the characters inside any labeled or predicted Item. The table shows the mean over filings. This catches Items that start right but end wrong, for example a last Item that runs on into an attached annual report.
- **Time.** All times are from a release build.
  - Parse is `document_to_text` and Split is `extract_sections`. Both columns show the median per filing in milliseconds.
  - Total is the sum over the group's filings.
  - The document is read from the cache before timing starts.

## Label sets

| Directory | Filings | Labeled Items | Role |
|---|--:|--:|---|
| `bench/labels` | 65 | 905 | Development set. The extractor was tuned on it. |
| `bench/holdout` | 20 | 276 | Labeled after the first round of changes and scored with them frozen (F1 0.955, span 0.875). Its misses were then fixed, so it is no longer held out. |
| `bench/blind` | 19 | 180 | Labeled and committed before any scoring, then scored once with the extractor frozen (F1 0.989, span 0.997). Its two misses were then fixed, so it is no longer blind either. |

The development set has these filings:
- 10-K: nine or ten in each of five eras from 1997 to 2026, 46 in all.
- 10-Q: ten, from 1999 to 2025.
- 8-K: nine.

Tags mark the hard cases:

| Tag | Filings | What it marks |
|---|--:|---|
| `plain-text` | 10 | Pre-2001 filings with no HTML document. |
| `exhibit-13` | 7 | The 10-K answers most Items by referring to an attached annual report. |
| `annual-report-attached` | 5 | A full annual report follows the last Item. |
| `cross-reference` | 4 | Items appear only in a cross-reference index, with no body headings. The labels then list the Items as absent. |
| `running-headers` | 5 | Every page repeats the Item heading as a header. |
| `bank` | 12 | Bank filings. |
| `insurer` | 6 | Insurer filings. |
| `small-cap` | 12 | Small-company filings. |

The filings come from many filing agents: self-filed, DFIN (Donnelley), Bowne, Toppan Merrill, Workiva, EdgarAgents, M2 Compliance and others. They include a 10-K405 and Items such as 1C, 9C and 16.

## Labels

There is one JSON file per filing, named by accession number. It contains:
- the filing's metadata;
- the URL of the document that was read (the primary document, or the complete submission for filings without one);
- `tags`;
- `items`: one entry per Item that has a heading in the body, in document order.

Each Item entry has these fields:
- `text`: the heading line as printed.
- `occurrence`: the line's index among lines with the same normalized text.
- `next`: the start of the following line.
- `incorporated`: optional. The Item only refers to another document or says "not applicable".
- `uncertain`: optional. The label itself is uncertain.

At the filing level:
- `absent` lists Items the form defines but the filing has no heading for.
- `end` marks where the last Item ends: the signatures, an exhibit index, or an attached report.

An Item runs from its heading to the next labeled heading, or to `end`. Labels are found again by their text, not by line number, so they survive changes to the text conversion.

The labels were made without the extractor's output:
- `items ACCESSION` lists every line that looks like an Item, Part, signatures or exhibit-index heading, using a pattern of its own. It shows each line with the text that follows it, including contents entries and page headers.
- `lines ACCESSION [--from N --to M --grep RE]` prints numbered lines.
- `set ACCESSION 1:120 1A:340 7:900 ... --end 2400` writes the chosen lines to the label file and prints them back for checking.
- `add TICKER ACCESSION --tag ...` creates a label file for a new filing.

## Results

The "Before" column is the library at the initial commit. "After" is this branch. Both are from release builds, with each filing parsed 5 times and the fastest run kept.

| Label set | Before: P / R / F1 | Before: span | After: P / R / F1 | After: span |
|---|---|--:|---|--:|
| Development (65) | 0.946 / 0.885 / 0.914 | 0.807 | 1.000 / 1.000 / 1.000 | 0.990 |
| Held out (20) | 0.957 / 0.880 / 0.917 | 0.753 | 1.000 / 1.000 / 1.000 | 1.000 |
| Blind (19) | 0.799 / 0.839 / 0.818 | 0.686 | 1.000 / 1.000 / 1.000 | 1.000 |
| All 104 | 0.926 / 0.878 / 0.902 | 0.775 | 1.000 / 1.000 / 1.000 | 0.993 |

Development set, by group:

| Group | Before: F1 | Before: span | After: F1 | After: span |
|---|--:|--:|--:|--:|
| 10-K 1997 to 2001 | 0.683 | 0.551 | 1.000 | 0.998 |
| 10-K 2002 to 2008 | 0.959 | 0.878 | 1.000 | 0.968 |
| 10-K 2009 to 2015 | 1.000 | 0.898 | 1.000 | 1.000 |
| 10-K 2016 to 2020 | 0.955 | 0.865 | 1.000 | 0.962 |
| 10-K 2021 to 2026 | 0.944 | 0.906 | 1.000 | 1.000 |
| 10-Q | 0.767 | 0.616 | 1.000 | 1.000 |
| 8-K | 1.000 | 0.950 | 1.000 | 1.000 |
| `plain-text` | 0.657 | 0.502 | 1.000 | 0.998 |
| `annual-report-attached` | 0.916 | 0.107 | 1.000 | 0.996 |
| `running-headers` | 0.751 | 0.995 | 1.000 | 1.000 |

The changes, in order:
1. **Plain-text filings.** Old filings whose text contains SGML `<TABLE>` tags are no longer rendered as HTML. Rendering them as HTML merged their lines into a few long ones.
2. **Contents entries.** A table-of-contents entry is dropped when the same Item and title come back later at a longer stretch of text. Entries next to a dropped one go with it.
3. **Running headers.** A bare running header ("Item 7") just before the Item's titled heading is dropped. A sub-item reference such as "Item 15(a)" with no title is not a heading.
4. **End of the last Item.** The last Item ends:
   - at the signatures;
   - at the signature block's opening sentence, when there is no signatures heading;
   - at an attached document with its own page-numbered table of contents.
5. **Page headers and footers.** Lines that carry a page number and repeat page after page are removed from the text.
6. **10-Q parts.** PART lines decide whether a 10-Q Item belongs to Part I or Part II. Items printed out of order are kept.
7. **10-K Items the form does not define.** A titled heading such as "Item 4A. Executive Officers" ends the Item before it.
8. **Item lists after the signatures.** A list of Item headings printed again after the signatures is ignored.

The last misses on the development set are span-only. In Netflix's 2003 and 2017 10-Ks, the financial statement pages follow the last Item without a table of contents of their own.

### Speed

These are per-filing times on the 104 filings, from the same runs as the results above:

| | Before | After |
|---|--:|--:|
| Parse plus split, mean | 30.7 ms | 30.6 ms |
| Parse plus split, median | 14.6 ms | 14.7 ms |
| Split alone, mean | 0.36 ms | 0.35 ms |
| Split alone, median | 0.20 ms | 0.25 ms |

Splitting stays under 1.5 ms per filing; most of the time is text conversion. The plain-text filings now keep all their lines, which gives the splitter more lines to scan.

## Other tools

`bench/external/run.sh` runs edgar-crawler and edgartools on the same filings and scores their output:

```sh
export PARSEC_EDGAR_USER_AGENT="Your Name you@example.com"
bench/external/run.sh            # all three sets
bench/external/run.sh holdout    # one set
```

It needs git, Python 3.11 for edgar-crawler and Python 3.10 or later for edgartools. Use `PYTHON311` and `PYTHON` to choose the interpreters. edgar-crawler pins numpy 1.24 and pandas 1.5, which have no wheels for later Pythons.

The script does the following:
1. Clones edgar-crawler at a fixed commit (84a8d0c, July 2025) and installs both tools (edgartools 5.62.1), each in its own virtual environment.
2. Exports the labeled filings with `export --full-submission`.
3. Runs each tool and writes its Items as one JSON file per filing.
4. Scores the output with `score-external`.

Clones, environments, exported filings and results stay in `bench/external` and are ignored by git. edgar-crawler is GPL-3.0, so `crawler_run.py` runs its `extract_items.py` as a separate program, and none of its code is in this repository.

- **edgar-crawler** reads the primary document, or the complete submission for filings without one, as its own downloader saves it. It runs with its default settings, including table removal.
- **edgartools** reads the complete submission through `Filing.from_sgml_text`, which is what it downloads for a filing. Its Items come from the report object's public API (`report.items`, then `report[item]`).
  - The network is blocked, so it never reaches the SEC.
  - For a filing without an HTML document (the pre-2001 plain-text filings), edgartools looks up the filing's index page, and the block stops that request. Run with network access on two of these filings (from 1997 and 1999), edgartools finds no HTML document and returns no Items, so the score is the same either way.
  - The one 10-K405 is read as a 10-K, because edgartools' 10-K class accepts only 10-K and 10-K/A.

### Placing another tool's text

`score-external` finds each section's text in our filing text, comparing letters and digits only. A section starts at the line where its opening characters appear and the following text agrees best, which tells a body heading from its table-of-contents entry. Formatting conventions that do not move an Item's start are not counted as errors:
- a Part line or page header before the heading;
- a heading left out of the section text;
- a page footer inside the text;
- a body too short to find, such as "None.", when it equals the labeled Item's body.

`predict DIR` writes this extractor's own sections in the same format. Scoring them through `score-external` reproduces the direct scores: F1 1.000 on all three sets, and span 0.987 against 0.990 on the development set.

### Scores

These are all 104 filings, with the three sets combined:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-Q F1 | 8-K F1 | Time per filing |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar (this branch) | 1.000 | 1.000 | 1.000 | 0.993 | 1.000 | 1.000 | 1.000 | 31 ms mean, 15 ms median |
| parsec-edgar (initial commit) | 0.926 | 0.878 | 0.902 | 0.775 | 0.936 | 0.708 | 1.000 | 31 ms mean, 15 ms median |
| edgar-crawler | 0.990 | 0.989 | 0.990 | 0.914 | 0.997 | 0.948 | 1.000 | 0.43 s mean |
| edgartools | 0.953 | 0.826 | 0.885 | 0.677 | 0.871 | 0.926 | 1.000 | 1.07 s mean, 0.54 s median |

By label set:

| Tool | Development: F1 | Development: span | Held out: F1 | Held out: span | Blind: F1 | Blind: span |
|---|--:|--:|--:|--:|--:|--:|
| parsec-edgar | 1.000 | 0.990 | 1.000 | 1.000 | 1.000 | 1.000 |
| edgar-crawler | 0.994 | 0.907 | 0.987 | 0.928 | 0.972 | 0.925 |
| edgartools | 0.876 | 0.639 | 0.884 | 0.675 | 0.928 | 0.810 |

How to read these numbers:
- **Tuning.** parsec-edgar was tuned on all three sets, and the other tools were not. The fairest comparison with them is the first scoring of the blind set (F1 0.989, span 0.997), or the held-out set as first scored (F1 0.955, span 0.875).
- **edgar-crawler** finds Item starts nearly as well, from a hard-coded list of each form's Items.
  - It loses span accuracy on the last Item, which runs on into attached annual reports (span 0.136 on the `annual-report-attached` filings).
  - Some 10-Q Items start at the table of contents or at a "(continued)" page header.
- **edgartools** returns no Items for filings without an HTML document, so it scores 0.015 F1 on the `plain-text` filings. It also misses Items in many exhibit-13 and small-company 10-Ks.
- **Time.** The Python times include reading the filing from disk, and edgartools' times include parsing the whole submission. The parsec-edgar times start after the cached document has been read.
  - edgar-crawler's time is its whole run, minus its start-up time, divided by the number of filings.
  - edgartools' time is measured per filing.
