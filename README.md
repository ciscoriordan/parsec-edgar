# parsec-edgar

Fast SEC EDGAR client and filing parser written in Rust, with Python bindings and a command-line tool.

- Look up companies by ticker or CIK
- List a company's filings with form and date filters, including full history back to 1994
- Convert filing HTML (or old plain-text submissions) to clean text, with table rows kept on one line and `$` signs merged into their numbers
- Split 10-K, 10-Q and 8-K filings into Item sections (Item 1A Risk Factors, Item 7 MD&A, 8-K Item 2.02, and so on)
- Stays within the SEC fair-access limit (8 requests per second by default, 10 at most) and caches archive documents on disk

Status: early development (0.1.0). Not yet published to crates.io or PyPI.

## The SEC User-Agent requirement

The SEC requires every request to identify the caller with a name and contact email. Pass it explicitly, or set `PARSEC_EDGAR_USER_AGENT` (the `EDGAR_IDENTITY` variable used by edgartools also works):

```sh
export PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com"
```

## Command line

```sh
cargo install --path crates/parsec-edgar-cli

parsec-edgar company AAPL
parsec-edgar filings AAPL --form 10-K --limit 5
parsec-edgar filings MSFT --form 10-K --since 1994 --until 2002   # reaches into older history pages
parsec-edgar text AAPL --form 10-K                                 # clean text of the latest 10-K
parsec-edgar sections AAPL --form 10-K --item 1A --item 7          # JSON
parsec-edgar sections MSFT --form 10-Q --no-text                   # ids, titles and offsets only
parsec-edgar sections AAPL --accession 0000320193-26-000026
```

Archive documents are cached in `~/.cache/parsec-edgar` (override with `--cache-dir` or `PARSEC_EDGAR_CACHE`, disable with `--no-cache`). Filing lists and the ticker file are always fetched fresh.

## Python

```sh
cd python && maturin develop --release
```

```python
import parsec_edgar as pe

client = pe.Client("Example Corp admin@example.com", cache_dir="~/.cache/parsec-edgar")
client.company("BRK.B")                     # {'cik': 1067983, 'ticker': 'BRK-B', ...}
filings = client.filings("MSFT", form=["10-K", "10-Q"], since="2024")
latest = client.latest("AAPL", "10-K")      # a filing dict
text = client.text(latest)                  # clean text
for s in client.sections(latest):
    print(s["id"], s["title"], len(s["text"]))

pe.html_to_text(html)                       # parse documents you already have
pe.extract_sections(text, "10-K")
```

Lookup failures raise `LookupError`, bad arguments raise `ValueError`, and network or HTTP failures raise `parsec_edgar.EdgarError`. Network calls release the GIL, so one client can be shared across threads; it stays within its rate limit.

## Rust

```rust
use parsec_edgar::{Client, FilingQuery};

let client = Client::builder("Example Corp admin@example.com")
    .cache_dir(Some("cache".into()))
    .build()?;
let filing = client.latest("AAPL", "10-K")?;
for s in client.filing_sections(&filing)? {
    println!("{} {} ({} bytes)", s.id, s.title, s.text.len());
}
let recent = client.filings("NVDA", &FilingQuery::forms(&["8-K"]).since("2026-01").limit(10))?;
```

## Bulk crawling

`parsec-edgar crawl` finds filings by form type and date, downloads them, and writes each one as JSON with its metadata and Item sections. It does in one step what edgar-crawler's `download_filings.py` and `extract_items.py` do in two.

```sh
# Every 8-K filed on two days, across all companies
parsec-edgar crawl --form 8-K --since 2026-11-05 --until 2026-11-06 --out 8k

# Whole years, or some quarters of them
parsec-edgar crawl --form 10-K --form 10-K/A --year 2020-2024 --out 10k
parsec-edgar crawl --form 10-Q --year 2024 --quarter 2,3 --out 10q --jsonl

# A list of companies (tickers or CIKs), through the submissions API
parsec-edgar crawl --form 10-K --company AAPL --company 789019 --year 2015-2025 --out big-tech --raw
parsec-edgar crawl --form 10-K --companies-file sp500.txt --out sp500    # no dates: whole history

parsec-edgar crawl --form 10-K --year 2025 --out 10k --dry-run           # count only, download nothing
```

