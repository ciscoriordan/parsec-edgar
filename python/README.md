# parsec-edgar

<!-- mcp-name: io.github.ciscoriordan/parsec-edgar -->

Fast SEC EDGAR client and filing parser written in Rust, for Python. It also installs the `parsec-edgar` command-line tool and MCP server.

- Look up companies by ticker or CIK
- List a company's filings with form and date filters, including full history back to 1994
- Convert filing HTML (or old plain-text submissions) to clean text, with table rows kept on one line
- Return the tables as rows of cells too, with their offsets in that text
- Split 10-K, 10-Q, 8-K, 10-KSB and 10-QSB filings into Item sections (Item 1A Risk Factors, Item 7 MD&A, 8-K Item 2.02, and so on)
- Search the full text of EDGAR, and bulk-download filings by form and date
- Stays within the SEC fair-access limit (8 requests per second by default, 10 at most) and caches archive documents on disk

```sh
pip install parsec-edgar
```

Wheels are built for Linux (x86-64 and ARM64), macOS (Intel and Apple silicon) and Windows (x86-64), for Python 3.10 and newer.

## The SEC User-Agent requirement

The SEC requires every request to identify the caller with a name and contact email. Pass it to `Client`, or set `PARSEC_EDGAR_USER_AGENT` (the `EDGAR_IDENTITY` variable used by edgartools also works):

```sh
export PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com"
```

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

A table is a list of rows, each a list of cells that read exactly as in the text: `$` and `)` or `%` cells are merged into their numbers ("$391,035", "(1,234)", "14.3%"), and empty cells are left out. Only numbers take them, so a header such as "Amount" keeps the "%" column beside it as a cell of its own, and "$" beside "N/A" stays apart. `columns` and `spans` give each cell's column in the table's grid, counted from 0, and how many columns it covers, with empty cells, `colspan` and `rowspan` counted as a browser counts them, hidden cells taking none, and a merged "$391,035" in the number's column. A header cell is above the values whose column is in its range, so `t["columns"][r][i]` lines values up with headers even when empty cells or a wide row label make the cell positions differ. `header_rows` lists the rows the filing marks as headers with `<thead>` or `<th>`, which most filings do not. `start` and `end` give the table's place in the text, the same text that `extract_sections` splits, so a table belongs to the section whose `start` and `end` hold its `start`. Offsets of sections and tables count bytes of the UTF-8 text: slice `text.encode()[t["start"]:t["end"]].decode()` when the text is not plain ASCII. Plain-text filings have no tables.

Dates in `since` and `until` are "YYYY", "YYYY-MM" or "YYYY-MM-DD" in digits, with the month 01 to 12 and the day 01 to 31. Lookup failures raise `LookupError`, bad arguments (such as a date "2024-13") raise `ValueError`, and network or HTTP failures raise `parsec_edgar.EdgarError`. Network calls release the GIL, so one client can be shared across threads; it stays within its rate limit.

The SEC ticker list behind `company()` is downloaded at the first lookup. Once it is a day old, the next lookup starts a new download in the background, and lookups use the old list until the new one has arrived. `tickers_max_age` sets another age in seconds, for example `pe.Client("Example Corp admin@example.com", tickers_max_age=3600)`; `math.inf` keeps the first list for the life of the client.

## Command line

```sh
parsec-edgar company AAPL
parsec-edgar filings AAPL --form 10-K --limit 5
parsec-edgar sections AAPL --form 10-K --item 1A --item 7     # JSON
parsec-edgar tables AAPL --form 10-K --item 8                 # Item 8's tables as rows of cells, JSON
parsec-edgar crawl --form 8-K --since 2026-11-05 --until 2026-11-06 --out 8k
```

`python -m parsec_edgar` runs the same command.

## MCP server

`parsec-edgar mcp` runs a [Model Context Protocol](https://modelcontextprotocol.io) server, so Claude and other LLM clients can look up companies, list filings, search the full text of EDGAR and read filings one section at a time. With [uv](https://docs.astral.sh/uv/) there is nothing to install first:

```sh
claude mcp add parsec-edgar -s user -e PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com" -- uvx parsec-edgar mcp
```

For Claude Desktop, add this to `claude_desktop_config.json` and restart it:

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

The [project README](https://github.com/ciscoriordan/parsec-edgar#readme) describes the MCP tools, bulk crawling, full-text search, the accuracy benchmark and the Rust library.

## License

MIT
