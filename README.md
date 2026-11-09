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

## Section ids

| Form | Ids |
|---|---|
| 10-K (and 10-K405, 10-KSB, 10-K/A) | `item_1`, `item_1a`, `item_1b`, `item_1c`, `item_2` ... `item_16` |
| 10-Q (and 10-QSB, 10-Q/A) | `part_1_item_1` ... `part_1_item_4`, `part_2_item_1`, `part_2_item_1a` ... `part_2_item_6` |
| 8-K (and 8-K/A) | `item_1.01` ... `item_9.01` |

Each section has `id`, `part` (10-Q only), `item`, `title` (the standard title), `heading` (as printed in the filing), `start` and `end` (byte offsets in the filing text), and `text`.

How it works: every line that looks like an Item heading is a candidate, including table-of-contents entries and running page headers. Table-of-contents entries whose heading reappears in the body, bare page headers just before an Item's titled heading, and lists of Items printed again after the signatures are dropped first. The extractor then picks one candidate per Item so that the chosen headings follow the form's Item order and start the longest runs of text. In a 10-Q, PART lines decide whether an Item belongs to Part I or Part II, and Items printed out of order (Part II before Part I) are kept. The last Item ends at the signatures, at the signature block's opening sentence when there is no signatures heading, or at an attached document with its own paginated table of contents, such as a full annual report.

Accuracy is measured against 104 hand-labeled filings from 1997 to 2026 (`bench/`). Every labeled Item start is found (F1 1.000), and 99% or more of the labeled text lands in the right Item. The extractor was tuned on these filings, including the two sets held back at first. [bench/README.md](bench/README.md) has the details and scores edgar-crawler and edgartools on the same filings.

## Known limitations

- Financial statement pages printed after the last Item without their own table of contents (Netflix, for example) stay in the last Item.
- Out-of-order Items are recognized only in 10-Q filings with PART lines.
- An executive officers section without its own Item number stays in the Item before it (usually Item 4).
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
