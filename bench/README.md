# Section-splitting benchmark

This benchmark measures how well `extract_sections` finds the Items of 10-K, 10-Q and 8-K filings, and how long text conversion and splitting take. It compares the output with hand labels for 129 filings filed from 1997 to 2026. The same labels also score [edgar-crawler](https://github.com/nlpaueb/edgar-crawler) and [edgartools](https://github.com/dgunning/edgartools).

## Accuracy in brief

The extractor was tuned on three of the four label sets, so its scores on them are in-sample. The numbers that show how it does on filings it has not seen are each set's first scoring, made before any of that set's misses were fixed. Only the fresh set is still untouched.

| Label set | Filings | First scoring | Item-start F1 | Span accuracy |
|---|--:|---|--:|--:|
| `bench/fresh` | 25 | Labels committed first, then scored once. Not tuned on. | 0.994 | 0.935 |
| `bench/blind` | 19 | Labels committed first, then scored once. Its two misses were fixed afterward. | 0.989 | 0.997 |
| `bench/holdout` | 20 | Scored after the first round of changes. Its misses were fixed afterward. | 0.955 | 0.875 |
| The 104 tuned filings, in-sample | 104 | After tuning on them | 1.000 | 0.993 |

On the fresh set, precision is 1.000 and recall 0.989: 345 of the 349 labeled Item starts are found, and no Item is predicted where none is labeled. On the same 25 filings, edgar-crawler scores F1 0.961 with span 0.843, edgartools 0.900 with span 0.705, and this library at its initial commit 0.927 with span 0.756.

Span accuracy is computed per filing: the characters assigned to the right Item, divided by the characters inside any labeled or predicted Item. The tables show the mean over filings. It is not the share of labeled text that was found. A filing whose last Item runs on through pages that the labels leave out of every Item scores well under 1, even when every Item starts on the right line.

## Running it

```sh
export PARSEC_EDGAR_USER_AGENT="Your Name you@example.com"
cargo run --release -p parsec-edgar-bench -- score                           # development set
cargo run --release -p parsec-edgar-bench -- --labels bench/holdout score
cargo run --release -p parsec-edgar-bench -- --labels bench/blind score
cargo run --release -p parsec-edgar-bench -- --labels bench/fresh score
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
- **Errors.** A filing whose document cannot be read, or whose labels no longer resolve in the text, stays in every row: all its labeled Items count as missed and its span accuracy as 0. The report lists these filings under "Errors".
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
| `bench/fresh` | 25 | 349 | Labeled and committed after all the changes below, then scored once (F1 0.994, span 0.935). The extractor has not been changed since, and its misses are left in place. |

The development set has these filings:
- 10-K: nine or ten in each of five eras from 1997 to 2026, 46 in all.
- 10-Q: ten, from 1999 to 2025.
- 8-K: nine.

The fresh set has 16 10-Ks from 1998 to 2025, five 10-Qs from 1999 to 2023 and four 8-Ks from 2007 to 2025. None of its companies appear in another set. The filings were picked from EDGAR's quarterly indexes and from company filing lists to cover eras, forms and filing agents, and labeled with the commands below without running the extractor.

Tags mark the hard cases. The counts are for the development set; the fresh set has 4 `plain-text`, 3 `exhibit-13`, 4 `bank`, 2 `insurer` and 7 `small-cap` filings.

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
- `end`: optional. Where the Item ends, when that is not the next labeled heading. It is used for text that belongs to no Item, such as an executive officers section numbered "Item 4A", or financial statements printed with no Item heading.

At the filing level:
- `absent` lists Items the form defines but the filing has no heading for.
- `end` marks where the last Item ends: the signatures, an exhibit index, or an attached report.

An Item runs from its heading to the next labeled heading, or to `end`. Labels are found again by their text, not by line number, so they survive changes to the text conversion.

The labels were made without the extractor's output:
- `items ACCESSION` lists every line that looks like an Item, Part, signatures or exhibit-index heading, using a pattern of its own. It shows each line with the text that follows it, including contents entries and page headers.
- `lines ACCESSION [--from N --to M --grep RE]` prints numbered lines.
- `set ACCESSION 1:120 1A:340 7:900 ... --end 2400` checks that the chosen lines resolve, writes them to the label file and prints them back for checking. A file is written only when all its labels resolve.
- `add TICKER ACCESSION --tag ...` creates a label file for a new filing. It reads the complete submission only when the filing has no primary document (the SEC answers 404); any other download failure stops it.

## Results

The "Before" column is the library at the initial commit. "After" is this branch. Both are from release builds, with each filing parsed 5 times and the fastest run kept. Every "After" score of 1.000 is in-sample: those sets were tuned on.

| Label set | Before: P / R / F1 | Before: span | After: P / R / F1 | After: span |
|---|---|--:|---|--:|
| Fresh (25), not tuned on | 0.949 / 0.905 / 0.927 | 0.756 | 1.000 / 0.989 / 0.994 | 0.935 |
| Development (65), in-sample | 0.946 / 0.885 / 0.914 | 0.807 | 1.000 / 1.000 / 1.000 | 0.990 |
| Held out (20), in-sample | 0.957 / 0.880 / 0.917 | 0.753 | 1.000 / 1.000 / 1.000 | 1.000 |
| Blind (19), in-sample | 0.799 / 0.839 / 0.818 | 0.686 | 1.000 / 1.000 / 1.000 | 1.000 |
| The 104 tuned filings | 0.926 / 0.878 / 0.902 | 0.775 | 1.000 / 1.000 / 1.000 | 0.993 |

### The fresh set

| Group | Filings | Before: F1 | Before: span | After: F1 | After: span |
|---|--:|--:|--:|--:|--:|
| 10-K 1997 to 2001 | 3 | 0.795 | 0.487 | 1.000 | 0.820 |
| 10-K 2002 to 2008 | 3 | 0.991 | 0.986 | 0.991 | 0.986 |
| 10-K 2009 to 2015 | 3 | 1.000 | 1.000 | 1.000 | 1.000 |
| 10-K 2016 to 2020 | 3 | 1.000 | 0.828 | 1.000 | 0.828 |
| 10-K 2021 to 2026 | 4 | 1.000 | 0.997 | 1.000 | 0.997 |
| 10-Q | 5 | 0.541 | 0.203 | 0.958 | 0.897 |
| 8-K | 4 | 1.000 | 1.000 | 1.000 | 1.000 |

The library before the review fixes (see change 9 below) scores exactly the same on the fresh set. Seven filings have misses:
- **Capital One 10-Q 2020.** Item 2 is printed before Item 1, under a single PART I line. Item 1 has no PART line of its own, so it is not added as an out-of-order Item: it is missed, and Item 2 runs over it (span 0.530).
- **Samsara Luggage 10-Q 2023.** "Item 3. - Quantitative and Qualitative Disclosures about Market Risk" and "Item 4. - Controls and Procedures" have two separators. The title then starts with a dash, the heading test rejects both, and their text goes into Item 2.
- **McDonald's 10-K 2003.** Item 5 is printed as "Item 5A" and "Item 5B". Neither is a 10-K Item, so Item 5 is missed, and its text belongs to no Item.
- **Kroger 10-K 2019.** There is no Item 8 heading. The labels leave the financial statements out of every Item, and the extractor keeps them in Item 7A (span 0.483).
- **Allen Organ 10-K 1998.** The financial statements are printed between Item 9 and Part III with no heading. The labels leave them out of every Item, and the extractor keeps them in Item 9 (span 0.461).
- **Lockheed Martin 10-K 2006.** "ITEM 4(a). EXECUTIVE OFFICERS OF THE REGISTRANT" is read as a sub-item of Item 4, so Item 4 runs over the executive officers section (span 0.983).
- **Patriot National Bancorp 10-K 2025.** Item 16 runs on through an exhibit index printed before the signatures (span 0.988).

edgar-crawler and edgartools also keep the Kroger and Allen Organ statements in the Item before them.

### Development set, by group

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

The last misses on the development set are span-only. In Netflix's 2003 and 2017 10-Ks, the financial statement pages follow the last Item without a table of contents of their own.

### The changes

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
9. **Review fixes.** A code review found layouts where changes 2, 3 and 8 lose Items that the initial commit found, and two layouts that change 6 gets wrong. Unit tests written for layouts near each rule found two more cases. Each case now has a unit test that fails without its fix:
   - A "Signatures" line inside the table of contents no longer makes the body's headings look like a list printed after the signatures. Only a signatures line at or after the first body heading counts, so an empty "Item 6. [Reserved]" and a short last Item are kept.
   - Contents entries spread to their neighbors, but not onto the later copy of a heading that appears twice, so a short body Item ("Not applicable.", "Item 6. [Reserved]") just before a second contents list is kept.
   - A bare heading ("ITEM 1." with "BUSINESS" on the next line) is kept when its next line is the title of the heading that follows it, or the Item's standard title. Only bare running headers are dropped.
   - The search for a PART line above a 10-Q heading stops at another Item's heading line, including a contents row such as "PART II - Item 6. Exhibits", so a "PART II" in the contents no longer moves the body's first Items to Part II.
   - Out-of-order 10-Q Items are added only before the signatures.
   - A 10-Q Item with no PART line above it takes its part from its title, when the title is the standard title of that Item in only one part. A lone "Item 1. Legal Proceedings" is then Part II.
   - A letter joins the Item number when spaces alone separate them ("Item 9 A.") or when a separator with no space after it does ("Item 1.A."), so "ITEM 1. A. GENERAL" stays Item 1.

The tests in `crates/parsec-edgar/src/sections.rs` also cover layouts where each special rule must not fire: a short attached document, a signature sentence without "duly caused", sub-item references, undefined Items mentioned in the text, short Part-marked lines and page lines that only look like headers or footers.

### Filings behind each rule

Each rule was turned off alone, and the table counts the filings whose score changes. A rule with a count of 0 or 1 was written for one layout; it rests on its unit tests more than on the benchmark.

| Rule | Tuned filings (of 104) | Fresh filings (of 25) | Tuned filings that change |
|---|--:|--:|---|
| Plain-text filings kept out of the HTML renderer | 13 | 3 | Plain-text filings from 1997 to 2009, from 9 companies |
| Contents entries dropped | 4 | 4 | 10-Qs: JPMorgan 2010, Netflix 2012, Bank of America 2019, Wells Fargo 2012 |
| Contents entries spread to their neighbors | 3 | 3 | The same 10-Qs except Bank of America |
| Bare running headers dropped | 3 | 0 | Microsoft 10-Ks 2006, 2019 and 2025 |
| Sub-item references are not headings | 1 | 0 | Coca-Cola 10-K 1999 |
| Wider signatures heading: "S I G N A T U R E S", "Signature(s)", a trailing colon | 5, span only | 0 | Espey 10-Ks 1998, 2004, 2009 and 2018, Apple 8-K 2008 |
| Signature sentence ends the last Item | 1, span only | 0 | Cisco 10-Q 2016 |
| Attached document ends the last Item | 6, span only | 0 | Five JPMorgan 10-Ks, Exxon Mobil 2005 |
| Page headers and footers removed | 0 | 1, span change under 0.001 | None; it keeps the text clean |
| PART lines set the 10-Q part | 1 | 0 | Bank of America 10-Q 2019 |
| Out-of-order 10-Q Items kept | 1 | 0 | Bank of America 10-Q 2019 |
| Undefined titled Items end the Item before | 2, span only | 0 | Walmart 2000, Ford 2010 |
| Item lists after the signatures ignored | 0 alone, 1 together with title-based parts | 0 | HP 10-Q 2004 |
| Tie-break: the earlier Item wins | 0 alone, 2 together with title-based parts | 0 | IBM 10-Q 2000, Coca-Cola 10-Q 2001 |
| 10-Q part from the Item's title | 0 alone, 3 together with one of the two rules above | 0 | HP 10-Q 2004, IBM 10-Q 2000, Coca-Cola 10-Q 2001 |
| Out-of-order Items only before the signatures | 0 | 0 | None; it guards a layout found in review |
| Letter joined to the Item number ("Item 9 A.", "Item 1.A.") | 1 | 0 | Berkshire Hathaway 10-K 2004 |

### Speed

These are per-filing times on the 104 tuned filings, from the same runs as the results above:

| | Before | After |
|---|--:|--:|
| Parse plus split, mean | 30.7 ms | 30.6 ms |
| Parse plus split, median | 14.6 ms | 14.7 ms |
| Split alone, mean | 0.36 ms | 0.35 ms |
| Split alone, median | 0.20 ms | 0.25 ms |

Splitting stays under 1.5 ms per filing; most of the time is text conversion. The plain-text filings now keep all their lines, which gives the splitter more lines to scan. Timed again after the review fixes, back to back with the initial commit on a busier machine, both take about 32 ms mean and 15 to 16 ms median per filing, and split alone 0.38 ms mean, so the fixes did not change the speed.

## Other tools

`bench/external/run.sh` runs edgar-crawler and edgartools on the same filings and scores their output:

```sh
export PARSEC_EDGAR_USER_AGENT="Your Name you@example.com"
bench/external/run.sh            # development, held-out and blind sets
bench/external/run.sh fresh      # one set: dev, holdout, blind or fresh
```

It needs git, Python 3.11 for edgar-crawler and Python 3.10 or later for edgartools. Use `PYTHON311` and `PYTHON` to choose the interpreters. edgar-crawler pins numpy 1.24 and pandas 1.5, which have no wheels for later Pythons. The script stops when `PARSEC_EDGAR_USER_AGENT` is not set, and edgartools is given the same identity.

The script does the following:
1. Clones edgar-crawler at a fixed commit (84a8d0c, July 2025) and installs both tools (edgartools 5.62.1), each in its own virtual environment.
2. Exports the labeled filings with `export --full-submission`.
3. Runs each tool and writes its Items as one JSON file per filing.
4. Scores the output with `score-external`.

Clones, environments, exported filings and results stay in `bench/external` and are ignored by git. edgar-crawler is GPL-3.0, so `crawler_run.py` runs its `extract_items.py` as a separate program, and none of its code is in this repository.

- **edgar-crawler** reads the primary document, or the complete submission for filings without one, as its own downloader saves it. It runs with its default settings, including table removal.
- **edgartools** reads the complete submission through `Filing.from_sgml_text`, which is what it downloads for a filing. Its Items come from the report object's public API (`report.items`, then `report[item]`).
  - The network is blocked, so it never reaches the SEC.
  - For some filings, edgartools looks up the filing's index page, and the block stops that request. This happens on 8 development-set filings, all from 1997 to 2000: seven plain-text filings and Walmart's 2000 10-K, which is HTML. It also happens on the 4 plain-text filings of the fresh set. Run with network access on all 12, edgartools makes one or two requests per filing and returns no Items, so the score is the same either way.
  - The one 10-K405 is read as a 10-K, because edgartools' 10-K class accepts only 10-K and 10-K/A.

### Placing another tool's text

`score-external` finds each section's text in our filing text, comparing letters and digits only. A section starts at the line where its opening characters appear and the following text agrees best, which tells a body heading from its table-of-contents entry. Formatting conventions that do not move an Item's start are not counted as errors:
- a Part line or page header before the heading;
- a heading left out of the section text;
- a page footer inside the text;
- a body too short to find, such as "None.", when it equals the labeled Item's body.

`predict DIR` writes this extractor's own sections in the same format. Scoring them through `score-external` reproduces the direct scores: F1 1.000 on the three tuned sets, and span 0.987 against 0.990 on the development set.

### Scores

The fresh set is the fair comparison, since no tool was tuned on it:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-Q F1 | 8-K F1 | Time per filing |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar (this branch) | 1.000 | 0.989 | 0.994 | 0.935 | 0.998 | 0.958 | 1.000 | 22 ms mean, 13 ms median |
| parsec-edgar (initial commit) | 0.949 | 0.905 | 0.927 | 0.756 | 0.973 | 0.541 | 1.000 | |
| edgar-crawler | 0.963 | 0.960 | 0.961 | 0.843 | 0.961 | 0.959 | 1.000 | 0.38 s mean |
| edgartools | 0.977 | 0.834 | 0.900 | 0.705 | 0.893 | 0.932 | 1.000 | 0.64 s mean, 0.35 s median |

On the fresh set:
- **edgar-crawler** takes the list of Item headings that McDonald's 2003 10-K prints again after the signatures as the headings of Items 5 to 15. FedEx's 10-Q has no Part I Item 1 heading in the body, and edgar-crawler starts that Item at a table-of-contents line. In Abbott's 10-Q it misses Item 2.
- **edgartools** returns no Items for the 4 plain-text filings. It misses some Items in five 10-Ks (McDonald's, Allstate, US Bancorp, Kroger, Research Frontiers).

These are the 104 tuned filings, with the three sets combined. parsec-edgar's scores here are in-sample:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-Q F1 | 8-K F1 | Time per filing |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar (this branch, in-sample) | 1.000 | 1.000 | 1.000 | 0.993 | 1.000 | 1.000 | 1.000 | 31 ms mean, 15 ms median |
| parsec-edgar (initial commit) | 0.926 | 0.878 | 0.902 | 0.775 | 0.936 | 0.708 | 1.000 | 31 ms mean, 15 ms median |
| edgar-crawler | 0.990 | 0.989 | 0.990 | 0.914 | 0.997 | 0.948 | 1.000 | 0.43 s mean |
| edgartools | 0.953 | 0.826 | 0.885 | 0.677 | 0.871 | 0.926 | 1.000 | 1.07 s mean, 0.54 s median |

By label set:

| Tool | Development: F1 | Development: span | Held out: F1 | Held out: span | Blind: F1 | Blind: span | Fresh: F1 | Fresh: span |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar, first scoring | | | 0.955 | 0.875 | 0.989 | 0.997 | 0.994 | 0.935 |
| parsec-edgar, after tuning | 1.000 | 0.990 | 1.000 | 1.000 | 1.000 | 1.000 | 0.994 | 0.935 |
| edgar-crawler | 0.994 | 0.907 | 0.987 | 0.928 | 0.972 | 0.925 | 0.961 | 0.843 |
| edgartools | 0.876 | 0.639 | 0.884 | 0.675 | 0.928 | 0.810 | 0.900 | 0.705 |

How to read these numbers:
- **Tuning.** parsec-edgar was tuned on the development, held-out and blind sets, and the other tools were not. Compare the tools on the fresh set, or on the first scorings of the blind and held-out sets. The development set has no such score, because the rules were written while looking at it.
- **edgar-crawler** finds Item starts nearly as well, from a hard-coded list of each form's Items.
  - It loses span accuracy on the last Item, which runs on into attached annual reports (span 0.136 on the `annual-report-attached` filings).
  - Some 10-Q Items start at the table of contents or at a "(continued)" page header.
- **edgartools** returns no Items for filings without an HTML document, so it scores 0.015 F1 on the `plain-text` filings. It also misses Items in many exhibit-13 and small-company 10-Ks.
- **Time.** The Python times include reading the filing from disk, and edgartools' times include parsing the whole submission. The parsec-edgar times start after the cached document has been read.
  - edgar-crawler's time is its whole run, minus its start-up time, divided by the number of filings.
  - edgartools' time is measured per filing.
