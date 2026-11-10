# Section-splitting benchmark

This benchmark measures how well `extract_sections` finds the Items of 10-K, 10-Q, 8-K, 10-KSB and 10-QSB filings, and how long text conversion and splitting take. It compares the output with hand labels for 163 filings filed from 1997 to 2026, in five sets. The same labels also score [edgar-crawler](https://github.com/nlpaueb/edgar-crawler) and [edgartools](https://github.com/dgunning/edgartools).

## Accuracy in brief

The numbers that show how the extractor does on filings it has not seen are each set's first scoring, made before any of that set's misses were fixed. The newest set, fresh2, was labeled and committed before any scoring, then scored once with changes 1 to 20 below. One fix followed from that scoring (change 21), so its current score is partly in-sample. The extractor has been tuned on the four older sets, so their current scores are in-sample. Measuring the current rules fully out of sample would take another new set, labeled and committed before it is scored.

| Label set | Filings | First scoring | Item-start F1 | Span accuracy |
|---|--:|---|--:|--:|
| `bench/fresh2` | 30 | Labels committed first, then scored once. | 0.988 | 0.895 |
| `bench/fresh` | 25 | Labels committed first, then scored once (version 0.1.1). Its misses were fixed afterward. | 0.994 | 0.935 |
| `bench/blind` | 19 | Labels committed first, then scored once. Its two misses were fixed afterward. | 0.989 | 0.997 |
| `bench/holdout` | 20 | Scored after the first round of changes. Its misses were fixed afterward. | 0.955 | 0.875 |
| `bench/fresh2` now, partly in-sample | 30 | After the one fix made from its first scoring | 0.990 | 0.900 |
| The other 133 filings, in-sample | 133 | After tuning on them | 1.000 | 1.000 |

At its first scoring fresh2 had precision 0.990 and recall 0.985: 397 of the 403 labeled Item starts were found, and 4 predicted Items start on no labeled line. Version 0.1.1 scores slightly better on the same labels, F1 0.989 with span 0.900. edgar-crawler scores F1 0.980 with span 0.917, and edgartools F1 0.803 with span 0.624. So edgar-crawler's span accuracy on fresh2 is higher than parsec-edgar's, at the first scoring and after the fix; two filings cause it (see [Scores](#scores)). Compare the other tools with the first scorings, not with the current in-sample 1.000 of the older sets.

Span accuracy is computed per filing: the characters assigned to the right Item, divided by the characters inside any labeled or predicted Item. The tables show the mean over filings. It is not the share of labeled text that was found. A filing whose last Item runs on through pages that the labels leave out of every Item scores well under 1, even when every Item starts on the right line.

## Running it

```sh
export PARSEC_EDGAR_USER_AGENT="Your Name you@example.com"
cargo run --release -p parsec-edgar-bench -- score                           # development set
cargo run --release -p parsec-edgar-bench -- --labels bench/holdout score
cargo run --release -p parsec-edgar-bench -- --labels bench/blind score
cargo run --release -p parsec-edgar-bench -- --labels bench/fresh score
cargo run --release -p parsec-edgar-bench -- --labels bench/fresh2 score
```

The first run downloads the labeled documents into the cache, at 3 requests per second. The cache is `~/.cache/parsec-edgar`, or `PARSEC_EDGAR_CACHE` when set. Later runs read only the cache.

`score` takes these options:
- `--filter TEXT` keeps only filings whose accession number, company or form contains TEXT.
- `--repeat N` times each filing N times and keeps the fastest run.
- `--json FILE` writes the per-filing results.
- `--no-misses` leaves out the list of misses.

The report has one row per group: 10-K by era of the filing date, 10-KSB, 10-QSB, 10-Q, 8-K, all 10-Ks (without the 10-KSBs), and all filings. A group with no filings in the set is left out. It then has one row per tag. After that it lists every filing with a miss, Item by Item, with the labeled and predicted line numbers and character counts.

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
| `bench/labels` | 69 | 962 | Development set. The extractor was tuned on it. |
| `bench/holdout` | 20 | 276 | Labeled after the first round of changes and scored with them frozen (F1 0.955, span 0.875). Its misses were then fixed, so it is no longer held out. |
| `bench/blind` | 19 | 180 | Labeled and committed before any scoring, then scored once with the extractor frozen (F1 0.989, span 0.997). Its two misses were then fixed, so it is no longer blind either. |
| `bench/fresh` | 25 | 349 | Labeled and committed after changes 1 to 8 below, then scored once (F1 0.994, span 0.935; version 0.1.1 scores the same). Its misses were fixed after version 0.1.1, so it is no longer untouched either. |
| `bench/fresh2` | 30 | 403 | Chosen by fixed rules, labeled twice independently and committed before any scoring, then scored once with changes 1 to 20 (F1 0.988, span 0.895). The one regression it showed was fixed afterward (change 21). |

The five sets hold 163 filings and 2,170 labeled Items. The development set has these filings:
- 10-K: nine or ten in each of five eras from 1997 to 2026, 46 in all.
- 10-KSB: four, from 1998 to 2008.
- 10-Q: ten, from 1999 to 2025.
- 8-K: nine.

The fresh set has 16 10-Ks from 1998 to 2025, five 10-Qs from 1999 to 2023 and four 8-Ks from 2007 to 2025. None of its companies appear in another set. The filings were picked from EDGAR's quarterly indexes and from company filing lists to cover eras, forms and filing agents, and labeled with the commands below without running the extractor.

The fresh2 set has 18 annual reports (17 10-Ks and a 10-K405) from 1997 to 2026, seven 10-Qs from 1997 to 2026 and five 8-Ks from 2005 to 2025. None of its companies appear in another set or in the regression sample. [The fresh2 set](#the-fresh2-set) says how they were chosen and labeled.

Tags mark the hard cases. The counts are for the development set; the fresh set has 4 `plain-text`, 3 `exhibit-13`, 4 `bank`, 2 `insurer` and 7 `small-cap` filings, and fresh2 has 8 `plain-text`, 2 `exhibit-13`, 1 `annual-report-attached`, 1 `running-headers`, 4 `bank`, 2 `insurer` and 10 `small-cap` filings.

| Tag | Filings | What it marks |
|---|--:|---|
| `plain-text` | 13 | Filings with no HTML document, most of them from before 2001. |
| `exhibit-13` | 7 | The 10-K answers most Items by referring to an attached annual report. |
| `annual-report-attached` | 5 | A full annual report follows the last Item. |
| `cross-reference` | 4 | Items appear only in a cross-reference index, with no body headings. The labels then list the Items as absent. |
| `running-headers` | 5 | Every page repeats the Item heading as a header. |
| `bank` | 12 | Bank filings. |
| `insurer` | 6 | Insurer filings. |
| `small-cap` | 16 | Small-company filings. |
| `10-ksb` | 4 | Form 10-KSB, the annual report of a small business issuer, with Items of its own. |

The filings come from many filing agents: self-filed, DFIN (Donnelley), Bowne, Toppan Merrill, Workiva, EdgarAgents, M2 Compliance and others. They include 10-K405s, 10-KSBs and Items such as 1C, 9C and 16.

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

Three versions are compared: the library at its initial commit, version 0.1.1, and main, the current code. All scores are from release builds.

| Label set | Initial commit: P / R / F1 | Initial commit: span | 0.1.1: P / R / F1 | 0.1.1: span | Main: P / R / F1 | Main: span |
|---|---|--:|---|--:|---|--:|
| Fresh2 (30) | | | 0.990 / 0.988 / 0.989 | 0.900 | 0.990 / 0.990 / 0.990 | 0.900 |
| Fresh (25) | 0.949 / 0.905 / 0.927 | 0.756 | 1.000 / 0.989 / 0.994 | 0.935 | 1.000 / 1.000 / 1.000 | 1.000 |
| Development (69) | 0.946 / 0.885 / 0.914 | 0.807 | 1.000 / 1.000 / 1.000 | 0.990 | 1.000 / 1.000 / 1.000 | 1.000 |
| Held out (20) | 0.957 / 0.880 / 0.917 | 0.753 | 1.000 / 1.000 / 1.000 | 1.000 | 1.000 / 1.000 / 1.000 | 1.000 |
| Blind (19) | 0.799 / 0.839 / 0.818 | 0.686 | 1.000 / 1.000 / 1.000 | 1.000 | 1.000 / 1.000 / 1.000 | 1.000 |
| Development, held out and blind (108) | 0.926 / 0.878 / 0.902 | 0.775 | 1.000 / 1.000 / 1.000 | 0.993 | 1.000 / 1.000 / 1.000 | 1.000 |

The initial commit was not scored on fresh2. The initial commit and version 0.1.1 were scored on the development set before its four 10-KSB filings were added, so their development rows are for 65 filings and their combined rows for 104; version 0.1.1 does not read the form type "10KSB" and gives those four no sections. Version 0.1.1's fresh and fresh2 rows are first scorings of those sets, out of sample; its other rows are in-sample. Every row for main is in-sample, except fresh2's, which is partly in-sample: it is after the one fix made from fresh2's first scoring (0.990 / 0.985 / 0.988, span 0.895).

### The fresh set

| Group | Filings | Initial commit: F1 | Initial commit: span | 0.1.1 (first scoring): F1 | 0.1.1: span | Main: F1 | Main: span |
|---|--:|--:|--:|--:|--:|--:|--:|
| 10-K 1997 to 2001 | 3 | 0.795 | 0.487 | 1.000 | 0.820 | 1.000 | 0.999 |
| 10-K 2002 to 2008 | 3 | 0.991 | 0.986 | 0.991 | 0.986 | 1.000 | 1.000 |
| 10-K 2009 to 2015 | 3 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| 10-K 2016 to 2020 | 3 | 1.000 | 0.828 | 1.000 | 0.828 | 1.000 | 1.000 |
| 10-K 2021 to 2026 | 4 | 1.000 | 0.997 | 1.000 | 0.997 | 1.000 | 1.000 |
| 10-Q | 5 | 0.541 | 0.203 | 0.958 | 0.897 | 1.000 | 1.000 |
| 8-K | 4 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |

The library before the review fixes (change 9 below) scores exactly the same on the fresh set as version 0.1.1. At that first scoring, seven filings had misses. Each was fixed after version 0.1.1, and each fix has unit tests of its own:
- **Capital One 10-Q 2020.** Item 2 is printed before Item 1, under a single PART I line. Item 1 had no PART line of its own, so it was not added as an out-of-order Item: it was missed, and Item 2 ran over it (span 0.530). Its title, "Consolidated Financial Statements", is a Part I title only, and the Item is now added out of order from it (change 6).
- **Samsara Luggage 10-Q 2023.** "Item 3. - Quantitative and Qualitative Disclosures about Market Risk" and "Item 4. - Controls and Procedures" have two separators. The title then started with a dash, the heading test rejected both, and their text went into Item 2. Dashes after the separator are now skipped (change 10).
- **McDonald's 10-K 2003.** Item 5 is printed as "Item 5A" and "Item 5B". Neither is a 10-K Item, so Item 5 was missed, and its text belonged to no Item. "Item 5A" now starts Item 5 (change 7).
- **Kroger 10-K 2019.** There is no Item 8 heading. The labels leave the financial statements out of every Item, and the extractor kept them in Item 7A (span 0.483). Item 7A now ends at the auditors' report (change 11).
- **Allen Organ 10-K 1998.** The financial statements are printed between Item 9 and Part III with no heading. The labels leave them out of every Item, and the extractor kept them in Item 9 (span 0.461). Item 9 now ends at the auditors' report (change 11). The auditors' name and address printed just above that report, 63 characters, stay in Item 9, so the span is 0.998.
- **Lockheed Martin 10-K 2006.** "ITEM 4(a). EXECUTIVE OFFICERS OF THE REGISTRANT" was read as a sub-item of Item 4, so Item 4 ran over the executive officers section (span 0.983). It now ends Item 4 the way "Item 4A" does (change 7).
- **Patriot National Bancorp 10-K 2025.** Item 16 ran on through an exhibit index printed before the signatures (span 0.988). Item 16 now ends at the exhibit index (change 11).

These are five kinds of layout: an out-of-order 10-Q Item found by its title (Capital One), dashes after the separator (Samsara), lettered Item numbers (McDonald's), an executive officers section numbered as a sub-item (Lockheed Martin), and text with no Item heading after Items 7A, 9 and 16 (Kroger, Allen Organ, Patriot National Bancorp). edgar-crawler and edgartools also keep the Kroger and Allen Organ statements in the Item before them.

### The fresh2 set

**How it was chosen.** A script chose the 30 filings from EDGAR's quarterly master indexes, before any labeling. The set is a fixed list of slots, each naming a form type, a filing year, an industry and a size class. A slot with no industry noted is for a company that is neither a bank nor an insurer, and a slot with no size noted is for a large one:
- 10-K: 1997 (bank), 1998 (10-K405, small), 2000, 2002, 2004 (insurer), 2006 (small), 2008, 2009, 2011 (bank, small), 2013, 2015, 2016, 2018 (insurer), 2020 (small), 2021, 2023 (bank), 2024 (small), 2026.
- 10-Q: 1997, 2002 (bank, small), 2007, 2011, 2016, 2021, 2026.
- 8-K: 2005, 2010, 2014, 2020 (small), 2025.

A slot's candidates are all filings of exactly its form type in the four quarterly indexes of its year (for 2026 the first three, since the fourth was still open), so amendments are never candidates. They are sorted by the SHA-256 hex digest of "fresh2:" followed by the accession number ("fresh2:0000928385-97-000562"), and the slot takes the first candidate that passes these tests, checked in this order. Slots are filled in the order listed above.
1. No company that the index lists for the filing is in another label set or in the regression sample below, or was chosen for an earlier slot.
2. The accession number starts with the company's own CIK, or its prefix has fewer than two filings in the set so far, so no filing agent has more than two.
3. The company is not a trust or an asset-backed issuer: its SIC code is not 6189, and its name holds neither the word Trust (banks excepted) nor a securitization term (receivables, pass-through, certificates, mortgage loan, asset-backed, grantor, securitization).
4. EDGAR has an SIC code for the company, and it fits the slot: banks are 6020 to 6036, insurers 6311 to 6399, and every other slot excludes both ranges.
5. The public float, the market value of common equity held by non-affiliates, fits the slot's size: under $75 million (the accelerated-filer threshold) for a small company, and $75 million or more for a large one. It is read from the filing's own cover for a 10-K, and from the cover of the company's latest 10-K or 10-K405 filed on or before it for a 10-Q or 8-K. A candidate is skipped when that cover states no float: no trading market, a wholly owned subsidiary, partnership units, $0 held by non-affiliates, or no earlier 10-K.
6. Lines 1 to 40 of the benchmark's text of the document are readable text.
7. For an 8-K, the `items` command shows at least one "Item N.NN" heading.

292 candidates were skipped: 185 for their industry, 40 as trusts or asset-backed issuers by name, 9 for SIC code 6189, 20 for falling on the wrong side of $75 million, 18 for stating no float, 10 for being in an existing label set and 10 for the filing-agent cap. None was skipped because its document could not be read. Company names and SIC codes are EDGAR's current values, so a few names differ from the filer's name at the time.

**How it was labeled.** Two labelers labeled each filing independently, from the document text alone, with the commands under [Labels](#labels). No parser was run on these filings. The labelers put all 403 Item starts on the same lines. They disagreed on where three filings end and on where one Item ends, and on fields the score does not use (`incorporated`, `absent` and tags). Each difference was resolved by reading the text and following the labels of the earlier sets:
- ACI Worldwide 10-K 2021 prints financial statements after Item 15, then an exhibit index, then the signatures. The filing ends at the exhibit index, the first end marker after the last Item's text. Item 15 has its own end where the statements begin, so this changes no score.
- MONY Group 10-K 2004 and Alleghany 10-K 2018 print an exhibit index right after Item 15, then the signatures. The index answers Item 15, so it stays in that Item and the filing ends at the signatures, as in three development set labels.
- In ITT's 10-K 2015, one labeler ended Item 15 at the index to the financial statements printed after it, and the other ran it on through the statements. Statements with no Item heading belong to no Item, as in the Netflix 2003 label, so Item 15 ends at the index.

The labels were committed to main in 4ddc5d9, before any scoring.

**First scoring.** The set was scored once, with the code at b0131b7, which has changes 1 to 20 below. Version 0.1.1 and main before changes 15 to 20 (e3d004b) score exactly the same as each other on these labels, and neither was tuned on them.

| Group | Filings | First scoring: F1 | First scoring: span | 0.1.1: F1 | 0.1.1: span | Main: F1 | Main: span |
|---|--:|--:|--:|--:|--:|--:|--:|
| 10-K 1997 to 2001 | 3 | 1.000 | 1.000 | 0.989 | 0.999 | 1.000 | 1.000 |
| 10-K 2002 to 2008 | 4 | 0.986 | 0.964 | 1.000 | 0.999 | 1.000 | 0.999 |
| 10-K 2009 to 2015 | 4 | 1.000 | 0.894 | 1.000 | 0.894 | 1.000 | 0.894 |
| 10-K 2016 to 2020 | 3 | 0.983 | 0.946 | 0.983 | 0.946 | 0.983 | 0.946 |
| 10-K 2021 to 2026 | 4 | 1.000 | 0.895 | 1.000 | 0.895 | 1.000 | 0.895 |
| 10-Q | 7 | 0.962 | 0.857 | 0.962 | 0.857 | 0.962 | 0.857 |
| 8-K | 5 | 0.900 | 0.800 | 0.900 | 0.800 | 0.900 | 0.800 |
| All | 30 | 0.988 | 0.895 | 0.989 | 0.900 | 0.990 | 0.900 |

The 10-K groups include the 10-K405. Compared with version 0.1.1, the first scoring has one filing worse and one better:
- **Norfolk Southern 10-K 2002** (span 0.856, against 0.998 for version 0.1.1). It prints "Item 1. Business." and "Item 2. Properties." on separate underlined lines with a line holding only "and" between them, and does the same for Items 12 and 13. The check for cross-references wrapped onto a new line (change 19) took "and" for a sentence going on, so Items 2 and 13 were missed, and Items 1 and 12 ran over them.
- **Ultimate Electronics 10-K405 1998.** "ITEM 7(a). QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK" is now Item 7A, by the rule for sub-items printed with a lettered Item's title (change 16). Version 0.1.1 missed it.

**The fix.** Change 21 makes a line that holds only a joining word link the two headings around it, so the heading after it stands. With it, Norfolk Southern finds all 15 Items, and fresh2 as a whole scores F1 0.990 (precision 0.990, recall 0.990) with span 0.900. No filing of the other four sets and none of the 150 sample filings changes. This score is partly in-sample, since the fix was made after seeing fresh2's first scoring.

**What is still missed.** Seven filings lose span after the fix:
- **Superior Well Services 8-K 2010 (span 0.000, #19).** The cover page lists "Item 7.01 Regulation FD Disclosure" and "SIGNATURES" as a short contents list, two lines above the body heading. The list's line is taken as the heading, and the Item ends at the first "SIGNATURES", so it holds 35 characters against 1,007 labeled.
- **Harleysville Savings Financial 10-Q 2002 (span 0.001, #20).** The body has no Part I Item headings, and the labels list those Items as absent. The table of contents, whose page numbers are ranges such as "9 - 10", is taken as Part I Items 1 to 3, the last of which then runs on through the report (45,459 characters), and "Item 1-5. Not applicable." in Part II is not read as a heading.
- **ACI Worldwide 10-K 2021 (span 0.581) and MSB Financial 10-K 2011 (span 0.592), #12.** The financial statements printed after Item 15 have no F-page numbers, so they stay in Item 15 (139,500 and 103,655 characters, against 1,303 and 2,071 labeled). The labels end Item 15 where the statements begin. All three tools score about the same on both.
- **A10 Networks 10-K 2016 (span 0.875, #23).** Item 1 is printed only as "BUSINESS" under "PART I.", with no number, so it is missed and its 40,909 characters are in no section. The label is marked uncertain.
- **Encision 10-K 2020 (span 0.961, #21).** "Item 9 A (T). Controls and Procedures." is not read as a heading, so Item 9A's 5,469 characters stay in Item 9.
- **ITT 10-K 2015 (span 0.985, #25).** The auditors' report on internal control is printed after Item 9B with no heading and stays in Item 9B (6,421 characters against 2,246); the label is marked uncertain. The index heading where Item 15 is labeled to end, 43 characters, also stays in Item 15.
- **Norfolk Southern 10-K 2002 (span 0.998).** A power of attorney paragraph between Item 14 and the signatures, 478 characters, stays in Item 14. The labels leave it out of every Item, since the contents list it apart from Item 14.

The other tools' scores on fresh2, and the filings where they differ most from parsec-edgar, are under [Scores](#scores).

### Development set, by group

| Group | Initial commit: F1 | Initial commit: span | 0.1.1: F1 | 0.1.1: span | Main: F1 | Main: span |
|---|--:|--:|--:|--:|--:|--:|
| 10-K 1997 to 2001 | 0.683 | 0.551 | 1.000 | 0.998 | 1.000 | 0.998 |
| 10-K 2002 to 2008 | 0.959 | 0.878 | 1.000 | 0.968 | 1.000 | 1.000 |
| 10-K 2009 to 2015 | 1.000 | 0.898 | 1.000 | 1.000 | 1.000 | 1.000 |
| 10-K 2016 to 2020 | 0.955 | 0.865 | 1.000 | 0.962 | 1.000 | 1.000 |
| 10-K 2021 to 2026 | 0.944 | 0.906 | 1.000 | 1.000 | 1.000 | 1.000 |
| 10-KSB | | | | | 1.000 | 1.000 |
| 10-Q | 0.767 | 0.616 | 1.000 | 1.000 | 1.000 | 1.000 |
| 8-K | 1.000 | 0.950 | 1.000 | 1.000 | 1.000 | 1.000 |
| `plain-text` | 0.657 | 0.502 | 1.000 | 0.998 | 1.000 | 0.999 |
| `annual-report-attached` | 0.916 | 0.107 | 1.000 | 0.996 | 1.000 | 0.996 |
| `running-headers` | 0.751 | 0.995 | 1.000 | 1.000 | 1.000 | 1.000 |

The initial commit and version 0.1.1 were not scored on the four 10-KSB filings, so their `plain-text` row has 10 filings and main's 13.

Every Item starts on its labeled line in all 133 filings of the four older sets. These filings still lose span:
- **JPMorgan Chase 10-K405 2001 (span 0.982).** The attached annual report starts with a "FINANCIAL TABLE OF CONTENTS" line and three paragraphs of notes, then the heading "MANAGEMENT'S DISCUSSION AND ANALYSIS:" and the page-numbered contents. Item 14 ends at that heading, so the first line and the notes, 1,455 characters, stay in Item 14.
- **Exxon Mobil 10-K 2005 (span 0.998).** Item 15 ends at the heading of the attached financial section. Two "Index to Financial Statements" page links and a "[THIS PAGE INTENTIONALLY LEFT BLANK]" line just above it, 97 characters, stay in Item 15.
- **Netflix 10-K 2003 (span 0.9999).** Version 0.1.1 scored it 0.684: the financial statement pages that follow Item 15 with no heading of their own ran on in Item 15 (68,270 characters against 4,910 labeled). Item 15 now ends at their index, whose entries have F-page numbers (change 17), and the "NETFLIX, INC." page header above that index, 14 characters, stays in it.
- **Netflix 10-K 2017 (span 0.9999).** Version 0.1.1 scored it 0.654: Item 16, "Form 10-K Summary", ran on through the financial statements. Item 16 now ends at the index to the financial statements (change 11), and the company name printed as a page header above that index, 14 characters, stays in it.
- **Allen Organ 10-K 1998** in the fresh set (span 0.998), described above.

### The changes

Changes 1 to 8 were made before the fresh set was labeled. Change 9 came after its first scoring and did not change it. Version 0.1.1 has changes 1 to 9. The parts marked "After version 0.1.1", and changes 10 to 14, came after it. Changes 15 to 20 were made while fresh2 was being labeled, and no rule was checked against its labels before its first scoring. Change 21 came after that scoring. Changes 10 to 21 are not in a release yet.

1. **Plain-text filings.** Old filings whose text contains SGML `<TABLE>` tags are no longer rendered as HTML. Rendering them as HTML merged their lines into a few long ones.
2. **Contents entries.** A table-of-contents entry is dropped when the same Item and title come back later at a longer stretch of text. Entries next to a dropped one go with it.
   - After version 0.1.1: the spread to neighboring entries is narrower. An entry goes when the candidate before it is a contents entry too, or when it comes before the first body heading that a contents entry names and no Item earlier in the form's order, with text of its own, comes before it. A short body Item printed once just before a second contents list, or before an index of Items, stays. An Item line above the contents (a 10-Q/A's explanatory note) no longer keeps the contents rows after it.
3. **Running headers.** A bare running header ("Item 7") just before the Item's titled heading is dropped. A sub-item reference such as "Item 15(a)" with no title is not a heading.
4. **End of the last Item.** The last Item ends:
   - at the signatures;
   - at the signature block's opening sentence, when there is no signatures heading;
   - at an attached document with its own page-numbered table of contents.
5. **Page headers and footers.** Lines that carry a page number and repeat page after page are removed from the text.
   - After version 0.1.1: short headers and footers that name the form ("2021 Form 10-K 100") or start with "Page N of M" are removed too, also when the form name is spelled with a non-breaking hyphen or an en dash. So are footers printed as a three-cell row with the page number in the middle cell, and short footers printed cell by cell that repeat on five or more pages with only a rising page number changing. A page numbered in roman numerals followed by one in digits counts as rising. Lines that are the cells of a table row with two or more cells are never removed one by one, so table values such as "5" or "2024" stay.
6. **10-Q parts.** PART lines decide whether a 10-Q Item belongs to Part I or Part II. Items printed out of order are kept.
   - After version 0.1.1: an Item printed after a higher-numbered Item with no PART line above it is added out of order when its title is the standard title of that Item in Part I only. Leading words "Consolidated", "Condensed", "Unaudited" and "Interim" do not stop that match, so "Item 1. Condensed Consolidated Financial Statements" counts as Part I. Part II titles are not used, because Part II Items 1A and 4 share number and title with 10-K Items that 10-Qs cite. An out-of-order Item can also come before the chain's first heading, when it is at or after the first body heading that a contents entry names (MD&A printed before longer financial statements).
7. **10-K Items the form does not define.** A titled heading such as "Item 4A. Executive Officers" ends the Item before it.
   - After version 0.1.1: an executive officers caption numbered as a sub-item ("ITEM 4(a). EXECUTIVE OFFICERS OF THE REGISTRANT") reads as Item 4A in a 10-K when the form does not define that Item, so it ends Item 4 the same way. "Item 4A" in a 10-K never starts Item 4, whatever its title. When an Item is printed only as lettered headings the form does not define ("Item 5A", "Item 5B"), the first one placed where that Item belongs starts it, if no heading of that Item was found; the later ones stay inside it. In a 10-Q, such a heading takes its part from a PART line or its title.
8. **Item lists after the signatures.** A list of Item headings printed again after the signatures is ignored.
9. **Review fixes.** A code review found layouts where changes 2, 3 and 8 lose Items that the initial commit found, and two layouts that change 6 gets wrong. Unit tests written for layouts near each rule found two more cases. Each case now has a unit test that fails without its fix:
   - A "Signatures" line inside the table of contents no longer makes the body's headings look like a list printed after the signatures. Only a signatures line at or after the first body heading counts, so an empty "Item 6. [Reserved]" and a short last Item are kept.
   - Contents entries spread to their neighbors, but not onto the later copy of a heading that appears twice, so a short body Item ("Not applicable.", "Item 6. [Reserved]") just before a second contents list is kept.
   - A bare heading ("ITEM 1." with "BUSINESS" on the next line) is kept when its next line is the title of the heading that follows it, or the Item's standard title. Only bare running headers are dropped.
   - The search for a PART line above a 10-Q heading stops at another Item's heading line, including a contents row such as "PART II - Item 6. Exhibits", so a "PART II" in the contents no longer moves the body's first Items to Part II.
   - Out-of-order 10-Q Items are added only before the signatures.
   - A 10-Q Item with no PART line above it takes its part from its title, when the title is the standard title of that Item in only one part. A lone "Item 1. Legal Proceedings" is then Part II.
   - A letter joins the Item number when spaces alone separate them ("Item 9 A.") or when a separator with no space after it does ("Item 1.A."), so "ITEM 1. A. GENERAL" stays Item 1.
10. **Dashes in headings** (after version 0.1.1). Dashes right after the separator ("Item 3. - Quantitative and Qualitative Disclosures", "ITEM 1 -- FINANCIAL STATEMENTS") are skipped before the title test, in 10-K, 10-Q and 8-K headings, so the text after them must still look like a title. A run of dashes between PART and ITEM ("PART I -- ITEM 2") is accepted too.
11. **Text with no Item heading** (after version 0.1.1). A line that opens such text ends 10-K Items 9 and 16, and Item 7A when the filing has no Item 8 heading: an auditors' report, an index to the financial statements, a financial statement schedule ("SCHEDULE I - CONDENSED PARENT COMPANY FINANCIAL STATEMENTS"), an exhibit index or an accountants' consent. The line must start with a capital letter. Other Items keep these headings as their own text. An Item 16 titled as the exhibits Item keeps its statements and exhibit index, and the rule does not cut a section that also holds the next Item ("Items 9 and 9A.", or "ITEM 9 A – CONTROLS AND PROCEDURES" after "ITEM 9.").
12. **Wrapped cross-references** (after version 0.1.1). In a 10-Q, a line that starts with "Item" and a Part I title but runs on as a sentence ("Item 1. Financial Statements, of this report", "Item 2. Management's Discussion and Analysis, under ...") is a hard-wrapped cross-reference, not a heading, and no longer cuts the Item it sits in short. Only a reference word such as "of this report", or a comma followed by a lowercase word, makes it one; a heading that goes on with words of its own stays.
13. **Form names without a hyphen** (after version 0.1.1). `FormKind::from_form` accepts EDGAR's index spellings 10KSB, 10KSB40, 10KT405, 10QSB and the like, so 10-KSB and 10-QSB filings are split. The four 10-KSB filings of the development set use the spelling "10KSB".
14. **Table cells in the text** (after version 0.1.1). A "$" or "(" cell joins the cell after it only when that cell starts with a digit, a dash or a ".", a "%" cell joins the cell before it only when that cell ends with a digit or a dash, and a ")" cell joins a cell that leaves a "(" open. Header cells such as "Amount" keep the "%" column beside them as a cell of their own, and "$" next to "N/A" stays apart. The same merges now happen in rows printed cell by cell. These change only the text inside Items.
15. **10-KSB and 10-QSB Items** (#14). 10-KSB and 10-QSB filings are split with Item lists of their own, with the titles of the forms' last versions (2008), instead of the 10-K and 10-Q lists. The 10-KSB list has Items 8A and 8B, and also Item 1A (risk factors) and Item 15 (accountant fees, printed in 10-KSBs for 2003, whose Item 14 was controls and procedures). The 10-QSB list has Part I Items 1 to 3 and Part II Items 1, 1A and 2 to 6. The 10-K rules for the executive officers caption and for titled Items the form does not define apply to a 10-KSB, and the 10-Q rules for parts apply to a 10-QSB, with its own titles. A block heading (change 11) ends 10-KSB Item 8 (changes in accountants) and Items 14 and 15, which come after the exhibits, unless the Item is titled as the exhibits Item.
    - A filing is split with the other form's list (10-K and 10-KSB, 10-Q and 10-QSB) when its headings fit that list strictly better. The count is the Items printed with a number and a title that one list gives to that number alone and the other list gives to another number and not to this one. So "Item 7. Financial Statements" counts for 10-KSB and "Item 8. Financial Statements" for 10-K, while "Item 4. Submission of Matters to a Vote of Security Holders" and Items 14 and 15 "Principal Accountant Fees and Services" count for neither. A tie keeps the form type's list.
16. **Lettered sub-items and form names in titles.** A sub-item printed with the standard title of a lettered Item that the form defines is that Item: "Item 8(A)T. Controls and Procedures" in a 10-KSB is Item 8A, and "ITEM 9(A). CONTROLS AND PROCEDURES" in a 10-K is Item 9A. This holds also when nothing follows the letter and the title is on the next line ("ITEM 9(A)." or "Item 9(a)." above "CONTROLS AND PROCEDURES"). Another title ("ITEM 1(a). GENERAL DEVELOPMENT OF BUSINESS") or a sub-item reference ("Item 9(a)(1)") stays inside the Item. A title may also start with a form's name followed by a capitalized word, so "ITEM 16. 10-K SUMMARY" is a heading, while "Item 16 10-K filed in 1998" is not.
17. **Financial statements after the exhibits Item** (#12). The exhibits Item (10-K Item 15, the old Item 14, 10-KSB Item 13) ends where it prints financial statements whose pages are numbered apart from the report's, after all of its own text. That needs an index of at least three entries with F-page numbers ("F-2", "F - 1", "F-7 to F-20"), then headings with the titles of at least two of its entries. The Item ends at the index's heading when the statements follow the index directly, and otherwise at the first statement, so an index that is the Item's own "(a)(1)" list stays in it. Statements with the report's own page numbers stay in the Item, as NVIDIA's 2025 labels have them, and 10-K Item 8 and 10-KSB Item 7 are never cut this way.
    - The statements stay in the Item when any of its own text follows the first of them: an exhibit index, a caption such as "(a)(3) Exhibits:", "3. Exhibits", "LIST OF EXHIBITS", "2. Financial Statement Schedules" or "(b) Reports on Form 8-K", or an exhibit table's header ("Exhibit No.", "Exhibit Number"). Numbered notes inside the statements ("2. Inventories", "(b) Use of estimates") do not count.
    - An Item is the exhibits Item by its printed title only when it comes after the Item that holds the statements (10-K Item 8, 10-KSB Item 7), so a 10-K's Item 7 printed as "Financial Statements" keeps its statements.
18. **Glossaries after the last Item** (#13). A glossary heading ("Glossary", "GLOSSARY OF SELECTED TERMS:", "Glossary of Terms and Acronyms") ends 10-K Item 16 and 10-KSB Items 14 and 15, as a block heading does, unless the Item is titled as the exhibits Item. The line must start with a capital letter. A contents entry ("Glossary" and a page number) or a sentence is not a heading, and a glossary inside another Item stays there.
19. **Wrapped cross-references in every form** (#15). An Item line is not a heading, in any form, when the line just above it ends inside a sentence: with a comma, a joining word ("in", "of", "to", "and" and the like), or "see", "under", "within" or "this". For example, "The information set forth in Part II," then "Item 7. Management's Discussion and Analysis, of the Company's Annual Report ...". A blank line, or a line holding only a PART label ("PART II,"), does not count. The words are compared in their case, so a page header in capitals that ends in "AND" does not count either.
20. **Page labels** (#16). Page numbers alone on their line after a dash and a prefix of one to three capital letters ("FS - 12", "S-3", "II-4"), a chapter number ("7-3") or nothing ("- 4") are removed when five or more copies of one prefix are mostly not on consecutive lines and mostly come one to three pages after the copy before, or start a new chapter at page 3 or lower. So exhibit lists ("EX-21", "EX-23") and number ranges stay. "F - 3" with any dash and spacing, "- -7-" in old text filings and numbered "<Page 2>" tags are removed as page numbers. A "Page 1 of M" footer after any page counts as rising, as when each exhibit numbers its own pages, unless the page is past the count. Running headers numbered in lowercase roman numerals join their family. A short header that does not name the form is removed when a "|" separates the number from it ("ACME | 23"), or when the number is at the start of some copies and at the end of others, at least two of each, as on facing pages. These change no score.
21. **Headings joined by "and"** (after the first scoring of fresh2). A line that holds only a joining word, such as "and" between "Item 1. Business." and "Item 2. Properties.", links the two headings, so change 19 no longer drops the heading after it.

The tests in `crates/parsec-edgar/src/sections.rs` also cover layouts where each special rule must not fire: a short attached document, a signature sentence without "duly caused", sub-item references, undefined Items mentioned in the text, short Part-marked lines, titled sub-items other than executive officers, lettered headings beside an Item that was found, Part I headings that go on with words of their own, and auditors' reports and exhibit indexes inside other Items. Since changes 15 to 21 they also cover sub-items with other titles or sub-item references on the next line, "Item 16 10-K filed" and "Item 16 10-K405", titles that both the 10-K and the 10-KSB numbering print, 10-KSB Items that keep their statements and exhibit indexes, statements numbered as the report's pages or printed before the exhibit index, numbered notes after F-paged statements, F-paged statements in 10-K Item 8 and 10-KSB Item 7, glossaries inside Items 1, 7 and 7A, and headings after lines that end without a period but outside a sentence ("None", "Not applicable", "PART II,", a blank line). The tests in `crates/parsec-edgar/src/html.rs` cover page lines and table cells that only look like headers or footers, and prefixed numbers that are not pages.

### Filings behind each rule

Each rule was turned off alone, and the tables count the filings whose score changes. Tuned filings are those of the development, held-out and blind sets: 104 for changes 1 to 14, counted before the four 10-KSB filings were added, and 108 for changes 15 to 21. Fresh filings are the 25 of the fresh set. Where a rule also changes fresh2 or the 150 sample filings of the regression check below, the last column says so. Turning off any rule also fails at least one unit test. A rule with a count of 0 or 1 was written for one layout; it rests on its unit tests and on the filings named here more than on the benchmark as a whole.

Section rules:

| Rule | Tuned filings | Fresh filings (of 25) | Filings that change |
|---|--:|--:|---|
| Plain-text filings kept out of the HTML renderer (1) | 13 | 3 | Plain-text filings from 1997 to 2009, from 9 companies; fresh: Hills Bancorporation 10-K 1998, Johnson & Johnson 10-K 1999, Chevron 10-Q 1999 |
| Contents entries dropped (2) | 4 | 4 | 10-Qs: JPMorgan 2010, Netflix 2012, Bank of America 2019, Wells Fargo 2012; fresh 10-Qs: Chevron 1999, FedEx 2008, Abbott 2014, Capital One 2020 |
| Contents entries spread to their neighbors (2) | 3 | 4 | The same tuned 10-Qs except Bank of America; fresh 10-Qs: Chevron, FedEx, Abbott, Samsara Luggage |
| Narrower spread, after version 0.1.1 (2) | 0 | 0 | None; it guards the short body Item before a second contents list |
| Bare running headers dropped (3) | 3 | 0 | Microsoft 10-Ks 2006, 2019 and 2025 |
| Sub-item references are not headings (3) | 1 | 0 | Coca-Cola 10-K 1999 |
| Wider signatures heading: "S I G N A T U R E S", "Signature(s)", a trailing colon (4) | 5, span only | 0 | Espey 10-Ks 1998, 2004, 2009 and 2018, Apple 8-K 2008 |
| Signature sentence ends the last Item (4) | 1, span only | 0 | Cisco 10-Q 2016 |
| Attached document ends the last Item (4) | 6, span only | 0 | Five JPMorgan 10-Ks, Exxon Mobil 2005 |
| PART lines set the 10-Q part (6) | 0 alone, 1 together with the title rule below | 0 alone, 1 together with it | Bank of America 10-Q 2019; fresh: Capital One 10-Q 2020 |
| Out-of-order 10-Q Items kept (6) | 1 | 1 | Bank of America 10-Q 2019; fresh: Capital One 10-Q 2020 |
| Out-of-order 10-Q Item found by its Part I title, after version 0.1.1 (6) | 0 | 1 | Capital One 10-Q 2020 |
| Out-of-order Items before the chain's first heading, after version 0.1.1 (6) | 0 | 0 | None; unit tests only |
| Leading "Consolidated", "Condensed", "Unaudited" in a title, after version 0.1.1 (6) | 0 | 0 | None; unit tests only |
| Undefined titled Items end the Item before (7) | 2, span only | 1, span only | Walmart 2000, Ford 2010; fresh: Lockheed Martin 2006 |
| "Item 4(a). Executive Officers" read as Item 4A, after version 0.1.1 (7) | 0 | 1, span only | Lockheed Martin 10-K 2006 |
| "Item 4A" never starts Item 4, after version 0.1.1 (7) | 0 | 0 | None; unit tests only |
| Lettered headings start a missing Item, after version 0.1.1 (7) | 0 | 1 | McDonald's 10-K 2003 |
| Item lists after the signatures ignored (8) | 0 alone, 1 together with title-based parts | 0 | HP 10-Q 2004 |
| Tie-break: the earlier Item wins (9) | 0 alone, 2 together with title-based parts | 0 | IBM 10-Q 2000, Coca-Cola 10-Q 2001 |
| 10-Q part from the Item's title (9) | 0 alone, 3 together with both rules above | 0 | HP 10-Q 2004, IBM 10-Q 2000, Coca-Cola 10-Q 2001 |
| Out-of-order Items only before the signatures (9) | 0 | 0 | None; it guards a layout found in review |
| Letter joined to the Item number ("Item 9 A.", "Item 1.A.") (9) | 1 | 0 | Berkshire Hathaway 10-K 2004 |
| Dashes after the separator (10) | 0 | 1 | Samsara Luggage 10-Q 2023 |
| A run of dashes between PART and ITEM (10) | 0 | 0 | None; unit tests only |
| Text with no Item heading ends Items 9, 16 and 7A (11) | 2, span only | 3, span only | Netflix 10-K 2017, NIKE 10-K 2021; fresh: Allen Organ 1998, Kroger 2019, Patriot National Bancorp 2025 |
| Financial statement schedules among those headings (11) | 0 | 0 | None; unit tests only |
| Item 16 titled as the exhibits Item keeps its text (11) | 0 | 0 | None; unit tests only |
| A section that holds the next Item keeps its text (11) | 0 | 0 | None; unit tests only |
| Wrapped cross-references are not headings (12) | 0 | 0 | None; unit tests only |
| Form names without a hyphen (13) | 0 | 0 | None among the 104; the four 10-KSB filings were added with change 15 |
| 10-KSB form type and Item list (15) | 2 | 0 | Credo Petroleum 10-KSB 2004 (Item 8A), LipidViro Tech 10-KSB 2008 (Items 8A and 8B); sample: the four 10-KSBs get 10-KSB titles |
| 10-KSB Items 8, 14 and 15 end at a block heading (15) | 0 | 0 | None; unit tests only |
| 10-KSB executive officers caption and undefined Items as in a 10-K (15) | 0 | 0 | None; unit tests only |
| 10-QSB form type, Item list and parts (15) | 0 | 0 | None; no 10-QSB is in the benchmark or the sample |
| 10-KSB Items 1A and 15, 10-QSB Part II Item 1A (15) | 0 | 0 | None; unit tests only |
| Item list chosen by the headings' fit (15) | 0 | 0 | None; no benchmark or sample filing changes list |
| Sub-item with a lettered Item's title is that Item (16) | 1 | 0 | LipidViro Tech 10-KSB 2008; fresh2: Ultimate Electronics 10-K405 1998 gains Item 7A with it |
| Sub-item with the title on the next line (16) | 0 | 0 | None; unit tests only |
| Titles that start with a form's name (16) | 0 | 0 | Sample: Interactive Brokers 10-K 2020 gains Item 16 |
| F-paged statements end the exhibits Item (17) | 1, span only | 0 | Netflix 10-K 2003; sample: Pinnacle Oil 10-K 1999, Interactive Brokers 10-K 2020 |
| F-paged statements stay when the Item's own text follows them (17) | 0 | 0 | None; unit tests only |
| Exhibits Item by printed title only after the statements Item (17) | 0 | 0 | None; unit tests only |
| Glossary heading ends Item 16 (18) | 0 | 0 | Sample: Gulf Coast Ultra Deep Royalty Trust 10-K 2020, Metropolitan Life Insurance 10-K 2026 |
| Item lines that carry on a sentence from the line above (19) | 0 | 0 | None; unit tests only. It removes 7 candidate lines in 5 benchmark and sample filings, none of which was kept as a heading |
| A line holding only a joining word links two headings (21) | 0 | 0 | fresh2: Norfolk Southern 10-K 2002 |

Text conversion rules. None of them changes a score, so the table counts the filings whose text changes:

| Rule | Tuned filings | Fresh filings (of 25) |
|---|--:|--:|
| Page headers and footers removed (5) | 15 | 2 |
| Short headers and footers that name the form, after version 0.1.1 (5) | 4 | 0 |
| Footers with the page number in the middle cell, after version 0.1.1 (5) | 0 | 0 |
| Footers printed cell by cell, after version 0.1.1 (5) | 0 | 0 |
| Form names with a non-breaking hyphen or an en dash, and roman page numbers followed by digits, after version 0.1.1 (5) | 0 | 0 |
| Lines of a row's cells never removed, after version 0.1.1 (5) | 35 | 5 |
| "$", "(", ")" and "%" cells merge only with numbers (14) | 12 | 1 |
| Merges in rows printed cell by cell (14) | 28 | 5 |
| Page labels with a prefix ("FS - 12", "S-3", "II-4", "- 4") (20) | 4 | 1 |
| "- -7-" in old text filings (20) | 1 | 0 |
| Chapter page numbers ("7-3"), "F - 3" and numbered "<Page 2>" tags (20) | 0 | 0 |
| Roman-numbered headers, "\|" and facing-page headers, and "Page 1 of M" footers that start again (20) | 0 | 0 |

The page label rules of change 20 change the text of Berkshire Hathaway 2025, MetLife 2012, AIG 2000 and Ford 2010 among the tuned filings, Science Dynamics 10-KSB 1998 ("- -7-"), Allstate 2008 in the fresh set, and ten sample filings (below).

### Regression check on unlabeled filings

Version 0.1.1 and main were also run on 150 filings that have no labels: 84 10-Ks, 4 10-KSBs, 2 10-K405s, 45 10-Qs and 15 8-Ks from 1997 to 2026, picked from EDGAR's quarterly indexes to cover banks, insurers, large and small companies, and many filing agents. Since there are no labels, this is a check for unwanted changes, not a score. The sections of 20 filings change:
- In seven 10-Ks, Item 7A, 9 or 16 now ends at an index to the financial statements, a financial statement schedule or an exhibit index, instead of running on through them (change 11): Pacific Mercantile 2004, Farmers National 2019, Prologis 2021, WEC Energy 2022, Reinsurance Group of America 2024, Energy Transfer 2025 and Metropolitan Life Insurance 2026. In Metropolitan Life Insurance 2026, and in Gulf Coast Ultra Deep Royalty Trust 2020, Item 16 also ends at a glossary heading (change 18).
- Pinnacle Oil 1999 and Interactive Brokers 2020 end the exhibits Item where financial statements with F-page numbers begin (change 17), and Interactive Brokers 2020 gains Item 16 at "ITEM 16. 10-K SUMMARY" (change 16).
- Credo Petroleum's 10-KSB 2004 gains Item 8A, and LipidViro Tech's 10-KSB 2008 gains Items 8A and 8B, printed as "Item 8(A)T." and "Item 8(B)." (changes 15 and 16). In LipidViro, Item 10 also gets back 56 table cells that version 0.1.1 removed as page numbers. All four 10-KSBs get the 10-KSB titles.
- Two 10-Qs whose body headings have dashes after the separator ("ITEM 1 -- FINANCIAL STATEMENTS", Tyco 1998 and Applied DNA Sciences 2015) now start their Items at those headings instead of at the table of contents. Lakeland Bancorp's 10-Q 1997 gains Item 2 at "PART I -- ITEM 2", and First National Bancshares' 10-Q 2007 gains Part II Item 2 at "Item 2A. Unregistered Sales of Equity Securities and Use of Proceeds".
- In five filings, page lines removed from the text shorten some Items by one line each: WEC Energy's running footer ("2021 Form 10-K", the page number, "WEC Energy Group, Inc."), "- -7-" in Science Dynamics' 10-KSB 1998, chapter page numbers such as "7-3" in Equitable Life 2002, "- 4" in Pathfinder Bancorp 2014 and "<Page 2>" tags in National Dentex's 10-Q 1997.

Each of these was checked against the filing, and each is an improvement. The text of 50 more filings changes while their sections stay within 2 percent: mostly table cell values that version 0.1.1 removed as page numbers come back, "$", "%" and ")" cells merge with their numbers or stay apart, and page labels such as "F - 3" and "S-3" are removed (Presidential Life 2004, Platinum Underwriters 2006, Freeport-McMoRan 2015, Third Point Reinsurance 2017). Change 21 changes none of the 150. The list of filings and the comparison script are not part of this repository.

### Speed

These are per-filing times on the 104 tuned filings of the time (before the four 10-KSB filings were added), each parsed 5 times with the fastest run kept. The first table is from the runs that produced the initial commit and version 0.1.1 results above:

| | Initial commit | Version 0.1.1 |
|---|--:|--:|
| Parse plus split, mean | 30.7 ms | 30.6 ms |
| Parse plus split, median | 14.6 ms | 14.7 ms |
| Split alone, mean | 0.36 ms | 0.35 ms |
| Split alone, median | 0.20 ms | 0.25 ms |

The second table compares version 0.1.1 with main before changes 15 to 21, in four rounds, alternating between the two. It shows the mean of the four rounds, with the range of the means in parentheses. The two tables were measured at different times and their absolute times differ, so compare within a table:

| | Version 0.1.1 | Main before change 15 |
|---|--:|--:|
| Parse plus split, mean | 32.8 ms (32.2 to 33.8) | 33.6 ms (33.1 to 33.9) |
| Parse plus split, median | 15.6 ms | 16.2 ms |
| Split alone, mean | 0.37 ms | 0.38 ms |
| Split alone, median | 0.29 ms | 0.29 ms |
| Split alone, slowest filing | 1.5 to 1.6 ms | 1.5 to 1.6 ms |

Main's mean was about 2 percent higher, within the spread between rounds. A second check timed `document_to_text` of both versions on each of the 129 labeled documents of the time in turn, best of 5 runs per document. In two such runs the totals differed by under 1 percent, once in each direction (0.994 and 1.005 times version 0.1.1's). `document_to_text_and_tables` returns the same text for all 129 and takes 1 to 2 percent longer than `document_to_text`. Most of the time is text conversion.

Changes 15 to 21 were not timed back to back. In single runs of the benchmark (one timing per filing) on the same 104 filings, the median split time went from about 0.34 ms before them to about 0.64 ms after, and the mean from about 0.5 ms to about 0.75 ms; the increase came with the fixes made in review to changes 15 to 17. The median parse time stayed near 20 ms, so parse plus split changes by under 1 percent.

## Other tools

`bench/external/run.sh` runs edgar-crawler and edgartools on the same filings and scores their output:

```sh
export PARSEC_EDGAR_USER_AGENT="Your Name you@example.com"
bench/external/run.sh            # development, held-out and blind sets
bench/external/run.sh fresh2     # one set: dev, holdout, blind, fresh or fresh2
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
  - For some filings, edgartools looks up the filing's index page, and the block stops that request with the error "network access is disabled in this benchmark". In the latest runs, of the development set and fresh2, this happened on one development filing, Science Dynamics' 10-KSB 1998, and on the 4 fresh2 filings from before 2001. Earlier runs saw it on 8 development filings from 1997 to 2000 (seven plain-text filings and Walmart's 2000 10-K, which is HTML) and on the 4 plain-text filings of the fresh set; run with network access on those 12, edgartools made one or two requests per filing and returned no Items, so the score was the same either way.
  - 10-K405 and 10-KSB filings are read as 10-Ks, because edgartools' 10-K class accepts only 10-K and 10-K/A. edgar-crawler is given them as 10-Ks too.

The other tools' scores on the development set and on fresh2 were measured with main. Those on the held-out, blind and fresh sets were measured before version 0.1.1 was released and were not repeated. The labels and the filings of those three sets are the same; what changed is this library's text, in which `score-external` places their sections (changes 5, 14 and 20).

### Placing another tool's text

`score-external` finds each section's text in our filing text, comparing letters and digits only. A section starts at the line where its opening characters appear and the following text agrees best, which tells a body heading from its table-of-contents entry. Formatting conventions that do not move an Item's start are not counted as errors:
- a Part line or page header before the heading;
- a heading left out of the section text;
- a page footer inside the text;
- a body too short to find, such as "None.", when it equals the labeled Item's body.

`predict DIR` writes this extractor's own sections in the same format. Scoring them through `score-external` gives F1 1.000 on the four older sets, with span 0.998 against the direct 1.000 on the development set. On fresh2 it gives F1 0.993 and span 0.901, against the direct 0.990 and 0.900: it places Superior Well Services' Item 7.01, which parsec-edgar takes from the cover page's list and which holds only that line, at the body heading that starts with the same words, and scores it as a hit. A section that holds only its heading line can be placed at a later line with the same words this way.

### Scores

Fresh2 at its first scoring is the newest fair comparison, since no tool had been tuned on it:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-Q F1 | 8-K F1 |
|---|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar, first scoring | 0.990 | 0.985 | 0.988 | 0.895 | 0.994 | 0.962 | 0.900 |
| parsec-edgar, version 0.1.1 | 0.990 | 0.988 | 0.989 | 0.900 | 0.996 | 0.962 | 0.900 |
| edgar-crawler | 0.985 | 0.975 | 0.980 | 0.917 | 0.985 | 0.941 | 1.000 |
| edgartools | 0.917 | 0.715 | 0.803 | 0.624 | 0.784 | 0.905 | 0.900 |
| parsec-edgar, main (partly in-sample) | 0.990 | 0.990 | 0.990 | 0.900 | 0.997 | 0.962 | 0.900 |

The 10-K column includes the 10-K405. edgar-crawler's span accuracy is higher than parsec-edgar's, at the first scoring and after the fix, and two filings account for it. These are the filings where the span accuracy of parsec-edgar and edgar-crawler differs by more than 0.02:

| Filing | parsec-edgar, first scoring | parsec-edgar, main | edgar-crawler | edgartools |
|---|--:|--:|--:|--:|
| Superior Well Services 8-K 2010 | 0.000 | 0.000 | 1.000 | 0.017 |
| Harleysville Savings Financial 10-Q 2002 | 0.001 | 0.001 | 0.123 | 0.000 |
| Norfolk Southern 10-K 2002 | 0.856 | 0.998 | 0.987 | 0.000 |
| ITT 10-K 2015 | 0.985 | 0.985 | 0.627 | 0.631 |
| TrustCo Bank 10-K 2023 | 1.000 | 1.000 | 0.912 | 0.996 |
| Golden State Water 10-K 2000 | 1.000 | 1.000 | 0.967 | 0.000 |
| LivePerson 10-Q 2026 | 1.000 | 1.000 | 0.965 | 0.997 |
| EMCORE 10-K 2008 | 1.000 | 1.000 | 0.972 | 0.863 |

On the other 28 filings, without Superior Well Services and Harleysville Savings, the mean span accuracy is 0.959 for parsec-edgar at its first scoring (0.964 for main), 0.942 for edgar-crawler and 0.668 for edgartools. On fresh2:
- **edgar-crawler** finds Superior Well Services' Item 7.01 at its body heading, and both labeled Items of Harleysville Savings' 10-Q. It runs ITT's Item 15 on through the financial statements printed after it, starts Part I Item 1 of three 10-Qs at an earlier line (USA Today 2016, American Woodmark 2021, LivePerson 2026), and misses Ultimate Electronics' Item 7A and Encision's Items 9A and 9B.
- **edgartools** raises "network access is disabled in this benchmark" on the 4 filings from before 2001, all plain text (CFSB Bancorp 1997, American Filtrona 1997, Ultimate Electronics 1998, Golden State Water 2000), and returns no Items for two more plain-text filings, Norfolk Southern 2002 and Harleysville Savings 2002. All six score 0.

The fresh set at its first scoring was the fair comparison before fresh2:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-Q F1 | 8-K F1 | Time per filing |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar, first scoring (version 0.1.1) | 1.000 | 0.989 | 0.994 | 0.935 | 0.998 | 0.958 | 1.000 | 22 ms mean, 12 ms median |
| parsec-edgar (initial commit) | 0.949 | 0.905 | 0.927 | 0.756 | 0.973 | 0.541 | 1.000 | |
| edgar-crawler | 0.963 | 0.960 | 0.961 | 0.843 | 0.961 | 0.959 | 1.000 | 0.38 s mean |
| edgartools | 0.977 | 0.834 | 0.900 | 0.705 | 0.893 | 0.932 | 1.000 | 0.64 s mean, 0.35 s median |
| parsec-edgar (main, in-sample) | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 22 ms mean, 13 ms median |

On the fresh set:
- **edgar-crawler** takes the list of Item headings that McDonald's 2003 10-K prints again after the signatures as the headings of Items 5 to 15. FedEx's 10-Q has no Part I Item 1 heading in the body, and edgar-crawler starts that Item at a table-of-contents line. In Abbott's 10-Q it misses Item 2.
- **edgartools** returns no Items for the 4 plain-text filings. It misses some Items in five 10-Ks (McDonald's, Allstate, US Bancorp, Kroger, Research Frontiers).

The parsec-edgar times in this table are from the back-to-back runs under [Speed](#speed), made before changes 15 to 21; the initial commit's are in the first table there.

These are the 108 filings of the development, held-out and blind sets, combined. parsec-edgar's scores here are in-sample:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-KSB F1 | 10-Q F1 | 8-K F1 |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar (main, in-sample) | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| edgar-crawler | 0.991 | 0.987 | 0.989 | 0.917 | 0.997 | 0.973 | 0.948 | 1.000 |
| edgartools | 0.950 | 0.796 | 0.866 | 0.654 | 0.871 | 0.154 | 0.926 | 1.000 |

On the 104 of them without the 10-KSBs, version 0.1.1 scores F1 1.000 with span 0.993 (in-sample), and the initial commit F1 0.902 with span 0.775.

By label set:

| Tool | Development: F1 | Development: span | Held out: F1 | Held out: span | Blind: F1 | Blind: span | Fresh: F1 | Fresh: span |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar, first scoring | | | 0.955 | 0.875 | 0.989 | 0.997 | 0.994 | 0.935 |
| parsec-edgar, version 0.1.1 | 1.000 | 0.990 | 1.000 | 1.000 | 1.000 | 1.000 | 0.994 | 0.935 |
| parsec-edgar, main | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| edgar-crawler | 0.993 | 0.911 | 0.987 | 0.928 | 0.972 | 0.925 | 0.961 | 0.843 |
| edgartools | 0.849 | 0.606 | 0.884 | 0.675 | 0.928 | 0.810 | 0.900 | 0.705 |

The development columns are for 69 filings, except version 0.1.1's, which are for the 65 without the 10-KSBs.

How to read these numbers:
- **Tuning.** parsec-edgar was tuned on the four older sets, and on fresh2 by one fix after its first scoring. The other tools were not tuned on any of them. Compare the tools on the first scorings of the fresh2, fresh, blind and held-out sets. The development set has no such score, because the rules were written while looking at it.
- **edgar-crawler** finds Item starts nearly as well, from a hard-coded list of each form's Items, and on fresh2 its span accuracy is higher than parsec-edgar's (above).
  - It loses span accuracy on the last Item, which runs on into attached annual reports (span 0.136 on the `annual-report-attached` filings).
  - Some 10-Q Items start at the table of contents or at a "(continued)" page header.
- **edgartools** returns no Items for most filings without an HTML document, so it scores 0.043 F1 on the 18 `plain-text` filings of the development, held-out and blind sets. It also misses Items in many exhibit-13 and small-company 10-Ks, and in the 10-KSBs.
- **Time.** The Python times include reading the filing from disk, and edgartools' times include parsing the whole submission. The parsec-edgar times start after the cached document has been read.
  - edgar-crawler's time is its whole run, minus its start-up time, divided by the number of filings.
  - edgartools' time is measured per filing.
