# Section-splitting benchmark

This benchmark measures how well `extract_sections` finds the Items of 10-K, 10-Q and 8-K filings, and how long text conversion and splitting take. It compares the output with hand labels for 129 filings filed from 1997 to 2026. The same labels also score [edgar-crawler](https://github.com/nlpaueb/edgar-crawler) and [edgartools](https://github.com/dgunning/edgartools).

## Accuracy in brief

The extractor has now been tuned on all four label sets, so its current scores are in-sample. The numbers that show how it does on filings it has not seen are each set's first scoring, made before any of that set's misses were fixed. The fresh set was the last one held back: it was scored once with version 0.1.1, and its misses were fixed after that release. No label set is untouched now; measuring the current rules out of sample would take a new set, labeled and committed before it is scored.

| Label set | Filings | First scoring | Item-start F1 | Span accuracy |
|---|--:|---|--:|--:|
| `bench/fresh` | 25 | Labels committed first, then scored once (version 0.1.1). Its misses were fixed afterward. | 0.994 | 0.935 |
| `bench/blind` | 19 | Labels committed first, then scored once. Its two misses were fixed afterward. | 0.989 | 0.997 |
| `bench/holdout` | 20 | Scored after the first round of changes. Its misses were fixed afterward. | 0.955 | 0.875 |
| All 129 filings, in-sample | 129 | After tuning on them | 1.000 | 0.997 |

At its first scoring the fresh set had precision 1.000 and recall 0.989: 345 of the 349 labeled Item starts were found, and no Item was predicted where none is labeled. On the same 25 filings, edgar-crawler scores F1 0.961 with span 0.843, edgartools 0.900 with span 0.705, and this library at its initial commit 0.927 with span 0.756. Compare those with the first scoring, not with the current in-sample 1.000.

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
| `bench/fresh` | 25 | 349 | Labeled and committed after changes 1 to 8 below, then scored once (F1 0.994, span 0.935; version 0.1.1 scores the same). Its misses were fixed after version 0.1.1, so it is no longer untouched either. |

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

Three versions are compared: the library at its initial commit, version 0.1.1, and this branch. All scores are from release builds.

| Label set | Initial commit: P / R / F1 | Initial commit: span | 0.1.1: P / R / F1 | 0.1.1: span | This branch: P / R / F1 | This branch: span |
|---|---|--:|---|--:|---|--:|
| Fresh (25) | 0.949 / 0.905 / 0.927 | 0.756 | 1.000 / 0.989 / 0.994 | 0.935 | 1.000 / 1.000 / 1.000 | 1.000 |
| Development (65) | 0.946 / 0.885 / 0.914 | 0.807 | 1.000 / 1.000 / 1.000 | 0.990 | 1.000 / 1.000 / 1.000 | 0.995 |
| Held out (20) | 0.957 / 0.880 / 0.917 | 0.753 | 1.000 / 1.000 / 1.000 | 1.000 | 1.000 / 1.000 / 1.000 | 1.000 |
| Blind (19) | 0.799 / 0.839 / 0.818 | 0.686 | 1.000 / 1.000 / 1.000 | 1.000 | 1.000 / 1.000 / 1.000 | 1.000 |
| The 104 tuned filings | 0.926 / 0.878 / 0.902 | 0.775 | 1.000 / 1.000 / 1.000 | 0.993 | 1.000 / 1.000 / 1.000 | 0.997 |

Version 0.1.1's fresh row is that set's first scoring, out of sample; its other rows are in-sample. Every row for this branch is in-sample.

### The fresh set

| Group | Filings | Initial commit: F1 | Initial commit: span | 0.1.1 (first scoring): F1 | 0.1.1: span | This branch: F1 | This branch: span |
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

### Development set, by group

| Group | Initial commit: F1 | Initial commit: span | 0.1.1: F1 | 0.1.1: span | This branch: F1 | This branch: span |
|---|--:|--:|--:|--:|--:|--:|
| 10-K 1997 to 2001 | 0.683 | 0.551 | 1.000 | 0.998 | 1.000 | 0.998 |
| 10-K 2002 to 2008 | 0.959 | 0.878 | 1.000 | 0.968 | 1.000 | 0.968 |
| 10-K 2009 to 2015 | 1.000 | 0.898 | 1.000 | 1.000 | 1.000 | 1.000 |
| 10-K 2016 to 2020 | 0.955 | 0.865 | 1.000 | 0.962 | 1.000 | 1.000 |
| 10-K 2021 to 2026 | 0.944 | 0.906 | 1.000 | 1.000 | 1.000 | 1.000 |
| 10-Q | 0.767 | 0.616 | 1.000 | 1.000 | 1.000 | 1.000 |
| 8-K | 1.000 | 0.950 | 1.000 | 1.000 | 1.000 | 1.000 |
| `plain-text` | 0.657 | 0.502 | 1.000 | 0.998 | 1.000 | 0.998 |
| `annual-report-attached` | 0.916 | 0.107 | 1.000 | 0.996 | 1.000 | 0.996 |
| `running-headers` | 0.751 | 0.995 | 1.000 | 1.000 | 1.000 | 1.000 |

Every Item starts on its labeled line in all 129 filings. These filings still lose span:
- **Netflix 10-K 2003 (span 0.684).** The financial statement pages follow Item 15 with no heading or table of contents of their own. The labels leave them out of every Item, and the extractor keeps them in Item 15 (68,270 characters against 4,910 labeled), because Item 15 is where a 10-K lists its financial statements, and many filings print them there.
- **JPMorgan Chase 10-K405 2001 (span 0.982).** The attached annual report starts with a "FINANCIAL TABLE OF CONTENTS" line and three paragraphs of notes, then the heading "MANAGEMENT'S DISCUSSION AND ANALYSIS:" and the page-numbered contents. Item 14 ends at that heading, so the first line and the notes, 1,455 characters, stay in Item 14.
- **Exxon Mobil 10-K 2005 (span 0.998).** Item 15 ends at the heading of the attached financial section. Two "Index to Financial Statements" page links and a "[THIS PAGE INTENTIONALLY LEFT BLANK]" line just above it, 97 characters, stay in Item 15.
- **Netflix 10-K 2017 (span 0.9999).** Version 0.1.1 scored it 0.654: Item 16, "Form 10-K Summary", ran on through the financial statements. Item 16 now ends at the index to the financial statements (change 11), and the company name printed as a page header above that index, 14 characters, stays in it.
- **Allen Organ 10-K 1998** in the fresh set (span 0.998), described above.

### The changes

Changes 1 to 8 were made before the fresh set was labeled. Change 9 came after its first scoring and did not change it. Version 0.1.1 has changes 1 to 9. The parts marked "After version 0.1.1", and changes 10 to 14, are on this branch.

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
11. **Text with no Item heading** (after version 0.1.1). A line that opens such text ends 10-K Items 9 and 16, and Item 7A when the filing has no Item 8 heading: an auditors' report, an index to the financial statements, a financial statement schedule ("SCHEDULE I - CONDENSED PARENT COMPANY FINANCIAL STATEMENTS"), an exhibit index or an accountants' consent. The line must start with a capital letter. Other Items keep these headings as their own text. An Item 16 titled as the exhibits Item keeps its statements and exhibit index, and the rule does not cut a section that also holds the next Item ("Items 9 and 9A.", or "ITEM 9(A). CONTROLS AND PROCEDURES", which reads as Item 9).
12. **Wrapped cross-references** (after version 0.1.1). In a 10-Q, a line that starts with "Item" and a Part I title but runs on as a sentence ("Item 1. Financial Statements, of this report", "Item 2. Management's Discussion and Analysis, under ...") is a hard-wrapped cross-reference, not a heading, and no longer cuts the Item it sits in short. Only a reference word such as "of this report", or a comma followed by a lowercase word, makes it one; a heading that goes on with words of its own stays.
13. **Form names without a hyphen** (after version 0.1.1). `FormKind::from_form` accepts EDGAR's index spellings 10KSB, 10KSB40, 10KT405, 10QSB and the like, so 10-KSB and 10-QSB filings are split. No benchmark filing uses these spellings.
14. **Table cells in the text** (after version 0.1.1). A "$" or "(" cell joins the cell after it only when that cell starts with a digit, a dash or a ".", a "%" cell joins the cell before it only when that cell ends with a digit or a dash, and a ")" cell joins a cell that leaves a "(" open. Header cells such as "Amount" keep the "%" column beside them as a cell of their own, and "$" next to "N/A" stays apart. The same merges now happen in rows printed cell by cell. These change only the text inside Items.

The tests in `crates/parsec-edgar/src/sections.rs` also cover layouts where each special rule must not fire: a short attached document, a signature sentence without "duly caused", sub-item references, undefined Items mentioned in the text, short Part-marked lines, titled sub-items other than executive officers, lettered headings beside an Item that was found, Part I headings that go on with words of their own, and auditors' reports and exhibit indexes inside other Items. The tests in `crates/parsec-edgar/src/html.rs` cover page lines and table cells that only look like headers or footers.

### Filings behind each rule

Each rule was turned off alone on this branch, and the tables count the filings whose score changes. Turning off any rule also fails at least one unit test. A rule with a count of 0 or 1 was written for one layout; it rests on its unit tests and on the filings named here more than on the benchmark as a whole.

Section rules:

| Rule | Tuned filings (of 104) | Fresh filings (of 25) | Filings that change |
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
| Form names without a hyphen (13) | 0 | 0 | None; no benchmark filing uses them |

Text conversion rules. None of them changes a score, so the table counts the filings whose text changes:

| Rule | Tuned filings (of 104) | Fresh filings (of 25) |
|---|--:|--:|
| Page headers and footers removed (5) | 15 | 2 |
| Short headers and footers that name the form, after version 0.1.1 (5) | 4 | 0 |
| Footers with the page number in the middle cell, after version 0.1.1 (5) | 0 | 0 |
| Footers printed cell by cell, after version 0.1.1 (5) | 0 | 0 |
| Form names with a non-breaking hyphen or an en dash, and roman page numbers followed by digits, after version 0.1.1 (5) | 0 | 0 |
| Lines of a row's cells never removed, after version 0.1.1 (5) | 35 | 5 |
| "$", "(", ")" and "%" cells merge only with numbers (14) | 12 | 1 |
| Merges in rows printed cell by cell (14) | 28 | 5 |

### Regression check on unlabeled filings

Before the changes after version 0.1.1 were merged, version 0.1.1 and this branch were also run on 150 filings that have no labels: 84 10-Ks, 4 10-KSBs, 2 10-K405s, 45 10-Qs and 15 8-Ks from 1997 to 2026, picked from EDGAR's quarterly indexes to cover banks, insurers, large and small companies, and many filing agents. Since there are no labels, this is a check for unwanted changes, not a score. The sections of 12 filings change:
- In seven 10-Ks, Item 7A, 9 or 16 now ends at an index to the financial statements, a financial statement schedule or an exhibit index, instead of running on through them (Pacific Mercantile 2004, Farmers National 2019, Prologis 2021, WEC Energy 2022, Reinsurance Group of America 2024, Energy Transfer 2025, Metropolitan Life Insurance 2026).
- Two 10-Qs whose body headings have dashes after the separator ("ITEM 1 -- FINANCIAL STATEMENTS", Tyco 1998 and Applied DNA Sciences 2015) now start their Items at those headings instead of at the table of contents. Lakeland Bancorp's 10-Q 1997 gains Item 2 at "PART I -- ITEM 2", and First National Bancshares' 10-Q 2007 gains Part II Item 2 at "Item 2A. Unregistered Sales of Equity Securities and Use of Proceeds".
- WEC Energy's running footer ("2021 Form 10-K", the page number, "WEC Energy Group, Inc.") is now removed, which shortens six of its Items by one line, and in LipidViro's 10-KSB 2008, Item 10 gets back 56 table cells that were removed as page numbers.

Each of these was checked against the filing, and each is an improvement. The text of 49 more filings changes while their sections stay within 2 percent: mostly table cell values that version 0.1.1 removed as page numbers come back, and "$", "%" and ")" cells merge with their numbers or stay apart. The list of filings and the comparison script are not part of this repository.

### Speed

These are per-filing times on the 104 tuned filings, each parsed 5 times with the fastest run kept. The first table is from the runs that produced the initial commit and version 0.1.1 results above:

| | Initial commit | Version 0.1.1 |
|---|--:|--:|
| Parse plus split, mean | 30.7 ms | 30.6 ms |
| Parse plus split, median | 14.6 ms | 14.7 ms |
| Split alone, mean | 0.36 ms | 0.35 ms |
| Split alone, median | 0.20 ms | 0.25 ms |

The second table compares version 0.1.1 with this branch in four rounds, alternating between the two. It shows the mean of the four rounds, with the range of the means in parentheses. The two tables were measured at different times and their absolute times differ, so compare within a table:

| | Version 0.1.1 | This branch |
|---|--:|--:|
| Parse plus split, mean | 32.8 ms (32.2 to 33.8) | 33.6 ms (33.1 to 33.9) |
| Parse plus split, median | 15.6 ms | 16.2 ms |
| Split alone, mean | 0.37 ms | 0.38 ms |
| Split alone, median | 0.29 ms | 0.29 ms |
| Split alone, slowest filing | 1.5 to 1.6 ms | 1.5 to 1.6 ms |

The branch's mean is about 2 percent higher, within the spread between rounds. A second check timed `document_to_text` of both versions on each of the 129 labeled documents in turn, best of 5 runs per document. In two such runs the totals differed by under 1 percent, once in each direction (0.994 and 1.005 times version 0.1.1's). `document_to_text_and_tables` returns the same text for all 129 and takes 1 to 2 percent longer than `document_to_text`. Splitting stays under 1.6 ms per filing on this branch; most of the time is text conversion.

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