Form types match exactly, so `--form 10-K` leaves out `10-K/A` and `10-K405`; repeat `--form` for each one you want. `--item 1A --item 7` keeps only those sections, `--limit N` stops after the first N filings, and `--threads N` (default 8) sets the number of workers. The workers share one client, so together they stay within `--rps`, while documents are parsed in parallel.

Output in the `--out` directory:

- `{cik}_{form}_{filing date}_{accession}.json` for each filing, or one line per filing in `filings.jsonl` with `--jsonl`
- `raw/{same name}.htm` (or `.txt`), the primary document as served, with `--raw`
- `failures.jsonl`, one line per filing that failed, with the URL and the error

Running the same command again resumes the crawl: filings that already have output are skipped, and failed ones are tried again. Progress and a final summary (ok, failed, skipped, filings per minute, requests per filing) go to stderr, and the exit status is 1 when anything failed. The SEC answers 403 to clients that go over its rate limit, so on a 403 every worker pauses for a minute, and five in a row stop the crawl so it can be resumed later.

Each record has these fields:

| Field | edgar-crawler field | Contents |
|---|---|---|
| `cik`, `company`, `filing_date` | same | |
| `form` | `filing_type` | |
| `report_date` | `period_of_report` | |
| `acceptance_datetime` | | UTC, as in the submissions API |
| `accession_number`, `items` | | `items` lists 8-K item numbers |
| `sic`, `sic_description` | `sic` | |
| `state_of_incorporation` | `state_of_inc` | |
| `state_location` | `state_location` | State or country code of the business address |
| `fiscal_year_end` | `fiscal_year_end` | `MMDD` |
| `index_url` | `filing_html_index` | The filing's `-index.htm` page |
| `primary_document`, `primary_document_url` | `htm_filing_link` | |
| `full_submission_url` | `complete_text_filing_link` | |
| `raw_file` | `filename` | With `--raw` |
| `sections` | `item_1`, `item_1A`, ... | Section text keyed by [section id](#section-ids), in filing order |
| `section_list` | | `id`, `part`, `item`, `title`, `heading`, `start`, `end` and `chars` of each section |
| `text` | | The whole text, for forms without Item sections |

### How filings are found

Without `--company`, the crawl reads the EDGAR full index, which lists every filing of every company, one file per quarter. It reads `master.gz`: it is pipe-delimited, while `form.idx` and `company.idx` use fixed-width columns that long company names can break, and at about 3 MB it is the smallest download (`master.idx` is 24 MB, `form.idx` 42 MB). A filing with several filers has one row per filer and is crawled once. Finished quarters are cached; the current quarter is downloaded again on each run, since the SEC adds the previous business day's filings every night.

The index gives only the path of each filing's complete submission file (all documents in one SGML file, often 10 MB or more for a 10-K). The crawl therefore reads each filing's `-index.htm` page (10 to 40 KB), which gives the primary document (the row whose type equals the form), the report period, acceptance time, 8-K items, and the filer's SIC code, state of incorporation, business address and fiscal year end as of the filing date. That is two requests per filing. The other option, the submissions API once per company, saves nothing when most filings come from different companies, as in a day of 8-Ks or a year of 10-Ks, and it costs a 100 KB to 1 MB file per company plus extra pages for filings older than the latest thousand.

With `--company` or `--companies-file`, filings come from the submissions API (`filings()`): one request per company, plus older pages when the date range reaches back, then one request per filing for its document. The company profile (SIC code, state of incorporation and so on, current values) costs one more request per company and is fetched once per crawl.

Measured in November 2026 at `--rps 3`, with 8 threads:

| Crawl | Filings | Time | Filings per minute | Requests per filing | Failed |
|---|---|---|---|---|---|
| Every 8-K filed on 2026-11-05 and 2026-11-06 (full index) | 1,009 | 11.0 min | 91.6 | 1.96 | 0 |
| The first 200 10-Ks of 2025 in index order (full index) | 200 | 2.2 min | 90.1 | 2.00 | 0 |
| 10-Ks of 2016 to 2025 for 7 companies (`--company`) | 60 | 45 s | 170 | 1.00, plus 70 to find them | 0 |

Both full-index crawls ran at the request limit (3 per second is 90 filings a minute at 2 requests each). Parsing is far from the limit: with every document already cached, the same 200 10-Ks (788 MB of HTML) were parsed in 1.8 seconds. Running the 10-K crawl again skipped all 200 with no requests. In the company crawl, 58 of the 70 discovery requests were JPMorgan's older submissions pages: it files about 26,000 documents a year, so its recent block covers only one year. For heavy filers over many years, the full index is cheaper.

A full year of 10-Ks, at the default 8 requests per second: 2025 has 6,431 10-K filings (7,139 with 10-K/A), so about 12,900 requests and 27 minutes. The 200-filing sample averaged 3.9 MB per primary document, so that year is about 25 GB, which needs about 16 MB/s (125 Mbit/s) of bandwidth to keep up; on a slower connection the download time sets the pace.

On the 8-K crawl, every record got its primary document, report date and acceptance time, 97.5% a SIC code and 91% a state of incorporation. The missing values are blank at the SEC as well, in the submissions API too.

### From Rust

```rust
use parsec_edgar::{Client, CrawlQuery};

let client = Client::builder("Example Corp admin@example.com").build()?;
let query = CrawlQuery::new(&["8-K"]).since("2026-11-05").until("2026-11-06");
for entry in client.crawl_index(&query)? {
    let (filing, index) = client.resolve_index_entry(&entry)?;  // reads the -index.htm page
    let profile = index.filer(filing.cik).map(|f| &f.profile);  // SIC code, state, fiscal year end
    let sections = client.filing_sections(&filing)?;
}
let tenks = client.crawl_company("AAPL", &CrawlQuery::new(&["10-K"]).years(2015, 2025))?;
let profile = client.company_profile(320193)?;
```

`Client::full_index(quarter)` returns one quarter's index rows, `parse_master_index` and `parse_filing_index` parse files you already have, and `Client::requests_sent()` counts the HTTP requests a client has made.

### Crawl limitations

- The SEC updates the current quarter's index once a night, so a filing shows up in a full-index crawl the next business day.
- With `--company`, the profile fields are the company's current values; a full-index crawl gives the values printed on each filing.
- A ticker means the company that holds it today. After a reorganization into a new holding company, the old filings stay under the old CIK: in 2026 `XOM` moved to ExxonMobil Holdings Corp (CIK 2115436), so Exxon Mobil's earlier 10-Ks need `--company 34088`.
- Filings from before about 2001 have no separate primary document, so the crawl reads the complete submission file and its first document.
- No equivalents yet for edgar-crawler's `remove_tables` and `include_signature` options, and no Python binding for crawling.

## Section ids

| Form | Ids |
|---|---|
| 10-K (and 10-K405, 10-KSB, 10-K/A) | `item_1`, `item_1a`, `item_1b`, `item_1c`, `item_2` ... `item_16` |
| 10-Q (and 10-QSB, 10-Q/A) | `part_1_item_1` ... `part_1_item_4`, `part_2_item_1`, `part_2_item_1a` ... `part_2_item_6` |
| 8-K (and 8-K/A) | `item_1.01` ... `item_9.01` |

Each section has `id`, `part` (10-Q only), `item`, `title` (the standard title), `heading` (as printed in the filing), `start` and `end` (byte offsets in the filing text), and `text`.

How it works: every line that looks like an Item heading is a candidate, including table-of-contents entries and running page headers. The extractor picks one candidate per Item so that the chosen headings follow the form's Item order and start the longest runs of text, which skips the table of contents.

## Known limitations

- Filings from before about 2002 split only partly (a 1997 Microsoft 10-K yields 4 of its Items).
- Running page headers such as "Apple Inc. | 2026 Form 10-K | 5" stay in the text.
- Filings that attach a full annual report after Item 15 (JPMorgan, for example) put that whole report in `item_15`.
- Tables are rendered as tab-separated lines, not yet as structured data.

## Layout

| Path | Contents |
|---|---|
| `crates/parsec-edgar` | Core library |
| `crates/parsec-edgar-cli` | `parsec-edgar` command-line tool |
| `python` | Python bindings (PyO3, built with maturin) |

## Development

```sh
cargo test                       # unit tests, no network
cd python && maturin develop     # build the Python extension into the active virtualenv
```

## License

MIT
