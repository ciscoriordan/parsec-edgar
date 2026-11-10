# parsec-edgar

Fast SEC EDGAR client and filing parser written in Rust, with Python bindings and a command-line tool.

- Look up companies by ticker or CIK
- List a company's filings with form and date filters, including full history back to 1994
- Convert filing HTML (or old plain-text submissions) to clean text, with table rows kept on one line and `$` signs merged into their numbers
- Return the tables as rows of cells too, with their offsets in that text
- Split 10-K, 10-Q, 8-K, 10-KSB and 10-QSB filings into Item sections (Item 1A Risk Factors, Item 7 MD&A, 8-K Item 2.02, and so on)
- Stays within the SEC fair-access limit (8 requests per second by default, 10 at most) and caches archive documents on disk

Status: early development (0.1.1).

## Install

```sh
pip install parsec-edgar          # Python library, plus the parsec-edgar command and MCP server
cargo install parsec-edgar-cli    # the parsec-edgar command, built from source (Rust 1.88 or newer)
cargo add parsec-edgar            # the Rust library
```

Python wheels are built for Linux (x86-64 and ARM64), macOS (Intel and Apple silicon) and Windows (x86-64), for Python 3.10 and newer. Each [GitHub release](https://github.com/ciscoriordan/parsec-edgar/releases) also has standalone `parsec-edgar` binaries for the same platforms.

## The SEC User-Agent requirement

The SEC requires every request to identify the caller with a name and contact email. Pass it explicitly, or set `PARSEC_EDGAR_USER_AGENT` (the `EDGAR_IDENTITY` variable used by edgartools also works):

```sh
export PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com"
```

## Command line

```sh
parsec-edgar company AAPL
parsec-edgar filings AAPL --form 10-K --limit 5
parsec-edgar filings MSFT --form 10-K --since 1994 --until 2002   # reaches into older history pages
parsec-edgar text AAPL --form 10-K                                 # clean text of the latest 10-K
parsec-edgar sections AAPL --form 10-K --item 1A --item 7          # JSON
parsec-edgar sections MSFT --form 10-Q --no-text                   # ids, titles and offsets only
parsec-edgar sections AAPL --accession 0000320193-26-000026
parsec-edgar tables AAPL --form 10-K --item 8                      # Item 8's tables as rows of cells, JSON
```

Dates are `YYYY`, `YYYY-MM` or `YYYY-MM-DD` in digits, with the month 01 to 12 and the day 01 to 31; anything else, such as `2024-13` or `2024-+1`, is an error. The same holds for `since` and `until` in Python and Rust. `--until 2024-11` keeps all of November.

Archive documents (filings, their index pages and finished quarters of the full index) are cached in `~/.cache/parsec-edgar` (override with `--cache-dir` or `PARSEC_EDGAR_CACHE`, disable with `--no-cache`). Filing lists, the ticker file and full-text search results are not cached on disk, so each command fetches them fresh. The MCP server keeps some of them in memory while it runs (see below).

## MCP server

`parsec-edgar mcp` runs a [Model Context Protocol](https://modelcontextprotocol.io) server, so Claude and other LLM clients can look up companies, list filings, search the full text of EDGAR and read filings one section at a time. It is part of the command-line tool and uses the same User-Agent, cache directory and rate settings. With [uv](https://docs.astral.sh/uv/), `uvx parsec-edgar mcp` runs it with nothing installed first; otherwise install the command as described above. The server is listed in the [MCP registry](https://registry.modelcontextprotocol.io) as `io.github.ciscoriordan/parsec-edgar`.

| Tool | Returns |
|---|---|
| `find_company` | Up to 10 filers matching a ticker, a CIK or part of a company name |
| `list_filings` | Compact rows (accession, form, filing date, period, 8-K Items), filtered by form and filing date |
| `get_filing_outline` | Filing metadata, total length, number of tables, and the Item sections' ids, titles, character counts and table counts, without the text; also how many tables are in no Item section (before the first Item, between two Items or after the last) and the index of the first of them |
| `read_section` | One Item section as clean text, `max_chars` characters at a time (default 20,000), with `next_offset` while more remains |
| `read_filing_text` | The whole primary document the same way, for forms without Item sections (S-1, DEF 14A, 20-F, ...), or one exhibit named by `document` |
| `read_tables` | The [tables](#tables) of a filing, of one Item section or of one exhibit as rows of cells, each with its section, its character offset and the short line just above it (often its title or units), `max_chars` characters of JSON at a time |
| `search_full_text` | SEC full-text search hits since 2001: company, CIK, form, filing date, accession and document URL, plus the exhibit type and file name (`document`) when the match is in an exhibit |

The tool descriptions steer the model to call `get_filing_outline` first and then read only the sections it needs. For Apple's 2026 10-K the outline is about 2,700 characters, while the full text is 230,000 and Risk Factors alone is 86,000. Offsets count characters, and a page ends at a paragraph, line or word boundary when there is one near the limit. `read_tables` pages by whole tables: `next_offset` is the index of the next table among all tables of the document, also when a section is given. A table longer than `max_chars` is split between rows, with `next_row` saying where to continue, and a single row longer than `max_chars` is cut inside a cell, with `next_char`; the last cell of one part joins the first cell of the next. So the JSON of the tables on a page never exceeds `max_chars`. Many 10-Ks print their financial statements after the last Item, or attach them, so Item 8 can have no tables at all (JPMorgan, Prologis); the outline lists those tables in `tables_outside_items`, and `read_tables` for a section without tables says where they are. A plain-text filing has its tables only in its text, and the outline and `read_tables` say so. Failures (unknown ticker or CIK, missing section, bad date) come back as tool errors that name the next step, for example the list of section ids that do exist.

Many search hits are in exhibits rather than the primary document, for example the EX-99.1 press release attached to an 8-K. For those, `search_full_text` returns the exhibit's file name as `document`, and `read_filing_text` (or `get_filing_outline`) reads that exhibit when given the hit's accession and `document`. Only plain file names ending in `.htm`, `.html`, `.txt` or `.xml` are accepted.

While it runs, the server keeps in memory the six most recently read documents (so paging through one parses it once), up to 1,000 filings it has looked up by accession number, and the latest filing of each form for 10 minutes (also up to 1,000), dropping the least recently used beyond those limits. Calls that need the same document at the same time share one download, and when it fails they all get its error. It downloads the SEC ticker file at the first company lookup. Once that copy is a day old (time the computer spends asleep counts), the next lookup starts a new download in the background, and lookups use the old copy until the new one has arrived, so a ticker added or changed while the server runs is found within about a day.

### Claude Code

```sh
claude mcp add parsec-edgar -s user -e PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com" -- uvx parsec-edgar mcp
```

With the command installed, end the line with `-- parsec-edgar mcp` instead.

### Claude Desktop

Add the server to `claude_desktop_config.json` (on macOS in `~/Library/Application Support/Claude/`, on Windows in `%APPDATA%\Claude\`) and restart Claude Desktop:

```json
{
  "mcpServers": {
    "parsec-edgar": {
      "command": "uvx",
      "args": ["parsec-edgar", "mcp"],
      "env": { "PARSEC_EDGAR_USER_AGENT": "Example Corp admin@example.com" }
    }
  }
}
```

Claude Desktop does not read your shell's `PATH`. If it cannot start the server, give the full path that `which uvx` prints as `command`, or use an installed command: the full path from `which parsec-edgar` as `command`, with `"args": ["mcp"]`.

### Other MCP clients

Over stdio, the command is `parsec-edgar mcp`, with `PARSEC_EDGAR_USER_AGENT` (or `EDGAR_IDENTITY`) set in its environment. The global options apply as usual, for example `parsec-edgar --rps 5 --cache-dir /data/edgar mcp`. The server exits with a clear message if no User-Agent is set.

For clients that connect over HTTP, run the Streamable HTTP transport:

```sh
parsec-edgar mcp --http 127.0.0.1:8080        # endpoint http://127.0.0.1:8080/mcp
claude mcp add --transport http parsec-edgar http://127.0.0.1:8080/mcp
```

The HTTP server accepts only requests addressed to `localhost`, `127.0.0.1`, `::1` or the address it listens on, which blocks DNS rebinding from web pages. With `--http 0.0.0.0:8080` it listens on every interface but still accepts only the loopback names, so give the names other machines use with `--allow-host`; behind a reverse proxy, add the public name the same way (`--allow-host mcp.example.com`). Requests from web pages must also carry a local `Origin` (`http://localhost` or `http://127.0.0.1` on any port) or one given with `--allow-origin https://app.example.com`. Native clients send no `Origin` and are not affected. The server has no authentication of its own, so do not expose it directly to the internet.

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

Words must all appear, `"double quotes"` match a phrase, `OR` matches either term, and `-word` excludes one. A form filter also matches the form's amendments. The SEC counts at most 10,000 hits (`total_is_exact` is false beyond that) and serves none past the 10,000th, so `offset` must be below 10,000. A hit whose search record names no filer or no file is left out of `hits` and its position listed in `skipped`, so the next page starts at `offset + hits.len() + skipped.len()`.

The search returns no text snippets. To read a hit, fetch `hit.document_url` and convert it with `document_to_text`, or use the MCP tools above. `hit.document` is the file that matched, which is often an exhibit (`hit.document_type` such as `EX-99.1`) rather than the filing's primary document.

## Python

```python
import parsec_edgar as pe

client = pe.Client("Example Corp admin@example.com", cache_dir="~/.cache/parsec-edgar")
client.company("BRK.B")                     # {'cik': 1067983, 'ticker': 'BRK-B', ...}
filings = client.filings("MSFT", form=["10-K", "10-Q"], since="2024")
latest = client.latest("AAPL", "10-K")      # a filing dict
text = client.text(latest)                  # clean text
for s in client.sections(latest):
    print(s["id"], s["title"], len(s["text"]))
tables = client.tables(latest)              # [{'start': ..., 'end': ..., 'header_rows': [], 'rows': [[...], ...],
                                            #   'columns': [[...], ...], 'spans': [[...], ...]}, ...]

pe.html_to_text(html)                       # parse documents you already have
pe.extract_sections(text, "10-K")
text, tables = pe.html_to_text_and_tables(html)
```

Offsets (`start` and `end` of sections and tables) count bytes of the UTF-8 text, so slice `text.encode()[t["start"]:t["end"]].decode()` when the text is not plain ASCII.

Lookup failures raise `LookupError`, bad arguments raise `ValueError`, and network or HTTP failures raise `parsec_edgar.EdgarError`. Network calls release the GIL, so one client can be shared across threads; it stays within its rate limit.

The SEC ticker list behind `company()` is downloaded at the first lookup. Once it is a day old, the next lookup starts a new download in the background, and lookups use the old list until the new one has arrived. `tickers_max_age` sets another age in seconds, for example `pe.Client("Example Corp admin@example.com", tickers_max_age=3600)`; `math.inf` keeps the first list for the life of the client.

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

// Text, tables and sections from one download and one parse.
let (text, tables) = parsec_edgar::document_to_text_and_tables(&client.primary_document(&filing)?);
let sections = parsec_edgar::extract_sections(&text, parsec_edgar::FormKind::TenK);
let item_8 = sections.iter().find(|s| s.id == "item_8").unwrap();
for t in tables.iter().filter(|t| item_8.start <= t.start && t.start < item_8.end) {
    println!("{} rows, first {:?}", t.rows.len(), t.rows[0]);
}
```

`Client::filing_tables(&filing)` returns the tables alone.

`FormKind` has two new variants, `TenKsb` and `TenQsb`, and is `#[non_exhaustive]`, and `Table` has two new fields, `columns` and `spans` (see [Tables](#tables)). Both are breaking changes from version 0.1.1: a `match` on `FormKind` needs a wildcard arm, and code that builds a `Table` with a struct literal must set the new fields. JSON written without them still reads, with empty lists.

A client downloads the SEC ticker file (`company_tickers_exchange.json`) at its first company lookup. Once that copy is a day old, counting time the computer spends asleep, the next lookup starts a new download on another thread, and lookups use the old copy until the new one has arrived. After a failed download the next try waits a minute from the failure, doubling up to an hour, counting time asleep as well. A lookup waiting for the first download gets its error as soon as it fails, also when the download panicked. `ClientBuilder::tickers_max_age` sets another age (in Python, the `tickers_max_age` keyword of `Client`, in seconds). If the wall clock is set back to before the download, the list's age is unknown and it is downloaded again; a `tickers_max_age` of `Duration::MAX` (`math.inf` in Python) keeps the first list in every case.

`Client::companies()` returns the whole list as an `Arc<[Company]>`, which stays valid after a newer list replaces it. This is a breaking change: the released version 0.1.1 returned `&[Company]`, so code written for it iterates the list with `.iter()` and passes `&list` where a `&[Company]` is wanted.

`Client::company(cik)` also finds filers that are missing from the ticker file, through the submissions API, and gives such a filer its first ticker with that ticker's own exchange.

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
- `failures.jsonl`, one line per filing that failed in the latest run, with the URL and the error (a run that fails before downloading anything leaves the previous list in place)

Running the same command again resumes the crawl: filings that already have output are skipped, and failed ones are tried again. A killed run, a full disk or a power failure cannot leave a partial record that counts as done: an incomplete JSON file is crawled again, and `filings.jsonl` is cut back to its last complete line. Progress and a final summary (ok, failed, skipped, filings per minute, requests per filing) go to stderr, and the exit status is 1 when anything failed. A panic while parsing one filing is recorded as that filing's failure, and the crawl goes on.

The SEC answers 403 to clients that go over its rate limit. On the first 403, every worker pauses for a minute and the filing is tried again; the other workers' 403s from the same moment count as the same event. If 403s come back after two such pauses with no successful download in between, the crawl stops so it can be resumed later.

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
| `primary_document`, `primary_document_url` | `htm_filing_link` | As in the submissions API; for a form the SEC renders from XML (Forms 3, 4 and 5, D, 13F and others), the HTML rendering, e.g. `xslF345X05/wk-form4_1759530830.xml` |
| `full_submission_url` | `complete_text_filing_link` | |
| `raw_file` | `filename` | With `--raw` |
| `sections` | `item_1`, `item_1A`, ... | Section text keyed by [section id](#section-ids), in filing order |
| `section_list` | | `id`, `part`, `item`, `title`, `heading`, `start`, `end` and `chars` of each section |
| `text` | | The whole text, for forms without Item sections, and for 10-K, 10-Q, 8-K, 10-KSB and 10-QSB filings in which no Item section was found (a 10-K/A that only amends the cover page, for example) |

### How filings are found

Without `--company`, the crawl reads the EDGAR full index, which lists every filing of every company, one file per quarter. It reads `master.gz`: it is pipe-delimited, while `form.idx` and `company.idx` use fixed-width columns that long company names can break, and at about 3 MB it is the smallest download (`master.idx` is 24 MB, `form.idx` 42 MB). A filing with several filers has one row per filer and is crawled once. Finished quarters are cached; the current quarter is downloaded again on each run, since the SEC adds the previous business day's filings every night. A new quarter's index appears only after its first business day; until then the crawl reports it as not published yet and finds nothing in it.

The index gives only the path of each filing's complete submission file (all documents in one SGML file, often 10 MB or more for a 10-K). The crawl therefore reads each filing's `-index.htm` page (10 to 40 KB), which gives the primary document (the row whose type equals the form, taken from its link), the report period, acceptance time, 8-K items, and the filer's SIC code, state of incorporation, business address and fiscal year end as of the filing date. That is two requests per filing. The other option, the submissions API once per company, saves nothing when most filings come from different companies, as in a day of 8-Ks or a year of 10-Ks, and it costs a 100 KB to 1 MB file per company plus extra pages for filings older than the latest thousand.

With `--company` or `--companies-file`, filings come from the submissions API: one request per company, which also gives the company profile (SIC code, state of incorporation and so on, current values). Older pages are read when the date range reaches back past the latest thousand filings, and all of them when there is no lower bound (no `--since` or `--year`). Then each filing takes one request for its document.

Measured in November 2026 at `--rps 3`, with 8 threads, each crawl starting from an empty cache directory:

| Crawl | Filings | Time | Filings per minute | Requests per filing | Failed |
|---|---|---|---|---|---|
| Every 8-K filed on 2026-11-05 and 2026-11-06 (full index) | 1,009 | 11.2 min | 90.0 | 2.00, plus 1 for the index | 0 |
| The first 200 10-Ks of 2025 in index order (full index) | 200 | 2.3 min | 89.8 | 2.00, plus 4 for the index | 0 |
| 10-Ks of 2016 to 2025 for 7 companies (`--company`) | 60 | 44 s | 173 | 1.00, plus 70 to find them | 0 |

Both full-index crawls ran at the request limit (3 per second is 90 filings a minute at 2 requests each). Parsing is far from the limit: with every document already cached, the same 200 10-Ks (788 MB of HTML) were parsed and written in under 2 seconds. Running the 10-K crawl again skipped all 200 with no requests; the company crawl's rerun skipped all 60 but repeated the 70 requests that find them, since submissions files are not cached. In the company crawl, 58 of the 70 discovery requests were JPMorgan's older submissions pages: it files about 26,000 documents a year, so its recent block covers only one year. For heavy filers over many years, the full index is cheaper.

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
let tenks = CrawlQuery::new(&["10-K"]).years(2015, 2025);
let (filings, profile) = client.crawl_company_with_profile("AAPL", &tenks)?;  // one submissions request
```

`Client::crawl_company` returns the filings alone and `Client::company_profile(cik)` the profile alone. A profile's `exchanges` has one entry per ticker, in the order of `tickers`, with `None` where the SEC lists no exchange; in version 0.1.1 it was a `Vec<String>` that left out missing exchanges, so it could fall out of step with `tickers`. A company field of an unexpected type in the submissions response is read leniently (a number becomes text, anything else counts as missing) instead of failing the filing lookup or the company crawl. `Client::full_index(quarter)` returns one quarter's index rows (none while a new quarter's index is not published yet), `parse_master_index` and `parse_filing_index` parse files you already have, and `Client::requests_sent()` counts the HTTP requests a client has made.

### Crawl limitations

- The SEC updates the current quarter's index once a night, so a filing shows up in a full-index crawl the next business day.
- A finished quarter's index is cached for good (from a few days after the quarter ends), so later corrections the SEC makes to it are not picked up. To read one again, delete `Archives/edgar/full-index/YYYY/QTRn/master.gz` from the cache directory, or crawl with `--no-cache`.
- Every document a crawl downloads also stays in the cache (`~/.cache/parsec-edgar` unless `--cache-dir` or `PARSEC_EDGAR_CACHE` says otherwise), which for a full year of 10-Ks is about 25 GB on top of the output. Point `--cache-dir` at a disk with room, or use `--no-cache` when nothing will be crawled twice.
- With `--company`, the profile fields are the company's current values; a full-index crawl gives the values printed on each filing.
- A ticker means the company that holds it today. After a reorganization into a new holding company, the old filings stay under the old CIK: in 2026 `XOM` moved to ExxonMobil Holdings Corp (CIK 2115436), so Exxon Mobil's earlier 10-Ks need `--company 34088`.
- Filings from before about 2001 have no separate primary document, so the crawl reads the complete submission file and its first document.
- No equivalents yet for edgar-crawler's `remove_tables` and `include_signature` options, and no Python binding for crawling.

## Tables

`html_to_text_and_tables` (and `document_to_text_and_tables` for any EDGAR document) returns the same text as `html_to_text`, byte for byte, plus each table as rows of cells. From Apple's 2026 10-K:

```json
{"start": 191504, "end": 191760, "header_rows": [],
 "rows": [["Number of\nRSUs\n(in thousands)", "Weighted-Average\nGrant-Date\nFair Value"],
          ["Balance as of September 27, 2025", "151,574", "$189.75"],
          ["RSUs granted", "75,538", "$260.81"], ...],
 "columns": [[3, 9], [0, 3, 10], [0, 3, 10], ...],
 "spans": [[3, 3], [3, 2, 1], [3, 2, 1], ...]}
```

- `start` and `end` are byte offsets in the text, as for sections, so a table belongs to the section whose `start..end` holds its `start`. The `tables` command adds that section's id to each table, and `--item` keeps only the tables of the given sections. `--item` (in `sections` too) takes the ids the MCP tools take: `8`, `Item 8`, `item_8`, and in a 10-Q an Item number that only one part has, such as `1A` for `part_2_item_1a`. Most 10-Qs have an Item 2 in both parts, so `2` there is an error that lists `part_1_item_2` and `part_2_item_2`; `Part I, Item 2` or `part_1_item_2` names it. An id that names no section is an error that lists the ids found.
- Cells read exactly as in the text, and empty cells are left out. A `$` or `(` cell is merged into the next cell when that cell starts with a digit, a dash or ".", so `$` and `391,035` give `$391,035`. A `%` cell is merged into the cell before it when that cell ends with a digit or a dash, and a `)` cell when that cell does or leaves a "(" open ("AA (3rd of 21" and ")"). So a header cell such as "Amount" keeps the "%" column beside it as a cell of its own, and "$" beside "N/A" stays apart. Cells of several lines, such as a header, are never merged.
- `columns` and `spans` give, for each cell of `rows`, its column in the table's grid, counted from 0, and how many columns it covers. The grid is laid out as a browser lays it out: empty cells take their columns too, a cell hidden with `display:none` or the `hidden` attribute takes none, and `colspan` and `rowspan` count, a `rowspan` only within its `<thead>`, `<tbody>` or `<tfoot>`. Only `<td>` and `<th>` cells take columns, so a hidden `<input>` or a `<form>` left inside a row does not move the cells after it. A cell merged from `$` and a number takes the number's column and span, and a cell left out of the rows, such as a page number, leaves its columns empty. A header cell is above the values whose column is in its range: above, "Number of RSUs" covers columns 3 to 5 and so holds "151,574" at column 3, and "Weighted-Average Grant-Date Fair Value" covers 9 to 11 and holds "$189.75" at column 10, while "$189.75" is the third cell of its row and the header only the second. A table in a cell of another table has a grid of its own.
- A row is one line of tab-separated cells in the text, unless a cell has several lines (the RSU header above) or the row is over 1,000 characters; then the text has its cells one after another, and the cells keep their line breaks.
- `header_rows` lists the rows inside `<thead>` and the rows whose cells are all `<th>`. Most filings use neither, so the list is usually empty; no other guess is made about headers.
- A row printed as one line can be removed as a running header or footer like any other line; the three-cell footer "2021 Form 10-K", "95", "WEC Energy Group, Inc." is one.
- In a row of two or more cells printed cell by cell, no line is removed on its own as a page number or running header, so values such as "390" or "x" stay. In a row over 1,000 characters, which is a layout table, this holds only for cells of one line. Such a row is removed as a whole when it is short and repeats on five or more pages, apart from each other, with only a rising page number changing, as a footer with the company name over the report name and the page number in the next cell does.
- Lines removed from the text as page numbers or running headers, such as a page number in a one-cell row, are removed from the tables too, and a table left with no rows is dropped.
- Layout tables, whose rows are all long blocks of text (some filings wrap whole pages in a table), are not returned, while the tables inside them are. A table in a cell of another table is returned on its own, inside the outer table's offsets, except a one-row table that became a single cell of the outer row. Tables nested more than 50 deep are printed as plain blocks of text and not returned.
- Plain-text filings (most filings before 2001) return no tables, since their tables are only lines of text. `document_is_html` tells such a document from an HTML one.

Over the 163 filings of the first five benchmark sets that is about 22,000 tables. Measured on 129 of those filings, before `columns` and `spans` were added, converting with tables took about 2% longer than `html_to_text`, whose own speed was unchanged.

## Section ids

| Form | Ids |
|---|---|
| 10-K (and 10-K405, 10-KT, 10-K/A) | `item_1`, `item_1a`, `item_1b`, `item_1c`, `item_2` ... `item_16` |
| 10-KSB (and 10-KSB40, 10-KSB405, 10-KSB/A) | `item_1`, `item_1a`, `item_2` ... `item_8`, `item_8a`, `item_8b`, `item_9` ... `item_15` |
| 10-Q (and 10-Q/A) | `part_1_item_1` ... `part_1_item_4`, `part_2_item_1`, `part_2_item_1a` ... `part_2_item_6` |
| 10-QSB (and 10-QSB/A) | `part_1_item_1` ... `part_1_item_3`, `part_2_item_1`, `part_2_item_1a` ... `part_2_item_6` |
| 8-K (and 8-K/A) | `item_1.01` ... `item_9.01` |

Form types also count without the hyphen, as EDGAR's indexes spell most small business filings (`10KSB`, `10KSB40`, `10QSB`). 10-KSB and 10-QSB filings have Items and standard titles of their own, from the forms' last versions (2008), and the earlier titles of Items that were renamed. In a 10-KSB, Item 6 is MD&A or the plan of operation, Item 7 the financial statements, Item 8 changes in accountants, Items 8A and 8B controls and procedures and other information, and Item 13 the exhibits; Item 1A (risk factors) and Item 15 (accountant fees, in 10-KSBs for 2003, whose Item 14 was controls and procedures) appear only in some filings. In a 10-QSB, Part I Item 3 is controls and procedures. When small companies moved to Forms 10-K and 10-Q in 2008 and 2009, some filings printed the other form's numbering. A filing whose titled Item headings fit the other list strictly better ("Item 8. Financial Statements" in a 10-KSB, "Item 7. Financial Statements" and "Item 8A(T)" in a 10-K) is split with that list's ids and titles.

Each section has `id`, `part` (10-Q and 10-QSB only), `item`, `title`, `heading` (as printed in the filing), `start` and `end` (byte offsets in the filing text), and `text`. `title` is the Item's standard title: the current one, or an earlier one when the printed heading names it, such as "Selected Financial Data" for 10-K Item 6 or "Submission of Matters to a Vote of Security Holders" for Item 4. The ids never change with the title.

How it works: every line that looks like an Item heading is a candidate, including table-of-contents entries and running page headers. Dashes after the separator ("Item 3. - Quantitative ...", "ITEM 1 -- FINANCIAL STATEMENTS") count as part of it, and a title may start with a form's name ("ITEM 16. 10-K SUMMARY"). A heading may name several Items ("Items 1 and 2.", "Items 1 through 5.", "Item 1-5.", "ITEMS 9, 9A AND 9B."); it starts the first of them, and its section holds the others. A range with a dash or a list with commas must go up in form order and be followed by nothing or by a title ("Items 2, 3 and 4. Not applicable.", but not "Items 3, 4 and 5 are not applicable"). A comma after the number of one Item, as running text also writes it ("Item 8, which ..."), counts only when the line ends where the Item's current standard title ends ("Item 8, Financial Statements and Supplementary Data").

An Item line that carries on a sentence from the line just above it, which ends with a comma or with a word such as "in", "see", "to" or "under" ("The information set forth in Part II," then "Item 7. Management's Discussion and Analysis, of the Company's Annual Report ..."), is a wrapped cross-reference, not a candidate. A blank line, a PART label alone ("PART II,") or a line holding only a joining word such as "and" between two headings does not count. An Item line whose title goes straight on to name the report or another document ("Item 1A. Risk Factors, of our Annual Report on Form 10-K for 2019 describes ...") is not a candidate either when a heading of that Item follows it before any later Item; without one, it may be a run-in heading and stays. A line that names several Items is dropped when a heading of its first Item alone follows it, as in an amendment's explanatory note that lists "Part II – Items 7, 8 and 9A". A list or range with nothing after the numbers is no heading on a line that starts with a Part ("Part III – Items 10, 11, 12, 13 and 14", as a cover's list of documents incorporated by reference prints it), nor, in a 10-K or 10-KSB, when it does not name Item 1 and comes before every other Item line.

Table-of-contents entries whose heading reappears in the body at a longer stretch of text, Item lines that end with a page number or a range of pages after a tab, two spaces or dot leaders ("Item 3. ... Market Risk   10 - 11"), with the rows just above them whose entries end with a page and name an earlier Item, a list on an 8-K's cover that names one Item and then "SIGNATURES" just above that Item's heading, bare page headers just before an Item's titled heading, and lists of Items printed again after the signatures are dropped first. The extractor then picks one candidate per Item so that the chosen headings follow the form's Item order and start the longest runs of text.

In a 10-Q or 10-QSB, a PART line just above an Item decides whether it belongs to Part I or Part II. Without one, an Item whose title is the standard title of only one part goes to that part ("Item 4. Controls and Procedures" is Part I). A line that starts with a Part I title and runs on as a sentence that cites it ("Item 1. Financial Statements, of this report is hereby updated") is a wrapped cross-reference, not a heading. Items printed out of order (Part II before Part I, or Item 1 after Item 2) are kept when they come before the signatures and a PART line marks them or their title is a Part I standard title.

In a 10-K or 10-KSB, "Item 4A" is the executive officers section, and so is "Item 4(a). Executive Officers ...": it starts no Item, and with its title on the same line it ends the Item before it. A sub-item printed with the standard title of a lettered Item the form defines is that Item: "ITEM 9(A). CONTROLS AND PROCEDURES" is Item 9A, also with the title on the next line, and a 10-KSB's "Item 8(A)T. Controls and Procedures" is Item 8A. "Item 9 A (T)." is Item 9A too. When an Item has no heading of its own and is printed only as lettered headings the form does not define ("Item 5A", "Item 5B"), the first of them starts it. When the first Item found is Item 1A or Item 2, a line that holds only Item 1's standard title after the last PART I line before it ("PART I." and then "BUSINESS") starts Item 1, unless an Item line comes between them or the title is printed there more than once (as a page header).

Items 9 and 16 (10-KSB Items 8, 14 and 15), and Item 7A when there is no Item 8 heading, end where text with no Item heading of its own begins: an auditors' report, an index to the financial statements, a financial statement schedule, an exhibit index or a consent. Item 16 (10-KSB Items 14 and 15) also ends at a glossary heading ("Glossary", "GLOSSARY OF TERMS"), and so does the exhibits Item when no Item follows it. Item 9B (10-KSB Item 8B) ends at the heading of an auditors' report or of management's report on internal control printed after it, but not at a sentence that starts with such a title; Item 9A keeps those reports. The exhibits Item (Item 15, the old Item 14, 10-KSB Item 13) ends where it prints financial statements whose pages are numbered apart from the report's (F-1, F-2, ...) after all of its own text: an index with such page numbers, then at least two of the statements it lists, and no exhibit index, exhibit list or other part of the Item after them. When no Item follows it and Item 8 only refers to the statements, it also ends where statements with any page numbers, or none, follow all of its own text: an auditors' report, then headings that repeat at least three titles the Item lists. There an exhibit index after the statements does not keep them in the Item, while the Item's own captions do ("(a)(3) Exhibits", an exhibit table's "Exhibit No." header). A glossary or "Power of Attorney" line next to an exhibit number ("24", "Exhibit 24", "EX-24") is an exhibit's description in a list printed cell by cell and ends nothing. Page headers printed just above any of these cuts, such as the company name at the top of each statement page, go with the text after the cut.

The last Item ends at the signatures, at the signature block's opening sentence when there is no signatures heading, at a "POWER OF ATTORNEY" heading before the signatures when the text after it reads as a power of attorney ("KNOW ALL PERSONS BY THESE PRESENTS", "constitutes and appoints"), or at an attached document with its own paginated table of contents, such as a full annual report.

Accuracy is measured against 200 hand-labeled filings from 1996 to 2026 (`bench/`), in six sets labeled at different times. The evidence for filings the extractor has not seen is each set's first scoring, made before any of its misses were fixed. The newest set, fresh3, has 34 filings, two 10-KSBs and two 10-QSBs among them, that were labeled after all earlier tuning and committed before any scoring. Scored once with the current code, it found 430 of the 432 labeled Item starts and predicted 12 Items that start on no labeled line: F1 0.984 (precision 0.973, recall 0.995), with span accuracy 0.978. Eleven of those 12 come from one 10-K that answers the form through a cross-reference index (Banknorth 1999), whose index rows are taken for Item headings (#32). Version 0.1.1 cannot read the 10-KSB and 10-QSB filings; on the other 30 it scores F1 0.977 with span 0.951, against 0.985 and 0.975 for the current code, and no filing scores worse with the current code. The earlier sets gave, at their first scoring, F1 0.988 with span 0.895 (fresh2, 30 filings), F1 0.994 with span 0.935 (fresh, 25 filings), F1 0.989 with span 0.997 (blind, 19 filings) and F1 0.955 with span 0.875 (held out, 20 filings). The misses of those four sets and of the development set, 166 filings together, have since been fixed, so their current scores are in-sample: F1 1.000 on all five sets, and span accuracy 1.000 when rounded to three places on four of them and 0.971 on fresh2, where most of the loss is one 10-Q whose last Item runs to the end of the document (Harleysville Savings 2002, #30). Span accuracy is computed per filing as the characters assigned to the right Item divided by the characters inside any labeled or predicted Item, then averaged over filings.

On fresh3, edgar-crawler scores F1 0.992 with span 0.950, and edgartools F1 0.842 with span 0.625. Neither was tuned on these filings. edgar-crawler's F1 is higher than parsec-edgar's (0.984) because of the one cross-reference 10-K: it finds only that filing's Item 14, at its body heading, while parsec-edgar takes the index rows for 11 Items, 10 of them for Items the labels list as absent. Without that filing, parsec-edgar scores F1 0.998 and edgar-crawler 0.992. parsec-edgar's span accuracy is higher: edgar-crawler runs the last Item of WellCare's 2019 and Omnicell's 2025 10-Ks on through the financial statements printed after it, and misses Part I Item 1 of a 10-QSB. On fresh2 at its first scoring, edgar-crawler scored F1 0.980 with span 0.917, so there its span accuracy was the higher one. [bench/README.md](https://github.com/ciscoriordan/parsec-edgar/blob/main/bench/README.md) has the details, every miss, how many benchmark filings each rule rests on, and a regression check on 150 unlabeled filings.

## Known limitations

- Text with no Item heading of its own is cut off only at the ends described above: 10-K Items 7A (when there is no Item 8 heading), 9, 9B and 16, 10-KSB Items 8, 8B, 14 and 15, and the exhibits Item where financial statements, or a glossary when no Item follows, come after all of its own text. Elsewhere it stays in the Item before it: statements or reports printed after Item 9A or a Part III Item, or after Item 7 when there is neither an Item 7A nor an Item 8 heading; schedules printed after the exhibit index of the last exhibits Item (Vesta Insurance 2003, whose Item 15 then holds 12,574 characters against 6,956 labeled, #12); and other untitled text after Item 16. In a 10-Q, text printed between the PART II line and Part II Item 1, such as the note on forward-looking statements in Wendy's 2018 10-Q, stays in Part I's last Item (#33). Exhibit rows printed after the statements with no caption or table header do not count as the exhibits Item's own text, so they leave the Item with the statements. The headings that end Item 9B and the text that marks a power of attorney are recognized from lists of words, so other wordings do not end the Item.
- A page header printed just above a cut moves out of the Item only when the text after the cut repeats it. A letterhead printed once stays (Allen Organ 1998), and so do the lines above a cut at an attached document (Exxon Mobil 2005, and the 43-character index heading of ITT 2015, #26). A filing that prints "Index to Financial Statements" at the top of every page (Exxon Mobil 2005, Energy Transfer 2025) has Items 9 and 16 cut at their first page break, which loses text when such an Item runs over more than one page.
- An executive officers section without its own Item number stays in the Item where it is printed (Item 1, or Item 4 when it follows it), as the benchmark labels have it. One numbered "Item 4A" or "Item 4(a)" ends Item 4 and belongs to no Item, while "Item 1(a). Executive Officers", a sub-item of an Item the form defines, stays inside Item 1.
- Out-of-order Items are recognized only in 10-Qs and 10-QSBs, when a PART line marks them or their title is a Part I standard title. One printed before the first Item kept in form order is added only when a contents list names the body headings, so MD&A printed before longer financial statements in a 10-Q without a matching contents list is missed. A Part II Item out of order with no PART line is missed too.
- A cross-reference is recognized when the line above it stops inside a sentence (it ends with a comma or a word such as "in", "see", "to" or "under"), when a 10-Q Part I title runs on as a sentence, or when the title goes on to name a document and the Item's own heading follows it. A reference printed after the Item's own heading, inside a later Item, or one that names no document after a line that ends some other way ("... under the caption", "... in Part II"), can still be taken as that Item's heading (#15).
- An Item line outside the body can take the place of a short body heading or of a missing one: an exhibit after the signatures that repeats an Item heading, or a 10-Q/A's explanatory note that lists Part I Items above a contents list without page numbers (#19). A cross-reference index whose rows give several pages or none ("3-30, 64", "N/A") and wrap their titles is not recognized as contents, so its rows become Items, and the one Item with a body heading starts at its row (Banknorth 1999, 11 Items from the index, #32). In a 10-K that prints no Item headings, a cover row that lists Items with "and" or "through" ("Part III – Items 10 through 14"), or that names Item 1, becomes the first Item and runs to the signatures (#35). A Part title printed with an Item number ("Item 1. Financial Information (Unaudited)" just above "Item 1. Financial Statements (Unaudited)") is taken for Part I Item 1, one line early (Artisanal Brands 10-QSB 2005, #34).
- Some headings are not recognized: Item numbers in roman numerals ("ITEM I.  BUSINESS", Stryker 1997, #31); a comma after the number of one Item followed by any title other than the form's current standard title ("Item 10, Directors and Executive Officers of the Registrant"); a heading that ends with a footnote's number after a tab, which is taken for a contents row, so its text goes to the Item before it (#36); and an Item 1 printed without a number whose pages print "PART I" and "Business" as page headers, whose text is then in no section (#37).
- Without a signatures heading, the last Item ends at the signature block's opening sentence only when that sentence is on one line. One wrapped over two lines ("... the Bank" / "has duly caused ...") is not found, and the last Item runs to the end of the document (Harleysville Savings 2002, #30).
- `title` follows the printed heading, not the filing date. A heading that names several Items ("ITEMS 10 - 13.") or prints other words keeps the current title even in a filing made before the form had that title, and an abbreviated recent heading that prints only words that an earlier title shares with the current one ("Item 10. Directors and Executive Officers" in a 10-K for 2015) gets the earlier title (#27). `heading` has the title as printed.
- Whether a 10-K, 10-KSB, 10-Q or 10-QSB is split with its own form's Items or the other form's is decided by the titles it prints. A filing whose titles fit both lists equally keeps its form type's list, so a 10-K/A that prints only "Item 8A(T)" and "Item 15" is split with the 10-K Items, and the text of its Item 8A(T) is in no section.
- Running page headers and footers are removed when they repeat on five or more pages with a rising page number at the start or end of the line, in a leading "Page N of M", or in the middle cell of a three-cell row ("2021 Form 10-K", "95", "WEC Energy Group, Inc."), also when the last cell ends in a number ("2021 Annual Report", "61", "Part II, Item 8"). Front matter numbered i, ii, iii followed by pages 1, 2, 3 counts as rising, and so does a "Page 1 of M" footer that starts again in each exhibit ("Page 1 of 3", then "Page 1 of 2"). A short header that does not name the form is removed when a "|" separates the number ("ACME | 23") or the number alternates sides as on facing pages ("24 ACME", "ACME 25"). Page numbers alone on their line after one to three capital letters, a chapter number or a lone dash ("FS - 12" in Ford 2010, "S-3", "II-4", "7-3", "- 4") are removed when five or more copies with the same prefix are mostly not on consecutive lines and mostly come one to three pages after the copy before, so exhibit lists such as "EX-21", "EX-23", "EX-31" stay. Page numbers alone on their line in parentheses or with a period ("(12)", "12.") are removed when each ends a page, as marked by page-break styles in HTML or `<PAGE>` tags in plain text, and five or more come one to three pages apart. A short row printed cell by cell is removed as a whole. Some stay: other short headers with fewer than 10 letters beside the number that do not name the form ("IBM 2020 23"), because captions such as "Note 4" look the same; numbers in the middle of a line that is not a three-cell row; prefixes that mix digits and letters, such as "7A-1"; and page numbers with a period in an HTML filing that marks no page breaks (Presidential Life 2004), which look like note numbers (#16).
- Table `columns` follow the filing's HTML grid, so values line up with their headers only as far as the filing's own `colspan` does. A filing that prints a header over the `$` column and the number column without a `colspan` has the header above the `$` column, one column left of the number.

## Layout

| Path | Contents |
|---|---|
| `crates/parsec-edgar` | Core library |
| `crates/parsec-edgar-cli` | `parsec-edgar` command-line tool |
| `python` | Python bindings (PyO3, built with maturin) |
| `bench` | Section-splitting accuracy and speed benchmark, with hand labels |
| `server.json` | The MCP registry entry |
| `.github/workflows` | Continuous integration and the release workflow |

## Development

```sh
cargo test                       # unit tests, no network
cargo fmt && cargo clippy --workspace --all-targets   # style (rustfmt.toml) and lints
cd python && maturin develop     # build the Python extension into the active virtualenv
python -m pytest python/tests    # Python package and command, no network
cargo run --release -p parsec-edgar-bench -- score   # section accuracy on the labeled filings (downloads them once)
```

GitHub Actions runs the tests on Linux, macOS and Windows, checks the minimum Rust version (1.88), and builds and tests the Python package.

### Releasing

Set the new version in `Cargo.toml` (`workspace.package.version`) and in both places in `server.json`, commit, then push a tag such as `v0.2.0`. The release workflow builds the wheels and binaries, then publishes to PyPI, crates.io, a GitHub release and the MCP registry. Running the workflow by hand from the Actions tab builds and tests everything without publishing.

## License

MIT