The other tools' scores below were measured before version 0.1.1 was released and were not repeated for this branch. The labels and the filings are the same; what changed is this library's text, in which `score-external` places their sections (changes 5 and 14).

### Placing another tool's text

`score-external` finds each section's text in our filing text, comparing letters and digits only. A section starts at the line where its opening characters appear and the following text agrees best, which tells a body heading from its table-of-contents entry. Formatting conventions that do not move an Item's start are not counted as errors:
- a Part line or page header before the heading;
- a heading left out of the section text;
- a page footer inside the text;
- a body too short to find, such as "None.", when it equals the labeled Item's body.

`predict DIR` writes this extractor's own sections in the same format. Scoring them through `score-external` reproduces the direct scores: F1 1.000 on all four sets, and span 0.993 against 0.995 on the development set.

### Scores

The fresh set at its first scoring is the fair comparison, since no tool had been tuned on it:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-Q F1 | 8-K F1 | Time per filing |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar, first scoring (version 0.1.1) | 1.000 | 0.989 | 0.994 | 0.935 | 0.998 | 0.958 | 1.000 | 22 ms mean, 12 ms median |
| parsec-edgar (initial commit) | 0.949 | 0.905 | 0.927 | 0.756 | 0.973 | 0.541 | 1.000 | |
| edgar-crawler | 0.963 | 0.960 | 0.961 | 0.843 | 0.961 | 0.959 | 1.000 | 0.38 s mean |
| edgartools | 0.977 | 0.834 | 0.900 | 0.705 | 0.893 | 0.932 | 1.000 | 0.64 s mean, 0.35 s median |
| parsec-edgar (this branch, in-sample) | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 22 ms mean, 13 ms median |

