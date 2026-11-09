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

## MCP server

`parsec-edgar mcp` runs a [Model Context Protocol](https://modelcontextprotocol.io) server, so Claude and other LLM clients can look up companies, list filings, search the full text of EDGAR and read filings one section at a time. It is part of the command-line tool (`cargo install --path crates/parsec-edgar-cli`) and uses the same User-Agent, cache and rate settings.

| Tool | Returns |
|---|---|
| `find_company` | Up to 10 filers matching a ticker, a CIK or part of a company name |
| `list_filings` | Compact rows (accession, form, filing date, period, 8-K Items), filtered by form and filing date |
| `get_filing_outline` | Filing metadata, total length, and the Item sections' ids, titles and character counts, without the text |
| `read_section` | One Item section as clean text, `max_chars` characters at a time (default 20,000), with `next_offset` while more remains |
| `read_filing_text` | The whole primary document the same way, for forms without Item sections (S-1, DEF 14A, 20-F, ...) |
| `search_full_text` | SEC full-text search hits since 2001: company, CIK, form, filing date, accession and document URL |

The tool descriptions steer the model to call `get_filing_outline` first and then read only the sections it needs. For Apple's 2026 10-K the outline is about 2,200 characters, while the full text is 232,000 and Risk Factors alone is 87,000. Offsets count characters, and a page ends at a paragraph, line or word boundary when there is one near the limit. Failures (unknown ticker, missing section, bad date) come back as tool errors that name the next step, for example the list of section ids that do exist.

### Claude Code

```sh
claude mcp add parsec-edgar -s user -e PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com" -- parsec-edgar mcp
```

### Claude Desktop

Add the server to `claude_desktop_config.json` (on macOS in `~/Library/Application Support/Claude/`, on Windows in `%APPDATA%\Claude\`) and restart Claude Desktop. Claude Desktop does not read your shell's `PATH`, so give the full path that `which parsec-edgar` prints:

```json
{
  "mcpServers": {
    "parsec-edgar": {
      "command": "/Users/you/.cargo/bin/parsec-edgar",
      "args": ["mcp"],
      "env": { "PARSEC_EDGAR_USER_AGENT": "Example Corp admin@example.com" }
    }
  }
}
```

### Other MCP clients

Over stdio, the command is `parsec-edgar mcp`, with `PARSEC_EDGAR_USER_AGENT` (or `EDGAR_IDENTITY`) set in its environment. The global options apply as usual, for example `parsec-edgar --rps 5 --cache-dir /data/edgar mcp`. The server exits with a clear message if no User-Agent is set.

For clients that connect over HTTP, run the Streamable HTTP transport:

```sh
parsec-edgar mcp --http 127.0.0.1:8080        # endpoint http://127.0.0.1:8080/mcp
claude mcp add --transport http parsec-edgar http://127.0.0.1:8080/mcp
```

The HTTP server accepts only requests addressed to `localhost`, `127.0.0.1` or `::1`, which blocks DNS rebinding from web pages. Behind a reverse proxy, add the public name with `--allow-host mcp.example.com`. It has no authentication of its own, so do not expose it directly to the internet.

### Testing

```sh
cargo test -p parsec-edgar-cli    # paging and lookup unit tests, plus stdio and HTTP protocol tests that make no SEC requests
PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com" \
    cargo test -p parsec-edgar-cli --test mcp -- --ignored --nocapture   # every tool against live SEC data, 3 requests per second
```

## Full-text search

The SEC's EDGAR full-text search (documents filed since 2001) is also available from the library. One call returns a page of up to 100 hits, best match first; page further with `offset`.

```rust
use parsec_edgar::{Client, FullTextQuery};

let client = Client::builder("Example Corp admin@example.com").build()?;
let q = FullTextQuery::new("\"supply chain disruption\" OR tariffs").forms(&["10-K"]).since("2025").cik(320193);
let results = client.full_text_search(&q)?;
println!("{} matching documents", results.total);
for hit in &results.hits {
    println!("{} {} {} {}", hit.company, hit.form, hit.filing_date, hit.document_url);
}
```

Words must all appear, `"double quotes"` match a phrase, `OR` matches either term, and `-word` excludes one. A form filter also matches the form's amendments. The SEC counts at most 10,000 hits (`total_is_exact` is false beyond that), does not page past 10,000, and returns no text snippets, so read a hit with the filing tools above.

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

How it works: every line that looks like an Item heading is a candidate, including table-of-contents entries and running page headers. Table-of-contents entries whose heading reappears in the body at a longer stretch of text, bare page headers just before an Item's titled heading, and lists of Items printed again after the signatures are dropped first. The extractor then picks one candidate per Item so that the chosen headings follow the form's Item order and start the longest runs of text. In a 10-Q, a PART line just above an Item decides whether it belongs to Part I or Part II. Without one, an Item whose title is the standard title of only one part goes to that part ("Item 4. Controls and Procedures" is Part I). Items printed out of order (Part II before Part I) are kept when a PART line marks them and they come before the signatures. The last Item ends at the signatures, at the signature block's opening sentence when there is no signatures heading, or at an attached document with its own paginated table of contents, such as a full annual report.

Accuracy is measured against 129 hand-labeled filings from 1997 to 2026 (`bench/`). On 25 of them, labeled after all tuning and scored once, the extractor finds 345 of the 349 labeled Item starts and no false ones (F1 0.994), with span accuracy 0.935. The two earlier test sets, at their first scoring and before their misses were fixed, gave F1 0.989 with span 0.997 (19 filings) and F1 0.955 with span 0.875 (20 filings). On the 104 filings it was tuned on, it scores F1 1.000 and span 0.993; those are in-sample numbers. Span accuracy is computed per filing as the characters assigned to the right Item divided by the characters inside any labeled or predicted Item, then averaged over filings. On the 25 untouched filings, edgar-crawler scores F1 0.961 with span 0.843, and edgartools 0.900 with span 0.705. [bench/README.md](bench/README.md) has the details, every miss, and how many benchmark filings each rule rests on.

## Known limitations

- Text with no Item heading stays in the Item before it. Examples: financial statement pages printed after the last Item without their own table of contents (Netflix), after Item 7A with no Item 8 heading (Kroger 2019), or between Item 9 and Part III (Allen Organ 1998), and an exhibit index printed between the last Item and the signatures (Patriot National Bancorp 2025).
- Out-of-order Items are recognized only in 10-Qs, and only when a PART line sits just above the Item. Capital One's 2020 10-Q prints Item 2 before Item 1 under one PART I line, so Item 1 is missed and Item 2 runs over it.
- An executive officers section without its own Item number, or numbered "Item 4(a)" (Lockheed Martin 2006), stays in Item 4.
- Item numbers the form does not define are not mapped to an Item: when Item 5 is printed only as "Item 5A" and "Item 5B" (McDonald's 2003), Item 5 is missing from the output.
- A heading with two separators, such as "Item 3. - Quantitative and Qualitative Disclosures" (Samsara Luggage 2023 10-Q), is not recognized, and its text goes to the Item before it.
- Running page headers and footers are removed when they carry a page number and repeat on five or more pages; short ones such as "2021 Form 10-K 100" stay.
- Tables are rendered as tab-separated lines, not yet as structured data.

## Layout

| Path | Contents |
|---|---|
| `crates/parsec-edgar` | Core library |
| `crates/parsec-edgar-cli` | `parsec-edgar` command-line tool |
| `python` | Python bindings (PyO3, built with maturin) |
| `bench` | Section-splitting accuracy and speed benchmark, with hand labels |

## Development

```sh
cargo test                       # unit tests, no network
cd python && maturin develop     # build the Python extension into the active virtualenv
cargo run --release -p parsec-edgar-bench -- score   # section accuracy on the labeled filings (downloads them once)
```

## License

MIT