On the fresh set:
- **edgar-crawler** takes the list of Item headings that McDonald's 2003 10-K prints again after the signatures as the headings of Items 5 to 15. FedEx's 10-Q has no Part I Item 1 heading in the body, and edgar-crawler starts that Item at a table-of-contents line. In Abbott's 10-Q it misses Item 2.
- **edgartools** returns no Items for the 4 plain-text filings. It misses some Items in five 10-Ks (McDonald's, Allstate, US Bancorp, Kroger, Research Frontiers).

These are the 104 tuned filings, with the three sets combined. parsec-edgar's scores here are in-sample:

| Tool | Precision | Recall | F1 | Span acc | 10-K F1 | 10-Q F1 | 8-K F1 | Time per filing |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar (this branch, in-sample) | 1.000 | 1.000 | 1.000 | 0.997 | 1.000 | 1.000 | 1.000 | 34 ms mean, 16 ms median |
| parsec-edgar (version 0.1.1, in-sample) | 1.000 | 1.000 | 1.000 | 0.993 | 1.000 | 1.000 | 1.000 | 33 ms mean, 16 ms median |
| parsec-edgar (initial commit) | 0.926 | 0.878 | 0.902 | 0.775 | 0.936 | 0.708 | 1.000 | |
| edgar-crawler | 0.990 | 0.989 | 0.990 | 0.914 | 0.997 | 0.948 | 1.000 | 0.43 s mean |
| edgartools | 0.953 | 0.826 | 0.885 | 0.677 | 0.871 | 0.926 | 1.000 | 1.07 s mean, 0.54 s median |

The parsec-edgar times in both tables are from the back-to-back runs under [Speed](#speed); the initial commit's are in the first table there.

By label set:

| Tool | Development: F1 | Development: span | Held out: F1 | Held out: span | Blind: F1 | Blind: span | Fresh: F1 | Fresh: span |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| parsec-edgar, first scoring | | | 0.955 | 0.875 | 0.989 | 0.997 | 0.994 | 0.935 |
| parsec-edgar, version 0.1.1 | 1.000 | 0.990 | 1.000 | 1.000 | 1.000 | 1.000 | 0.994 | 0.935 |
| parsec-edgar, this branch | 1.000 | 0.995 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| edgar-crawler | 0.994 | 0.907 | 0.987 | 0.928 | 0.972 | 0.925 | 0.961 | 0.843 |
| edgartools | 0.876 | 0.639 | 0.884 | 0.675 | 0.928 | 0.810 | 0.900 | 0.705 |

How to read these numbers:
- **Tuning.** parsec-edgar was tuned on all four sets, and the other tools were not. Compare the tools on the first scorings of the fresh, blind and held-out sets. The development set has no such score, because the rules were written while looking at it.
- **edgar-crawler** finds Item starts nearly as well, from a hard-coded list of each form's Items.
  - It loses span accuracy on the last Item, which runs on into attached annual reports (span 0.136 on the `annual-report-attached` filings).
  - Some 10-Q Items start at the table of contents or at a "(continued)" page header.
- **edgartools** returns no Items for filings without an HTML document, so it scores 0.015 F1 on the `plain-text` filings. It also misses Items in many exhibit-13 and small-company 10-Ks.
- **Time.** The Python times include reading the filing from disk, and edgartools' times include parsing the whole submission. The parsec-edgar times start after the cached document has been read.
  - edgar-crawler's time is its whole run, minus its start-up time, divided by the number of filings.
  - edgartools' time is measured per filing.
