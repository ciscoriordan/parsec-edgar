//! MCP server: `parsec-edgar mcp` over stdio, or Streamable HTTP with `--http ADDR`.
//!
//! Tools return compact JSON and page long text and lists of tables, so an
//! agent can read an outline first and then only the sections it needs.

use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use parsec_edgar::{
    Client, Company, Error, FULL_TEXT_MAX_RESULTS, FULL_TEXT_PAGE_SIZE, Filing, FilingQuery, FormKind, FullTextHit,
    FullTextQuery, FullTextResults, Section, Table, document_is_html, document_to_text_and_tables, extract_sections,
};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::OnceCell;

use crate::find_section;

const DEFAULT_MAX_CHARS: usize = 20_000;
const MAX_MAX_CHARS: usize = 100_000;
const MIN_MAX_CHARS: usize = 200;
const DEFAULT_FILINGS: usize = 20;
const MAX_FILINGS: usize = 200;
const DEFAULT_HITS: usize = 20;
const MAX_COMPANY_MATCHES: usize = 10;
/// Parsed filings kept in memory, so paging through one filing parses it once.
const DOC_CACHE_SIZE: usize = 6;
/// Filings kept in memory by accession, and separately latest-filing lookups.
/// Each entry is small, so there can be many more of them than documents.
const FILING_CACHE_SIZE: usize = 1_000;
/// How long "latest 10-K of X" stays resolved to one filing.
const LATEST_TTL: Duration = Duration::from_secs(600);
/// Longest line shown as the text just above a table.
const MAX_TEXT_BEFORE: usize = 200;

const INSTRUCTIONS: &str = "Tools for SEC EDGAR filings (10-K, 10-Q, 8-K, proxy statements and more). \
Typical flow: find_company (or use a ticker directly), then list_filings or search_full_text to pick a filing, \
then get_filing_outline to see its Item sections and their sizes, then read_section for only the parts you need. \
Text comes in pages of max_chars characters (default 20,000, about 5,000 tokens); follow next_offset only when \
you need more. Use read_filing_text for forms without Item sections, and with a search hit's document for \
exhibits. Use read_tables for a filing's tables (financial statements and others) as rows of cells.";

/// Options of `parsec-edgar mcp`.
#[derive(clap::Args)]
pub struct McpArgs {
    /// Serve Streamable HTTP at this address instead, e.g. 127.0.0.1:8080 (endpoint /mcp).
    #[arg(long, value_name = "ADDR")]
    http: Option<SocketAddr>,
    /// Host header value to accept over HTTP besides localhost and the address served,
    /// e.g. mcp.example.com. Repeat for several.
    #[arg(long = "allow-host", value_name = "HOST", requires = "http")]
    allow_hosts: Vec<String>,
    /// Browser origin to accept over HTTP besides localhost, e.g. https://app.example.com
    /// (add :* for any port). Repeat for several.
    #[arg(long = "allow-origin", value_name = "ORIGIN", requires = "http")]
    allow_origins: Vec<String>,
}

/// Run the server until the client disconnects (stdio) or Ctrl-C (HTTP).
pub fn serve(client: Client, args: McpArgs) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let server = EdgarServer::new(client);
    runtime.block_on(async move {
        match args.http {
            None => {
                let running = server
                    .serve(rmcp::transport::stdio())
                    .await
                    .map_err(|e| Error::Invalid(format!("MCP initialization failed: {e}")))?;
                running.waiting().await.map_err(|e| Error::Invalid(format!("MCP server stopped: {e}")))?;
            }
            Some(addr) => serve_http(server, addr, &args.allow_hosts, &args.allow_origins).await?,
        }
        Ok(())
    })
}

/// Which requests the HTTP server answers, and the endpoint to print.
#[derive(Debug, PartialEq)]
struct HttpAccess {
    /// Accepted `Host` header values (any port).
    hosts: Vec<String>,
    /// Accepted browser `Origin` values. Requests without an Origin, from native clients, always pass.
    origins: Vec<String>,
    url: String,
    note: Option<String>,
}

/// Loopback names always work. A specific address served is accepted too, while
/// for 0.0.0.0 or :: other names must be given with --allow-host.
fn http_access(bound: SocketAddr, allow_hosts: &[String], allow_origins: &[String]) -> HttpAccess {
    let mut hosts: Vec<String> = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];
    let mut origins: Vec<String> = ["localhost", "127.0.0.1", "[::1]"]
        .iter()
        .flat_map(|h| [format!("http://{h}:*"), format!("https://{h}:*")])
        .collect();
    let ip = bound.ip();
    let mut note = None;
    let shown = if ip.is_unspecified() {
        note = Some(
            "listening on all interfaces: requests must address localhost or 127.0.0.1, \
             or a name given with --allow-host"
                .to_owned(),
        );
        SocketAddr::new(IpAddr::from([127, 0, 0, 1]), bound.port())
    } else {
        if !hosts.contains(&ip.to_string()) {
            let host = if ip.is_ipv6() { format!("[{ip}]") } else { ip.to_string() };
            origins.push(format!("http://{host}:*"));
            hosts.push(host);
        }
        bound
    };
    hosts.extend(allow_hosts.iter().cloned());
    origins.extend(allow_origins.iter().cloned());
    HttpAccess { hosts, origins, url: format!("http://{shown}/mcp"), note }
}

/// Check an --allow-origin value the way rmcp reads it, since rmcp skips an
/// entry it cannot read without saying so: `null`, or a scheme and host with
/// an optional port or `:*`, and no path.
fn check_origin(value: &str) -> Result<(), Error> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("null") {
        return Ok(());
    }
    let (base, any_port) = v.strip_suffix(":*").map_or((v, false), |b| (b, true));
    let ok = axum::http::Uri::try_from(base).is_ok_and(|uri| {
        uri.scheme().is_some()
            // `:*` replaces the port, so an entry cannot carry both.
            && uri.authority().is_some_and(|a| !a.host().is_empty() && (!any_port || a.port().is_none()))
            && matches!(uri.path(), "" | "/")
            && uri.query().is_none()
    });
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "--allow-origin {value:?} is not an origin: give a scheme and host, \
             e.g. https://app.example.com, https://app.example.com:8443 or http://localhost:*"
        )))
    }
}

async fn serve_http(
    server: EdgarServer,
    addr: SocketAddr,
    allow_hosts: &[String],
    allow_origins: &[String],
) -> Result<(), Error> {
    for origin in allow_origins {
        check_origin(origin)?;
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let access = http_access(listener.local_addr()?, allow_hosts, allow_origins);
    let config =
        StreamableHttpServerConfig::default().with_allowed_hosts(access.hosts).with_allowed_origins(access.origins);
    let cancel = config.cancellation_token.clone();
    let service =
        StreamableHttpService::new(move || Ok(server.clone()), Arc::new(LocalSessionManager::default()), config);
    let router = axum::Router::new().nest_service("/mcp", service);
    eprintln!("parsec-edgar MCP server listening on {}", access.url);
    if let Some(note) = access.note {
        eprintln!("{note}");
    }
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancel.cancel();
        })
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct EdgarServer {
    client: Arc<Client>,
    cache: Arc<Mutex<Cache>>,
    tool_router: ToolRouter<Self>,
}

/// One document of a filing, as text and sections, parsed once. Co-registrants
/// share it, so it holds no filer metadata: that comes from the filing each call resolves.
struct Doc {
    /// File name of an exhibit or other attachment; `None` for the primary document.
    document: Option<String>,
    text: String,
    total_chars: usize,
    /// `None` for forms without Item sections, and for attachments.
    sections: Option<Vec<Section>>,
    /// Tables, with byte offsets into `text`.
    tables: Vec<Table>,
    /// False for a plain-text document, whose tables are only in its text.
    html: bool,
}

/// Accession number, and the file name of an attachment or "" for the primary document.
type DocKey = (String, String);

/// What a running server keeps in memory. Each map is bounded, by a size limit
/// or, for loads, by the calls in progress, so a long-running server does not
/// keep growing.
#[derive(Default)]
struct Cache {
    /// (company, accession) to filing. Filings never change.
    by_accession: Lru<(String, String), Filing, FILING_CACHE_SIZE>,
    /// (company, form) to the latest filing, with the time it was looked up.
    latest: Lru<(String, String), (Instant, Filing), FILING_CACHE_SIZE>,
    /// Parsed documents.
    docs: Lru<DocKey, Arc<Doc>, DOC_CACHE_SIZE>,
    /// Documents being loaded, so concurrent calls share one download and parse.
    loading: HashMap<DocKey, Loading>,
}

/// A document being loaded. The entry goes when its last call returns or is cancelled.
#[derive(Default)]
struct Loading {
    /// Calls waiting for the document.
    waiting: usize,
    doc: Arc<OnceCell<Arc<Doc>>>,
}

impl Cache {
    /// The latest filing for `key` if it was looked up less than LATEST_TTL
    /// before `now`. An older entry is dropped.
    fn fresh_latest(&mut self, key: &(String, String), now: Instant) -> Option<Filing> {
        if let Some((at, filing)) = self.latest.get(key)
            && now.saturating_duration_since(*at) < LATEST_TTL
        {
            return Some(filing.clone());
        }
        self.latest.remove(key);
        None
    }
}

/// A map that keeps at most `CAP` entries, dropping the least recently used.
struct Lru<K, V, const CAP: usize> {
    /// Each value with the clock reading of its last use.
    entries: HashMap<K, (u64, V)>,
    /// Counts lookups and inserts.
    clock: u64,
}

impl<K, V, const CAP: usize> Default for Lru<K, V, CAP> {
    fn default() -> Self {
        Self { entries: HashMap::new(), clock: 0 }
    }
}

impl<K: Eq + Hash + Clone, V, const CAP: usize> Lru<K, V, CAP> {
    /// The value under `key`, which becomes the most recently used.
    fn get(&mut self, key: &K) -> Option<&V> {
        self.clock += 1;
        let entry = self.entries.get_mut(key)?;
        entry.0 = self.clock;
        Some(&entry.1)
    }

    /// Keep `value` under `key`, replacing any value there, and drop the least
    /// recently used entry beyond CAP.
    fn put(&mut self, key: K, value: V) {
        self.clock += 1;
        self.entries.insert(key, (self.clock, value));
        if self.entries.len() > CAP
            && let Some(oldest) = self.entries.iter().min_by_key(|(_, (used, _))| *used).map(|(k, _)| k.clone())
        {
            self.entries.remove(&oldest);
        }
    }

    fn remove(&mut self, key: &K) {
        self.entries.remove(key);
    }
}

// Tool parameters. Doc comments become the schema descriptions the model sees.

#[derive(Deserialize, JsonSchema)]
struct FindCompanyParams {
    /// Ticker (AAPL, BRK.B), CIK (320193) or part of a company name (case-insensitive, e.g. "berkshire").
    query: String,
}

#[derive(Deserialize, JsonSchema)]
struct ListFilingsParams {
    /// Ticker or CIK of the filer.
    company: String,
    /// Form types to keep, e.g. ["10-K", "10-Q"]. Exact match, so add "10-K/A" for amendments. Omit for all forms.
    #[serde(default, deserialize_with = "string_or_list")]
    forms: Vec<String>,
    /// Earliest filing date: YYYY, YYYY-MM or YYYY-MM-DD.
    since: Option<String>,
    /// Latest filing date: YYYY, YYYY-MM or YYYY-MM-DD.
    until: Option<String>,
    /// Maximum rows, newest first (default 20, at most 200).
    limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct FilingParams {
    /// Ticker or CIK of the filer.
    company: String,
    /// Accession number from list_filings or search_full_text, e.g. 0000320193-26-000026.
    accession: Option<String>,
    /// Without an accession, use the latest filing of this form (default 10-K).
    form: Option<String>,
    /// File name of an exhibit or other attachment of the filing, from a search_full_text hit's `document`
    /// (e.g. ex99-1.htm). Needs the accession. Omit for the primary document.
    document: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ReadSectionParams {
    /// Ticker or CIK of the filer.
    company: String,
    /// Accession number from list_filings or search_full_text, e.g. 0000320193-26-000026.
    accession: Option<String>,
    /// Without an accession, use the latest filing of this form (default 10-K).
    form: Option<String>,
    /// Section id from get_filing_outline, e.g. item_1a, item_7, part_1_item_2, item_2.02. "1A" and "Item 7" also work.
    section: String,
    /// Character offset to start at (default 0). Use next_offset from the previous page.
    offset: Option<usize>,
    /// Maximum characters to return (default 20000, at most 100000).
    max_chars: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct ReadTextParams {
    /// Ticker or CIK of the filer.
    company: String,
    /// Accession number from list_filings or search_full_text, e.g. 0000320193-26-000026.
    accession: Option<String>,
    /// Without an accession, use the latest filing of this form (default 10-K).
    form: Option<String>,
    /// File name of an exhibit or other attachment of the filing, from a search_full_text hit's `document`
    /// (e.g. ex99-1.htm). Needs the accession. Omit for the primary document.
    document: Option<String>,
    /// Character offset to start at (default 0). Use next_offset from the previous page.
    offset: Option<usize>,
    /// Maximum characters to return (default 20000, at most 100000).
    max_chars: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct ReadTablesParams {
    /// Ticker or CIK of the filer.
    company: String,
    /// Accession number from list_filings or search_full_text, e.g. 0000320193-26-000026.
    accession: Option<String>,
    /// Without an accession, use the latest filing of this form (default 10-K).
    form: Option<String>,
    /// File name of an exhibit or other attachment of the filing, such as the ex99-1.htm press release of an
    /// 8-K, from a search_full_text hit's `document`. Needs the accession. Omit for the primary document.
    document: Option<String>,
    /// Only tables in this section: an id from get_filing_outline, e.g. item_8, item_7, part_1_item_1.
    /// Omit for every table of the document, including those in no Item section.
    section: Option<String>,
    /// Index of the first table to return (default 0), counting every table of the document, also when a
    /// section is given: the same index as next_offset, a table's index and tables_outside_items' first_index.
    offset: Option<usize>,
    /// Rows to skip in the table at offset (default 0). Use next_row from the previous page when it is set.
    row: Option<usize>,
    /// Characters of that row to skip (default 0). Use next_char from the previous page when it is set.
    char: Option<usize>,
    /// Maximum characters of table JSON to return (default 20000, at most 100000).
    max_chars: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct SearchParams {
    /// Words that must all appear. "double quotes" match an exact phrase, OR between terms matches either,
    /// -word excludes a word. Example: "supply chain" OR tariffs
    query: String,
    /// Form types, e.g. ["10-K", "8-K"]. A form also matches its amendments.
    #[serde(default, deserialize_with = "string_or_list")]
    forms: Vec<String>,
    /// Earliest filing date: YYYY, YYYY-MM or YYYY-MM-DD.
    since: Option<String>,
    /// Latest filing date: YYYY, YYYY-MM or YYYY-MM-DD.
    until: Option<String>,
    /// Only filings by this company (ticker or CIK).
    company: Option<String>,
    /// Number of hits to skip (default 0). Use next_offset from the previous page.
    offset: Option<usize>,
    /// Maximum hits to return (default 20, at most 100).
    limit: Option<usize>,
}

/// Accept `"10-K"`, `"10-K, 10-Q"` or `["10-K", "10-Q"]`.
fn string_or_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    let items = match Option::<OneOrMany>::deserialize(d)? {
        None => Vec::new(),
        Some(OneOrMany::One(s)) => s.split(',').map(str::to_owned).collect(),
        Some(OneOrMany::Many(v)) => v,
    };
    Ok(items.into_iter().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect())
}

#[tool_router]
impl EdgarServer {
    pub fn new(client: Client) -> Self {
        Self { client: Arc::new(client), cache: Arc::default(), tool_router: Self::tool_router() }
    }

    #[tool(
        description = "Look up SEC filers by ticker (AAPL, BRK.B), CIK (320193) or part of a company name \
            (case-insensitive). Returns up to 10 matches with CIK, name, tickers and exchange. Pass a ticker or \
            CIK from the result as `company` to the other tools. Name search covers companies with a listed ticker; \
            others can be found by CIK or with search_full_text.",
        annotations(title = "Find company", read_only_hint = true, open_world_hint = true)
    )]
    async fn find_company(&self, Parameters(p): Parameters<FindCompanyParams>) -> Result<String, String> {
        let client = self.client.clone();
        let query = p.query.trim().to_owned();
        if query.is_empty() {
            return Err("query is empty: pass a ticker, CIK or part of a company name".into());
        }
        let matches = blocking(move || {
            if let Some(cik) = parse_cik(&query) {
                let company = client.company(&query).map_err(explain)?;
                let mut m = CompanyMatch::from(&company);
                // Add the other share classes from the ticker file.
                if let Ok(all) = client.companies() {
                    for c in all.iter().filter(|c| c.cik == cik) {
                        if let Some(t) = &c.ticker
                            && !m.tickers.contains(t)
                        {
                            m.tickers.push(t.clone());
                        }
                    }
                }
                return Ok(vec![m]);
            }
            let all = client.companies().map_err(explain)?;
            Ok(search_companies(&all, &query, MAX_COMPANY_MATCHES))
        })
        .await?;
        if matches.is_empty() {
            return Err(format!(
                "no company matches {:?} in the SEC ticker file. Try a shorter part of the name, a CIK, \
                 or search_full_text (which also covers filers without a ticker).",
                p.query.trim()
            ));
        }
        Ok(to_json(&CompanyMatches { matches }))
    }

    #[tool(
        description = "List a company's SEC filings, newest first, as compact rows: accession, form, filing date, \
            period of report and, for 8-K, the Item numbers. Filter by form types and filing dates (older history \
            is fetched when `since` reaches back). Pass an accession to get_filing_outline, read_section or \
            read_filing_text to read that filing.",
        annotations(title = "List filings", read_only_hint = true, open_world_hint = true)
    )]
    async fn list_filings(&self, Parameters(p): Parameters<ListFilingsParams>) -> Result<String, String> {
        let limit = p.limit.unwrap_or(DEFAULT_FILINGS).clamp(1, MAX_FILINGS);
        let client = self.client.clone();
        let company = p.company.clone();
        let query = list_query(p.forms, p.since, p.until, limit);
        let filings = blocking(move || client.filings(&company, &query).map_err(explain)).await?;
        let rows: Vec<FilingRow> = filings.iter().take(limit).map(FilingRow::from).collect();
        let note = list_note(filings.len(), limit);
        Ok(to_json(&FilingList {
            company: filings.first().map(|f| f.company.clone()),
            cik: filings.first().map(|f| f.cik),
            count: rows.len(),
            note,
            filings: rows,
        }))
    }

    #[tool(
        description = "Outline of one filing without its text: filing metadata, the total character count, the \
            number of tables and, for 10-K, 10-Q and 8-K (and amendments), the Item sections with ids, titles, \
            character counts and table counts. Tables in no Item section, such as financial statements printed \
            after the last Item, are counted in tables_outside_items, each run with first_index, the index to \
            pass to read_tables as offset. Call this first, then read only the sections you need with \
            read_section, or their tables with read_tables. Identify the filing by company plus accession, or \
            company plus form for the latest filing of that form (default 10-K). For an exhibit hit from \
            search_full_text, also pass its document.",
        annotations(title = "Filing outline", read_only_hint = true, open_world_hint = true)
    )]
    async fn get_filing_outline(&self, Parameters(p): Parameters<FilingParams>) -> Result<String, String> {
        let (filing, doc) =
            self.doc(&p.company, p.accession.as_deref(), p.form.as_deref(), p.document.as_deref()).await?;
        let outside = match &doc.sections {
            Some(sections) if !sections.is_empty() => outside_runs(sections, &doc.tables),
            _ => Vec::new(),
        };
        let mut next = match (&doc.document, &doc.sections) {
            (Some(name), _) => format!(
                "{name} is an exhibit or attachment without Item sections; read it with read_filing_text, \
                 passing the same accession and document"
            ),
            (None, Some(s)) if !s.is_empty() => {
                "read_section with one of these ids; long sections come in pages of max_chars".to_owned()
            }
            (None, Some(_)) if !doc.tables.is_empty() => format!(
                "no Item headings were found in this filing; use read_filing_text, and read_tables without \
                 section for its {} tables",
                doc.tables.len()
            ),
            (None, Some(_)) => "no Item headings were found in this filing; use read_filing_text".to_owned(),
            (None, None) => format!(
                "Item sections are extracted for 10-K, 10-Q and 8-K only, not {}; use read_filing_text",
                filing.form
            ),
        };
        if !outside.is_empty() {
            next.push_str(
                "; read the tables in tables_outside_items with read_tables without section, from offset=first_index",
            );
        }
        if !doc.html {
            next.push_str("; this is a plain-text document, so its tables are only in its text");
        }
        let sections = doc.sections.as_ref().map(|sections| {
            sections
                .iter()
                .map(|s| SectionRow {
                    id: &s.id,
                    title: &s.title,
                    chars: s.text.chars().count(),
                    tables: doc.tables.iter().filter(|t| holds(s, t)).count(),
                })
                .collect()
        });
        Ok(to_json(&Outline {
            filing: FilingMeta::new(&filing, &doc),
            total_chars: doc.total_chars,
            tables: doc.tables.len(),
            next,
            sections,
            tables_outside_items: outside,
        }))
    }

    #[tool(
        description = "Read one Item section of a 10-K, 10-Q or 8-K as clean text, in pages. Get section ids from \
            get_filing_outline (for example item_1a Risk Factors and item_7 MD&A in a 10-K, part_1_item_2 MD&A in \
            a 10-Q, item_2.02 results in an 8-K). Returns a JSON header line, then up to max_chars characters \
            starting at offset. When more remains, next_offset in the header is set: call again with \
            offset=next_offset to continue.",
        annotations(title = "Read section", read_only_hint = true, open_world_hint = true)
    )]
    async fn read_section(&self, Parameters(p): Parameters<ReadSectionParams>) -> Result<String, String> {
        let (filing, doc) = self.doc(&p.company, p.accession.as_deref(), p.form.as_deref(), None).await?;
        let Some(sections) = &doc.sections else {
            return Err(format!("{} filings have no Item sections; use read_filing_text instead", filing.form));
        };
        if sections.is_empty() {
            return Err("no Item headings were found in this filing; use read_filing_text".into());
        }
        let section = find_section(sections, &p.section)?;
        let page = paginate(&section.text, p.offset.unwrap_or(0), max_chars(p.max_chars))?;
        Ok(page_response(FilingMeta::new(&filing, &doc), Some(section), &page, "read_section"))
    }

    #[tool(
        description = "Read the full clean text of a filing's primary document, or of one exhibit given as \
            document, in pages: a JSON header line, then up to max_chars characters starting at offset, with \
            next_offset set when more remains. Use it for forms without Item sections (S-1, DEF 14A, 20-F, 6-K, \
            ...), when get_filing_outline finds none, and for exhibit hits from search_full_text (pass the hit's \
            accession and document); for 10-K, 10-Q and 8-K prefer read_section.",
        annotations(title = "Read filing text", read_only_hint = true, open_world_hint = true)
    )]
    async fn read_filing_text(&self, Parameters(p): Parameters<ReadTextParams>) -> Result<String, String> {
        let (filing, doc) =
            self.doc(&p.company, p.accession.as_deref(), p.form.as_deref(), p.document.as_deref()).await?;
        let page = paginate(&doc.text, p.offset.unwrap_or(0), max_chars(p.max_chars))?;
        Ok(page_response(FilingMeta::new(&filing, &doc), None, &page, "read_filing_text"))
    }

    #[tool(
        description = "Read the tables of a filing's primary document, or of one exhibit given as document, as \
            JSON rows of cells: financial statements, schedules and any other table, with `$`, `)` and `%` joined \
            to their numbers as in the text, and empty cells left out. Pass section (an id from \
            get_filing_outline) for only the tables of that Item. A 10-K's financial statements are often in \
            item_8, but many filings print them after the last Item or in an attached annual report: \
            get_filing_outline lists those tables under tables_outside_items, and you read them without section, \
            from offset=first_index. Each table has its index (counting every table of the document), its \
            section, text_before (the line just above it, often a title or units such as \"(in millions)\", when \
            that line is short and not part of another table), text_offset (its character position for \
            read_filing_text), header_rows when the filing marks them, and rows. Returns tables until their \
            JSON reaches max_chars characters; when more remain, call again with offset=next_offset, plus \
            row=next_row and char=next_char when they are set. A table longer than max_chars is split between \
            rows, and a row longer than max_chars alone is cut inside a cell: join the last cell of one part \
            with the first cell of the next. Plain-text filings (most filings before 2001) have their tables only \
            in the text: read those with read_section or read_filing_text.",
        annotations(title = "Read tables", read_only_hint = true, open_world_hint = true)
    )]
    async fn read_tables(&self, Parameters(p): Parameters<ReadTablesParams>) -> Result<String, String> {
        let (filing, doc) =
            self.doc(&p.company, p.accession.as_deref(), p.form.as_deref(), p.document.as_deref()).await?;
        let section = match p.section.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => None,
            Some(wanted) => match &doc.sections {
                Some(sections) if sections.is_empty() => {
                    return Err(format!(
                        "no Item headings were found in this filing; leave out section to read all {} tables",
                        doc.tables.len()
                    ));
                }
                Some(sections) => Some(find_section(sections, wanted)?),
                None if doc.document.is_some() => {
                    return Err("exhibits have no Item sections; leave out section to read all their tables".into());
                }
                None => {
                    return Err(format!(
                        "{} filings have no Item sections; leave out section to read all tables",
                        filing.form
                    ));
                }
            },
        };
        let at = TableCursor { offset: p.offset.unwrap_or(0), row: p.row.unwrap_or(0), char: p.char.unwrap_or(0) };
        let page = table_page(&doc, section, at, max_chars(p.max_chars))?;
        Ok(to_json(&TablePageResponse {
            filing: FilingMeta::new(&filing, &doc),
            section: section.map(|s| s.id.as_str()),
            title: section.map(|s| s.title.as_str()),
            page,
        }))
    }

    #[tool(
        description = "Search the text of all EDGAR filings since 2001 with the SEC's full-text search. Words must \
            all appear; \"double quotes\" match a phrase, OR between terms matches either, -word excludes. Filter by \
            forms (a form also matches its amendments), filing dates and company. Returns matching documents (main \
            documents and exhibits), best match first, with company, CIK, form, filing date, accession and \
            document URL. To read a hit in a main document, pass its cik as company and its accession to \
            get_filing_outline, then read_section (or read_filing_text). A hit in an exhibit has document_type \
            and document set: pass its cik, accession and document to read_filing_text, since the matching text \
            is not in the main document. Page with offset and next_offset.",
        annotations(title = "Full-text search", read_only_hint = true, open_world_hint = true)
    )]
    async fn search_full_text(&self, Parameters(p): Parameters<SearchParams>) -> Result<String, String> {
        let limit = p.limit.unwrap_or(DEFAULT_HITS).clamp(1, FULL_TEXT_PAGE_SIZE);
        let offset = p.offset.unwrap_or(0);
        let client = self.client.clone();
        let mut query =
            FullTextQuery { query: p.query, forms: p.forms, since: p.since, until: p.until, ..Default::default() };
        query.offset = offset;
        let company = p.company.filter(|c| !c.trim().is_empty());
        let results = blocking(move || {
            if let Some(company) = company {
                let cik = match parse_cik(company.trim()) {
                    Some(cik) => cik,
                    None => client.company(&company).map_err(explain)?.cik,
                };
                query.ciks.push(cik);
            }
            client.full_text_search(&query).map_err(explain)
        })
        .await?;
        let (shown, next_offset) = search_window(offset, limit, &results);
        let hits: Vec<_> = results.hits.iter().take(shown).map(SearchRow::from).collect();
        // EFTS stops counting at 10,000: show "10000+" then.
        let total = if results.total_is_exact {
            serde_json::json!(results.total)
        } else {
            serde_json::json!(format!("{}+", results.total))
        };
        Ok(to_json(&SearchPage { total, offset, next_offset, hits }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for EdgarServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("parsec-edgar", env!("CARGO_PKG_VERSION"))
                    .with_title("parsec-edgar: SEC EDGAR filings")
                    .with_website_url("https://pars.ec"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

impl EdgarServer {
    /// Resolve a filing and load one of its documents (the primary one unless
    /// `document` names another), from the cache when possible.
    async fn doc(
        &self,
        company: &str,
        accession: Option<&str>,
        form: Option<&str>,
        document: Option<&str>,
    ) -> Result<(Filing, Arc<Doc>), String> {
        let company = company.trim();
        if company.is_empty() {
            return Err("company is empty: pass a ticker or CIK (find_company looks them up)".into());
        }
        let document = document.map(str::trim).filter(|d| !d.is_empty());
        if let Some(name) = document {
            check_document_name(name)?;
            if accession.is_none_or(|a| a.trim().is_empty()) {
                return Err("document needs the accession of its filing, as given by search_full_text".into());
            }
        }
        let filing = self.filing(company, accession, form).await?;
        // The primary document named explicitly is read as the primary document.
        let attachment = document
            .filter(|name| !filing.primary_document.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(name)))
            .map(str::to_owned);
        let key = (filing.accession_number.clone(), attachment.clone().unwrap_or_default());
        let client = self.client.clone();
        let target = filing.clone();
        let doc = self.shared_doc(key, move || load_doc(&client, &target, attachment)).await?;
        Ok((filing, doc))
    }

    /// The parsed document for `key`, loading it with `load` unless it is cached.
    /// Concurrent calls for one document share a single download and parse.
    async fn shared_doc(
        &self,
        key: DocKey,
        load: impl FnOnce() -> Result<Doc, String> + Send + 'static,
    ) -> Result<Arc<Doc>, String> {
        let cell = {
            let mut cache = self.lock();
            if let Some(doc) = cache.docs.get(&key) {
                return Ok(doc.clone());
            }
            let loading = cache.loading.entry(key.clone()).or_default();
            loading.waiting += 1;
            loading.doc.clone()
        };
        let _waiting = Waiting { server: self, key: key.clone() };
        let init = || async move { blocking(move || load().map(Arc::new)).await };
        let loaded = cell.get_or_try_init(init).await.cloned();
        if let Ok(doc) = &loaded {
            self.lock().docs.put(key, doc.clone());
        }
        loaded
    }

    async fn filing(&self, company: &str, accession: Option<&str>, form: Option<&str>) -> Result<Filing, String> {
        let company_key = company.to_ascii_uppercase();
        let client = self.client.clone();
        let company = company.to_owned();
        if let Some(acc) = accession.map(str::trim).filter(|a| !a.is_empty()) {
            let key = (company_key, acc.replace('-', ""));
            if let Some(f) = self.lock().by_accession.get(&key) {
                return Ok(f.clone());
            }
            let acc = acc.to_owned();
            let filing = blocking(move || client.filing(&company, &acc).map_err(explain)).await?;
            self.lock().by_accession.put(key, filing.clone());
            return Ok(filing);
        }
        let form = form.map(str::trim).filter(|f| !f.is_empty()).unwrap_or("10-K").to_owned();
        let key = (company_key, form.to_ascii_uppercase());
        if let Some(f) = self.lock().fresh_latest(&key, Instant::now()) {
            return Ok(f);
        }
        let filing = blocking(move || client.latest(&company, &form).map_err(explain)).await?;
        self.lock().latest.put(key, (Instant::now(), filing.clone()));
        Ok(filing)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// One call waiting for a document load. Dropping it, when the call returns or
/// is cancelled, removes the load from `loading` once no other call waits for it.
struct Waiting<'a> {
    server: &'a EdgarServer,
    key: DocKey,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        let mut cache = self.server.lock();
        if let Some(loading) = cache.loading.get_mut(&self.key) {
            loading.waiting -= 1;
            if loading.waiting == 0 {
                cache.loading.remove(&self.key);
            }
        }
    }
}

/// Download and parse the primary document of a filing, or the attachment named `attachment`.
fn load_doc(client: &Client, filing: &Filing, attachment: Option<String>) -> Result<Doc, String> {
    let Some(name) = attachment else {
        let raw = client.primary_document(filing).map_err(explain)?;
        return Ok(parse_doc(&raw, FormKind::from_form(&filing.form), None));
    };
    let raw = client.get_text(&format!("{}/{name}", filing.folder_url())).map_err(|e| match e {
        Error::Status { status: 404, .. } => format!(
            "filing {} has no document {name:?}; pass the document of a search_full_text hit, \
             or leave document out for the primary document",
            filing.accession_number
        ),
        e => explain(e),
    })?;
    Ok(parse_doc(&raw, None, Some(name)))
}

/// Text, tables and, for a form with Items, sections of one downloaded document.
fn parse_doc(raw: &str, kind: Option<FormKind>, document: Option<String>) -> Doc {
    let (text, tables) = document_to_text_and_tables(raw);
    let sections = kind.map(|kind| extract_sections(&text, kind));
    Doc { document, total_chars: text.chars().count(), text, sections, tables, html: document_is_html(raw) }
}

/// A document is a file name from a search hit, such as `ex99-1.htm`: no paths,
/// and only formats that convert to text.
fn check_document_name(name: &str) -> Result<(), String> {
    let plain = name.len() <= 200
        && !name.starts_with('.')
        && !name.contains("..")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if !plain {
        return Err(format!("document must be a file name from search_full_text, such as ex99-1.htm, not {name:?}"));
    }
    let ext = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    if !matches!(ext.as_deref(), Some("htm" | "html" | "txt" | "xml")) {
        return Err(format!("{name:?} is not a text document; only .htm, .html, .txt and .xml files can be read"));
    }
    Ok(())
}

/// Run blocking client work off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| format!("internal error: {e}"))?
}

/// Turn a library error into a message that tells the model what to do next.
fn explain(e: Error) -> String {
    match e {
        Error::CompanyNotFound(q) => {
            format!("no company with ticker or CIK {q:?}; use find_company to search by name")
        }
        Error::FilingNotFound(what) => {
            format!("filing not found: {what}. Use list_filings to see this company's filings and accession numbers")
        }
        Error::Status { status: 429, .. } | Error::Status { status: 403, .. } => format!(
            "{e}. The SEC is throttling requests; wait a minute before retrying, and check that \
             PARSEC_EDGAR_USER_AGENT names you and a contact email"
        ),
        Error::Status { status: 404, url } => match submissions_cik(&url) {
            Some(cik) => format!("no EDGAR filer has CIK {cik}; use find_company to search by name or ticker"),
            None => format!("the SEC has no document at {url} (HTTP 404)"),
        },
        other => other.to_string(),
    }
}

/// The CIK of a submissions API URL, which answers 404 for a CIK that does not exist.
fn submissions_cik(url: &str) -> Option<u64> {
    url.strip_prefix("https://data.sec.gov/submissions/CIK")?.strip_suffix(".json")?.parse().ok()
}

/// The query list_filings sends. One row past `limit` shows whether more filings
/// match. Older history pages are read only when the recent block has too few.
fn list_query(forms: Vec<String>, since: Option<String>, until: Option<String>, limit: usize) -> FilingQuery {
    FilingQuery { forms, since, until, limit: Some(limit + 1), all_history: true }
}

fn list_note(found: usize, limit: usize) -> Option<&'static str> {
    if found > limit {
        Some("more filings match: raise limit or narrow with since/until")
    } else if found == 0 {
        Some("no filings match these filters")
    } else {
        None
    }
}

/// How many of a page's hits to show for `limit`, and the offset of the next page.
/// Offsets count every hit EFTS returned, including those it left out as unreadable.
fn search_window(offset: usize, limit: usize, results: &FullTextResults) -> (usize, Option<usize>) {
    let returned = results.hits.len() + results.skipped.len();
    let (mut shown, mut used) = (0, 0);
    while used < returned && (shown < limit || results.skipped.contains(&used)) {
        if !results.skipped.contains(&used) {
            shown += 1;
        }
        used += 1;
    }
    let end = offset + used;
    let more = used > 0 && (end as u64) < results.total && end < FULL_TEXT_MAX_RESULTS;
    (shown, more.then_some(end))
}

#[derive(Serialize)]
struct CompanyMatches {
    matches: Vec<CompanyMatch>,
}

#[derive(Serialize)]
struct CompanyMatch {
    cik: u64,
    name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tickers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exchange: Option<String>,
}

impl From<&Company> for CompanyMatch {
    fn from(c: &Company) -> Self {
        Self {
            cik: c.cik,
            name: c.name.clone(),
            tickers: c.ticker.iter().cloned().collect(),
            exchange: c.exchange.clone(),
        }
    }
}

/// Rank companies against a ticker or name query and group share classes by CIK.
/// Ties keep the ticker file's order, which lists larger companies first.
fn search_companies(all: &[Company], query: &str, limit: usize) -> Vec<CompanyMatch> {
    let ticker = query.trim().to_ascii_uppercase().replace(['.', '/'], "-");
    let q = normalize_name(query);
    if q.is_empty() {
        return Vec::new();
    }
    let q_words: Vec<&str> = q.split(' ').collect();
    let rank = |c: &Company| -> Option<u8> {
        if c.ticker.as_deref() == Some(ticker.as_str()) {
            return Some(0);
        }
        let name = normalize_name(&c.name);
        if name == q {
            Some(1)
        } else if name.starts_with(&format!("{q} ")) {
            Some(2)
        } else if format!(" {name} ").contains(&format!(" {q} ")) {
            Some(3)
        } else if name.contains(&q) {
            Some(4)
        } else if q_words.iter().all(|w| name.split(' ').any(|n| n.starts_with(w))) {
            Some(5)
        } else {
            None
        }
    };
    // (rank, first position in file) per CIK.
    let mut best: HashMap<u64, (u8, usize)> = HashMap::new();
    for (i, c) in all.iter().enumerate() {
        if let Some(r) = rank(c) {
            let e = best.entry(c.cik).or_insert((r, i));
            if r < e.0 {
                e.0 = r;
            }
        }
    }
    let mut ciks: Vec<(u64, (u8, usize))> = best.into_iter().collect();
    ciks.sort_by_key(|&(_, key)| key);
    ciks.truncate(limit);
    ciks.into_iter()
        .map(|(cik, _)| {
            let rows: Vec<&Company> = all.iter().filter(|c| c.cik == cik).collect();
            let mut m = CompanyMatch::from(rows[0]);
            m.tickers = rows.iter().filter_map(|c| c.ticker.clone()).collect();
            m
        })
        .collect()
}

/// Lowercase, punctuation to spaces, single spaces: "Apple Inc." -> "apple inc".
fn normalize_name(s: &str) -> String {
    let mapped: String = s.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' }).collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `320193`, `0000320193` or `CIK0000320193`.
fn parse_cik(q: &str) -> Option<u64> {
    let digits = q.strip_prefix("CIK").or_else(|| q.strip_prefix("cik")).unwrap_or(q);
    if digits.is_empty() || digits.len() > 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

#[derive(Serialize)]
struct FilingRow {
    accession: String,
    form: String,
    filed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    period: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    items: Vec<String>,
}

impl From<&Filing> for FilingRow {
    fn from(f: &Filing) -> Self {
        Self {
            accession: f.accession_number.clone(),
            form: f.form.clone(),
            filed: f.filing_date.clone(),
            period: f.report_date.clone(),
            items: f.items.clone(),
        }
    }
}

#[derive(Serialize)]
struct SearchRow {
    company: String,
    cik: u64,
    form: String,
    filed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    period: Option<String>,
    accession: String,
    /// Exhibit type such as EX-10.1, when the hit is not the main document.
    #[serde(skip_serializing_if = "Option::is_none")]
    document_type: Option<String>,
    /// File name of that exhibit, for the document parameter of read_filing_text.
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<String>,
    url: String,
}

impl From<&FullTextHit> for SearchRow {
    fn from(h: &FullTextHit) -> Self {
        // The main document's type is the form, or the form without "/A" in an amendment.
        let is_main = h.document_type.eq_ignore_ascii_case(&h.form)
            || h.form.split('/').next().is_some_and(|base| h.document_type.eq_ignore_ascii_case(base));
        Self {
            company: h.company.clone(),
            cik: h.cik,
            form: h.form.clone(),
            filed: h.filing_date.clone(),
            period: h.report_date.clone(),
            accession: h.accession_number.clone(),
            document_type: (!is_main).then(|| h.document_type.clone()),
            document: (!is_main).then(|| h.document.clone()),
            url: h.document_url.clone(),
        }
    }
}

#[derive(Serialize)]
struct FilingList {
    company: Option<String>,
    cik: Option<u64>,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'static str>,
    filings: Vec<FilingRow>,
}

#[derive(Serialize)]
struct SearchPage {
    /// A number, or a string such as "10000+" when EFTS stopped counting.
    total: serde_json::Value,
    offset: usize,
    next_offset: Option<usize>,
    hits: Vec<SearchRow>,
}

/// Filing metadata shared by the outline and the page headers.
#[derive(Serialize)]
struct FilingMeta<'a> {
    company: &'a str,
    cik: u64,
    form: &'a str,
    filed: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    period: Option<&'a str>,
    accession: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    items: &'a [String],
    /// The exhibit or attachment read, when it is not the primary document.
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<&'a str>,
    url: String,
}

impl<'a> FilingMeta<'a> {
    /// Metadata of the filing as this call resolved it, for one of its documents.
    fn new(f: &'a Filing, doc: &'a Doc) -> Self {
        let mut meta = Self::from(f);
        if let Some(name) = &doc.document {
            meta.document = Some(name);
            meta.url = format!("{}/{name}", f.folder_url());
        }
        meta
    }
}

impl<'a> From<&'a Filing> for FilingMeta<'a> {
    fn from(f: &'a Filing) -> Self {
        Self {
            company: &f.company,
            cik: f.cik,
            form: &f.form,
            filed: &f.filing_date,
            period: f.report_date.as_deref(),
            accession: &f.accession_number,
            items: &f.items,
            document: None,
            url: f.primary_document_url.clone().unwrap_or_else(|| f.full_submission_url()),
        }
    }
}

#[derive(Serialize)]
struct Outline<'a> {
    #[serde(flatten)]
    filing: FilingMeta<'a>,
    total_chars: usize,
    tables: usize,
    next: String,
    /// Absent for forms without Item sections.
    #[serde(skip_serializing_if = "Option::is_none")]
    sections: Option<Vec<SectionRow<'a>>>,
    /// Tables in no Item section, when the filing has Item sections.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tables_outside_items: Vec<OutsideRun<'a>>,
}

#[derive(Serialize)]
struct SectionRow<'a> {
    id: &'a str,
    title: &'a str,
    chars: usize,
    tables: usize,
}

/// Consecutive tables in no Item section: before the first Item, between two
/// Items, or after the last one.
#[derive(Debug, PartialEq, Serialize)]
struct OutsideRun<'a> {
    /// The Item before the tables; absent before the first Item.
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<&'a str>,
    /// The Item after the tables; absent after the last Item.
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<&'a str>,
    tables: usize,
    /// Index of the first of these tables, the offset to pass to read_tables.
    first_index: usize,
}

/// The tables that no section holds, grouped by the gap between sections they are in.
fn outside_runs<'a>(sections: &'a [Section], tables: &[Table]) -> Vec<OutsideRun<'a>> {
    let mut runs: Vec<OutsideRun<'a>> = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        if sections.iter().any(|s| holds(s, t)) {
            continue;
        }
        let after = sections.iter().filter(|s| s.end <= t.start).max_by_key(|s| s.end).map(|s| s.id.as_str());
        let before = sections.iter().filter(|s| s.start > t.start).min_by_key(|s| s.start).map(|s| s.id.as_str());
        match runs.last_mut() {
            Some(run) if run.after == after && run.before == before => run.tables += 1,
            _ => runs.push(OutsideRun { after, before, tables: 1, first_index: i }),
        }
    }
    runs
}

/// How many runs of tables in no Item section the note of a section without tables names.
const MAX_RUNS_NOTED: usize = 4;

const PLAIN_TEXT_NOTE: &str = "this is a plain-text document: its tables are only in its text, \
    so read them with read_filing_text or read_section";

/// The note for a section without tables: where the tables in no Item section are.
fn empty_section_note(doc: &Doc) -> String {
    let runs = doc.sections.as_deref().map(|s| outside_runs(s, &doc.tables)).unwrap_or_default();
    if runs.is_empty() {
        return "no tables in this section; get_filing_outline gives the number of tables in each section".into();
    }
    let places: Vec<String> = runs
        .iter()
        .take(MAX_RUNS_NOTED)
        .map(|r| {
            let place = match (r.after, r.before) {
                (None, Some(b)) => format!("before {b}"),
                (Some(a), Some(b)) => format!("between {a} and {b}"),
                (Some(a), None) => format!("after {a}"),
                (None, None) => "outside the Items".to_owned(),
            };
            format!("{} {place} from index {}", r.tables, r.first_index)
        })
        .collect();
    let more = if runs.len() > MAX_RUNS_NOTED { "; more in get_filing_outline's tables_outside_items" } else { "" };
    format!(
        "no tables in this section. Tables in no Item section: {}{more}. To read them, call read_tables \
         without section and with offset set to one of those indexes",
        places.join("; ")
    )
}

#[derive(Serialize)]
struct TablePageResponse<'a> {
    #[serde(flatten)]
    filing: FilingMeta<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(flatten)]
    page: TablePage<'a>,
}

/// One page of a document's tables.
#[derive(Debug, Serialize)]
struct TablePage<'a> {
    /// Tables in the document, or in the section asked for.
    total_tables: usize,
    offset: usize,
    /// Index of the table to continue at. Null on the last page.
    next_offset: Option<usize>,
    /// Set when the table at next_offset was split: the row to continue at.
    #[serde(skip_serializing_if = "Option::is_none")]
    next_row: Option<usize>,
    /// Set when that row was cut inside a cell: the character of the row to continue at.
    #[serde(skip_serializing_if = "Option::is_none")]
    next_char: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    tables: Vec<TableOut<'a>>,
}

#[derive(Debug, Serialize)]
struct TableOut<'a> {
    /// Position among all tables of the document.
    index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<&'a str>,
    /// Character offset of the table in the document text, as read_filing_text counts.
    text_offset: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_before: Option<&'a str>,
    /// Indexes, in the whole table, of the header rows shown.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    header_rows: Vec<usize>,
    /// Set when only part of the table is shown: the index of its first row shown...
    #[serde(skip_serializing_if = "Option::is_none")]
    first_row: Option<usize>,
    /// ...and its number of rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    total_rows: Option<usize>,
    /// Set when the first row shown continues a row cut on the page before: how
    /// many characters of that row, counting its cells one after another, came
    /// before. Its first cell continues the last cell of that page.
    #[serde(skip_serializing_if = "Option::is_none")]
    first_char: Option<usize>,
    rows: Vec<Cow<'a, [String]>>,
}

/// Where a page of tables starts: the index of a table in the document, a row
/// of that table, and a character of that row.
#[derive(Clone, Copy, Debug, Default)]
struct TableCursor {
    offset: usize,
    row: usize,
    char: usize,
}

/// What `,"header_rows":[]` adds to a table's JSON besides its indexes: 17
/// characters, less the one comma too many when each index counts a comma.
const HEADER_ROWS_KEY: usize = 16;

/// Whether a section holds a table: a table belongs to the section its start is in.
fn holds(section: &Section, table: &Table) -> bool {
    section.start <= table.start && table.start < section.end
}

/// The tables of `doc` (those in `section` when given) from `at`, up to `max`
/// characters of JSON in the list of tables. A table that does not fit waits
/// for the next page, unless it is the first one, which is then split after as
/// many rows as fit. When not even one row fits, the row is cut inside a cell.
/// With `max` of at least MIN_MAX_CHARS every page fits; below that a page
/// still shows one character, so that paging always moves on.
fn table_page<'a>(
    doc: &'a Doc,
    section: Option<&'a Section>,
    at: TableCursor,
    max: usize,
) -> Result<TablePage<'a>, String> {
    let TableCursor { offset, row, char: skip } = at;
    let total = doc.tables.len();
    if offset > total {
        let indexes = if total > 0 { format!(" (indexes 0 to {})", total - 1) } else { String::new() };
        return Err(format!(
            "offset {offset} is past the last table: the document has {total} tables{indexes}, and offset \
             counts every table of the document, also when a section is given"
        ));
    }
    let selected: Vec<usize> = (0..total).filter(|&i| section.is_none_or(|s| holds(s, &doc.tables[i]))).collect();
    let mut page = TablePage {
        total_tables: selected.len(),
        offset,
        next_offset: None,
        next_row: None,
        next_char: None,
        note: None,
        tables: Vec::new(),
    };
    let (Some(&first_selected), Some(&last_selected)) = (selected.first(), selected.last()) else {
        page.note = Some(if !doc.html {
            PLAIN_TEXT_NOTE.to_owned()
        } else if doc.tables.is_empty() {
            "no tables in this document".to_owned()
        } else {
            empty_section_note(doc)
        });
        return Ok(page);
    };
    // row and char belong to the table at offset, which must be one of those asked for.
    if row > 0 || skip > 0 {
        if selected.binary_search(&offset).is_err() {
            return Err(match section {
                Some(s) if offset < total => format!(
                    "row and char apply to the table at offset, and table {offset} is not in {}, whose tables are \
                     at indexes {first_selected} to {last_selected}",
                    s.id
                ),
                _ => format!("row and char apply to the table at offset, and there is no table {offset}"),
            });
        }
        let rows = &doc.tables[offset].rows;
        if row >= rows.len() {
            return Err(format!("row {row} is past the last row of table {offset}, which has {}", rows.len()));
        }
        let len = row_chars(&rows[row]);
        if skip > 0 && skip >= len {
            return Err(format!("char {skip} is past the end of row {row} of table {offset}, which has {len}"));
        }
    }
    // The brackets around the list of tables.
    let mut used = 2;
    let (mut chars, mut char_at) = (0, 0);
    for &i in selected.iter().filter(|&&i| i >= offset) {
        let t = &doc.tables[i];
        let first_on_page = page.tables.is_empty();
        let (first, skip) = if first_on_page { (row, skip) } else { (0, 0) };
        chars += doc.text[char_at..t.start].chars().count();
        char_at = t.start;
        let partial = first > 0 || skip > 0;
        let mut out = TableOut {
            index: i,
            section: doc.sections.as_deref().and_then(|s| s.iter().find(|s| holds(s, t))).map(|s| s.id.as_str()),
            text_offset: chars,
            text_before: text_before(doc, t),
            header_rows: Vec::new(),
            first_row: partial.then_some(first),
            total_rows: partial.then_some(t.rows.len()),
            first_char: (skip > 0).then_some(skip),
            rows: Vec::new(),
        };
        // The rest of a row cut on the page before.
        let first_part = (skip > 0).then(|| row_piece(&t.rows[first], skip, row_chars(&t.rows[first])));
        let row_at = |k: usize| -> Cow<'a, [String]> {
            match (&first_part, k) {
                (Some(part), 0) => Cow::Owned(part.clone()),
                _ => Cow::Borrowed(&t.rows[first + k]),
            }
        };
        let n = t.rows.len() - first;
        let header = |k: usize| t.header_rows.binary_search(&(first + k)).is_ok();
        let header_cost = |k: usize| if header(k) { digits(first + k) + 1 } else { 0 };
        // How many rows fit beside `meta` characters, and the characters they all take: each row
        // adds its JSON and a comma, a header row its index and a comma, and the first header row
        // the header_rows key. Stops at the first row that does not fit.
        let fitting = |meta: usize, room: usize| {
            let (mut size, mut take, mut any_header) = (meta, 0, false);
            while take < n {
                let row_json = match (&first_part, take) {
                    (Some(part), 0) => json_chars(part),
                    _ => json_chars(&t.rows[first + take]),
                };
                let key = if header(take) && !any_header { HEADER_ROWS_KEY } else { 0 };
                let add = row_json + 1 + header_cost(take) + key;
                if size + add > room {
                    break;
                }
                size += add;
                any_header |= header(take);
                take += 1;
            }
            (take, size)
        };
        let comma = usize::from(!first_on_page);
        let left = max.saturating_sub(used + comma);
        let (take, size) = fitting(json_chars(&out), left);
        if take == n {
            out.header_rows = (0..n).filter(|&k| header(k)).map(|k| first + k).collect();
            out.rows = (0..n).map(row_at).collect();
            page.tables.push(out);
            used += size + comma;
            continue;
        }
        if !first_on_page {
            page.next_offset = Some(i);
            break;
        }
        out.first_row = Some(first);
        out.total_rows = Some(t.rows.len());
        // The title gives way when the smallest part of a row would not fit beside it.
        let first_header = header_cost(0) + if header(0) { HEADER_ROWS_KEY } else { 0 };
        let smallest = json_chars(&row_piece(&t.rows[first], skip, skip + 1)) + first_header;
        if json_chars(&out) + smallest > left {
            out.text_before = None;
        }
        let meta = json_chars(&out);
        let (take, _) = fitting(meta, left);
        let (next_row, next_char) = if take > 0 {
            out.header_rows = (0..take).filter(|&k| header(k)).map(|k| first + k).collect();
            out.rows = (0..take).map(row_at).collect();
            (first + take, 0)
        } else {
            // Not even one row fits: cut it inside a cell.
            let r = &t.rows[first];
            let end = cut_row(r, skip, left.saturating_sub(meta + first_header));
            out.header_rows = if header(0) { vec![first] } else { Vec::new() };
            out.rows = vec![Cow::Owned(row_piece(r, skip, end))];
            if end < row_chars(r) { (first, end) } else { (first + 1, 0) }
        };
        used += json_chars(&out);
        page.tables.push(out);
        if next_row < t.rows.len() {
            page.next_offset = Some(i);
            page.next_row = Some(next_row);
            page.next_char = (next_char > 0).then_some(next_char);
            break;
        }
    }
    if page.tables.is_empty() {
        page.note = Some(match section {
            Some(s) => format!(
                "no tables of {} at or after index {offset}: its tables are at indexes {first_selected} to \
                 {last_selected}",
                s.id
            ),
            None => format!("no tables at or after index {offset}: the last table is at index {last_selected}"),
        });
    }
    Ok(page)
}

/// Characters of a row, counting its cells one after another.
fn row_chars(row: &[String]) -> usize {
    row.iter().map(|c| c.chars().count()).sum()
}

/// The part of `row` from character `from` to `to`, counting its cells one
/// after another. A part that ends between two cells ends with an empty cell,
/// so the last cell of one part always joins the first cell of the next.
fn row_piece(row: &[String], from: usize, to: usize) -> Vec<String> {
    let total = row_chars(row);
    let mut piece = Vec::new();
    let mut at = 0;
    for cell in row {
        let len = cell.chars().count();
        if from < at + len && at < to {
            let start = from.saturating_sub(at);
            let end = (to - at).min(len);
            piece.push(cell.chars().skip(start).take(end - start).collect());
        }
        at += len;
        if at >= to {
            if at == to && to < total {
                piece.push(String::new());
            }
            break;
        }
    }
    piece
}

/// Where to cut `row`, from character `from`, so that the part takes at most
/// `budget` characters of JSON. The part has at least one character.
fn cut_row(row: &[String], from: usize, budget: usize) -> usize {
    let total = row_chars(row);
    let mut best = from + 1;
    // `[]`, then `""` for the first cell and `,""` for each other one.
    let mut cost = 2;
    let mut started = false;
    let mut at = 0;
    'cells: for cell in row {
        let len = cell.chars().count();
        if at + len <= from {
            at += len;
            continue;
        }
        cost += if started { 3 } else { 2 };
        started = true;
        for (k, c) in cell.chars().enumerate().skip(from.saturating_sub(at)) {
            cost += json_escaped_len(c);
            if cost > budget {
                break 'cells;
            }
            let to = at + k + 1;
            // A cut between two cells adds the empty cell `,""`.
            let end = if k + 1 == len && to < total { 3 } else { 0 };
            if cost + end <= budget {
                best = to;
            }
        }
        at += len;
    }
    best
}

/// Characters that `c` takes in a JSON string, as serde_json writes it.
fn json_escaped_len(c: char) -> usize {
    match c {
        '"' | '\\' | '\u{8}' | '\u{c}' | '\n' | '\r' | '\t' => 2,
        c if u32::from(c) < 0x20 => 6,
        _ => 1,
    }
}

/// Decimal digits of `n`.
fn digits(n: usize) -> usize {
    n.checked_ilog10().map_or(1, |d| d as usize + 1)
}

/// The line just above a table, when it is short and not part of another
/// table: often the table's title or its units.
fn text_before<'a>(doc: &'a Doc, table: &Table) -> Option<&'a str> {
    let above = doc.text[..table.start].strip_suffix('\n')?;
    let line_start = above.rfind('\n').map_or(0, |i| i + 1);
    let line = &above[line_start..];
    let in_table = doc.tables.iter().any(|t| t.start <= line_start && line_start < t.end);
    (!line.is_empty() && !in_table && line.chars().count() <= MAX_TEXT_BEFORE).then_some(line)
}

#[derive(Serialize)]
struct PageHeader<'a> {
    #[serde(flatten)]
    filing: FilingMeta<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    offset: usize,
    end: usize,
    total_chars: usize,
    /// Null on the last page.
    next_offset: Option<usize>,
}

fn max_chars(requested: Option<usize>) -> usize {
    requested.unwrap_or(DEFAULT_MAX_CHARS).clamp(MIN_MAX_CHARS, MAX_MAX_CHARS)
}

/// One page of a longer text. Offsets count characters, not bytes.
#[derive(Debug, PartialEq)]
struct Page<'a> {
    text: &'a str,
    offset: usize,
    end: usize,
    total: usize,
    next_offset: Option<usize>,
}

/// Up to `max` characters of `text` from character `offset`. A page that does not
/// reach the end breaks after a paragraph, a line or a space in its last fifth
/// when there is one, so words and lines stay whole.
fn paginate(text: &str, offset: usize, max: usize) -> Result<Page<'_>, String> {
    let total = text.chars().count();
    if offset > total {
        return Err(format!("offset {offset} is past the end of the text ({total} characters)"));
    }
    let start_byte = byte_index(text, offset);
    let rest = &text[start_byte..];
    let window_end = byte_index(rest, max);
    if window_end == rest.len() {
        return Ok(Page { text: rest, offset, end: total, total, next_offset: None });
    }
    let window = &rest[..window_end];
    let tail_start = byte_index(window, max - max / 5);
    let tail = &window[tail_start..];
    let cut = tail
        .rfind("\n\n")
        .map(|i| i + 2)
        .or_else(|| tail.rfind('\n').map(|i| i + 1))
        .or_else(|| tail.rfind(' ').map(|i| i + 1))
        .map_or(window_end, |i| tail_start + i);
    let page = &rest[..cut];
    let end = offset + page.chars().count();
    Ok(Page { text: page, offset, end, total, next_offset: Some(end) })
}

/// Byte index of character `n` in `s`, or `s.len()` past the end.
fn byte_index(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

/// A JSON header line with the page position, the text, and a reminder when more remains.
fn page_response(filing: FilingMeta<'_>, section: Option<&Section>, page: &Page<'_>, tool: &str) -> String {
    let header = PageHeader {
        filing,
        section: section.map(|s| s.id.as_str()),
        title: section.map(|s| s.title.as_str()),
        offset: page.offset,
        end: page.end,
        total_chars: page.total,
        next_offset: page.next_offset,
    };
    let mut out = to_json(&header);
    out.push_str("\n\n");
    out.push_str(page.text);
    if let Some(next) = page.next_offset {
        out.push_str(&format!(
            "\n\n[{} more characters. Call {tool} again with offset={next} to continue.]",
            page.total - next
        ));
    }
    out
}

fn to_json<T: Serialize + ?Sized>(v: &T) -> String {
    serde_json::to_string(v).expect("values serialize")
}

fn json_chars<T: Serialize + ?Sized>(v: &T) -> usize {
    to_json(v).chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    #[test]
    fn pages_cover_text_exactly() {
        let text = "Alpha beta gamma.\n\nDelta epsilon zeta eta theta.\nIota kappa lambda mu nu xi omicron pi rho.";
        let mut offset = 0;
        let mut joined = String::new();
        let mut pages = 0;
        loop {
            let p = paginate(text, offset, 20).unwrap();
            assert!(p.text.chars().count() <= 20);
            joined.push_str(p.text);
            pages += 1;
            match p.next_offset {
                Some(next) => {
                    assert!(next > offset);
                    offset = next;
                }
                None => break,
            }
        }
        assert_eq!(joined, text);
        assert!(pages > 3);
    }

    #[test]
    fn pages_break_at_paragraphs_lines_and_spaces() {
        let text = format!("{}\n\n{}", "a".repeat(90), "b".repeat(50));
        let p = paginate(&text, 0, 100).unwrap();
        assert_eq!(p.text, format!("{}\n\n", "a".repeat(90)));
        assert_eq!(p.next_offset, Some(92));

        let text = format!("{} {}", "a".repeat(85), "b".repeat(50));
        assert_eq!(paginate(&text, 0, 100).unwrap().end, 86);

        // No break in the last fifth: cut at the limit.
        let text = format!("{} {}", "a".repeat(10), "b".repeat(200));
        assert_eq!(paginate(&text, 0, 100).unwrap().end, 100);
    }

    #[test]
    fn pages_count_characters_not_bytes() {
        let text = "Ελληνικά κείμενα και €100 εκατομμύρια. ".repeat(10);
        let total = text.chars().count();
        let p = paginate(&text, 5, 30).unwrap();
        assert_eq!(p.offset, 5);
        assert!(p.end <= 35);
        assert_eq!(p.total, total);
        let last = paginate(&text, total - 3, 30).unwrap();
        assert_eq!(last.next_offset, None);
        assert_eq!(last.text.chars().count(), 3);
        assert_eq!(paginate(&text, total, 30).unwrap().text, "");
        assert!(paginate(&text, total + 1, 30).is_err());
    }

    #[test]
    fn page_response_shows_next_offset() {
        let text = "word ".repeat(100);
        let filing = filing();
        let risk = section("item_1a");
        let out = page_response((&filing).into(), Some(&risk), &paginate(&text, 0, 200).unwrap(), "read_section");
        let header: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
        assert_eq!(header["section"], "item_1a");
        assert_eq!(header["next_offset"], 200);
        assert_eq!(header["total_chars"], 500);
        assert!(out.ends_with("[300 more characters. Call read_section again with offset=200 to continue.]"));
        let last = page_response((&filing).into(), None, &paginate(&text, 400, 200).unwrap(), "read_filing_text");
        assert!(last.starts_with(r#"{"company":"Apple Inc.","cik":320193,"form":"10-K","filed":"2026-11-03","#));
        assert!(last.contains(r#""offset":400,"end":500,"total_chars":500,"next_offset":null}"#));
        assert!(!last.contains("more characters"));
    }

    fn filing() -> Filing {
        serde_json::from_value(serde_json::json!({
            "cik": 320193,
            "company": "Apple Inc.",
            "accession_number": "0000320193-26-000026",
            "form": "10-K",
            "filing_date": "2026-11-03",
            "report_date": "2026-09-26",
            "primary_document_url": "https://www.sec.gov/Archives/edgar/data/320193/000032019326000026/aapl-20260926.htm"
        }))
        .unwrap()
    }

    fn section(id: &str) -> Section {
        let s = serde_json::json!({
            "id": id, "item": "", "title": "Risk Factors", "heading": "", "start": 0, "end": 0, "text": ""
        });
        serde_json::from_value(s).unwrap()
    }

    fn company(cik: u64, name: &str, ticker: &str) -> Company {
        Company { cik, name: name.into(), ticker: Some(ticker.into()), exchange: Some("NYSE".into()) }
    }

    #[test]
    fn company_search() {
        let all = vec![
            company(320193, "Apple Inc.", "AAPL"),
            company(1067983, "BERKSHIRE HATHAWAY INC", "BRK-B"),
            company(1067983, "BERKSHIRE HATHAWAY INC", "BRK-A"),
            company(1, "Applied Materials Inc", "AMAT"),
            company(2, "Pineapple Holdings", "PNPL"),
            company(3, "Berkshire Hills Bancorp", "BHLB"),
            company(4, "Apple Hospitality REIT, Inc.", "APLE"),
        ];
        let names = |q: &str| search_companies(&all, q, 10).into_iter().map(|m| m.name).collect::<Vec<_>>();
        assert_eq!(names("apple"), ["Apple Inc.", "Apple Hospitality REIT, Inc.", "Pineapple Holdings"]);
        assert_eq!(names("brk.b"), ["BERKSHIRE HATHAWAY INC"]);
        // The exact name comes first, then names with every word.
        assert_eq!(names("apple inc"), ["Apple Inc.", "Apple Hospitality REIT, Inc."]);
        assert_eq!(names("berk hath"), ["BERKSHIRE HATHAWAY INC"]);
        assert!(names("zzz").is_empty());
        let berkshire = search_companies(&all, "berkshire", 10);
        assert_eq!(berkshire.len(), 2);
        assert_eq!(berkshire[0].tickers, vec!["BRK-B", "BRK-A"]);
        assert_eq!(search_companies(&all, "a", 2).len(), 2);
    }

    #[test]
    fn form_lists() {
        let p: ListFilingsParams = serde_json::from_str(r#"{"company":"AAPL","forms":"10-K, 10-Q"}"#).unwrap();
        assert_eq!(p.forms, ["10-K", "10-Q"]);
        let p: ListFilingsParams = serde_json::from_str(r#"{"company":"AAPL","forms":["8-K"]}"#).unwrap();
        assert_eq!(p.forms, ["8-K"]);
        let p: ListFilingsParams = serde_json::from_str(r#"{"company":"AAPL"}"#).unwrap();
        assert!(p.forms.is_empty());
    }

    fn filing_of(cik: u64, company: &str, accession: &str, document: &str) -> Filing {
        let folder = format!("https://www.sec.gov/Archives/edgar/data/{cik}/{}", accession.replace('-', ""));
        serde_json::from_value(serde_json::json!({
            "cik": cik,
            "company": company,
            "accession_number": accession,
            "form": "10-K",
            "filing_date": "2025-02-14",
            "primary_document": document,
            "primary_document_url": format!("{folder}/{document}"),
        }))
        .unwrap()
    }

    fn offline_server() -> EdgarServer {
        EdgarServer::new(Client::builder("parsec tests test@example.com").build().unwrap())
    }

    fn doc(text: &str, document: Option<&str>) -> Arc<Doc> {
        let sections = document.is_none().then(Vec::new);
        let document = document.map(str::to_owned);
        let tables = Vec::new();
        let total_chars = text.chars().count();
        Arc::new(Doc { document, total_chars, text: text.to_owned(), sections, tables, html: true })
    }

    /// A 10-K with Item 7 and Item 8, `n` tables of `rows` rows each, half of
    /// them in each Item, and a title line above each table.
    fn table_doc(n: usize, rows: usize) -> Doc {
        let mut html = String::from("<p>Item 7. Management’s Discussion</p>");
        for i in 0..n {
            if i == n / 2 {
                html += "<p>Item 8. Financial Statements</p>";
            }
            html += &format!("<p>Table {i} (in millions of €)</p><table>");
            for r in 0..rows {
                html += &format!("<tr><td>Row {r} of table {i}</td><td>$</td><td>{r},000</td></tr>");
            }
            html += "</table>";
        }
        let (text, tables) = parsec_edgar::html_to_text_and_tables(&html);
        let item_8 = text.find("Item 8.").unwrap();
        let section = |id: &str, start: usize, end: usize| -> Section {
            let s = serde_json::json!({
                "id": id, "item": "", "title": id, "heading": "", "start": start, "end": end,
                "text": text[start..end].trim()
            });
            serde_json::from_value(s).unwrap()
        };
        let sections = vec![section("item_7", 0, item_8), section("item_8", item_8, text.len())];
        Doc { document: None, total_chars: text.chars().count(), text, sections: Some(sections), tables, html: true }
    }

    /// A document parsed as load_doc parses one, as a 10-K when `ten_k`.
    fn parsed(raw: &str, ten_k: bool) -> Doc {
        parse_doc(raw, ten_k.then_some(FormKind::TenK), None)
    }

    fn at(offset: usize, row: usize, char: usize) -> TableCursor {
        TableCursor { offset, row, char }
    }

    /// A table's index and its rows.
    type Rows = (usize, Vec<Vec<String>>);

    /// Every page from the start, following next_offset, next_row and
    /// next_char: each table's rows with the parts of cut rows joined again,
    /// and the number of pages. Checks that each page keeps within `max`.
    fn all_pages(doc: &Doc, section: Option<&Section>, max: usize) -> (Vec<Rows>, usize) {
        let (mut tables, mut pages, mut cursor) = (Vec::<Rows>::new(), 0, at(0, 0, 0));
        loop {
            let page = table_page(doc, section, cursor, max).unwrap();
            pages += 1;
            assert!(json_chars(&page.tables) <= max, "{} > {max}: {page:?}", json_chars(&page.tables));
            for t in &page.tables {
                let mut rows = t.rows.iter().map(|r| r.to_vec());
                if t.first_row.is_some_and(|r| r > 0) || t.first_char.is_some() {
                    let (index, so_far) = tables.last_mut().expect("a continued table");
                    assert_eq!(*index, t.index);
                    assert_eq!(so_far.len(), t.first_row.unwrap() + usize::from(t.first_char.is_some()));
                    if t.first_char.is_some() {
                        // The first cell of this part continues the last cell of the part before.
                        let mut rest = rows.next().unwrap().into_iter();
                        let cut = so_far.last_mut().unwrap();
                        cut.last_mut().unwrap().push_str(&rest.next().unwrap());
                        cut.extend(rest);
                    }
                    so_far.extend(rows);
                } else {
                    tables.push((t.index, rows.collect()));
                }
            }
            match page.next_offset {
                Some(next) => cursor = at(next, page.next_row.unwrap_or(0), page.next_char.unwrap_or(0)),
                None => break,
            }
            assert!(pages < 100_000, "paging does not end");
        }
        (tables, pages)
    }

    /// The rows of all tables that all_pages returned, one after another.
    fn rows_of(tables: &[Rows]) -> Vec<Vec<String>> {
        tables.iter().flat_map(|(_, rows)| rows.clone()).collect()
    }

    #[test]
    fn table_pages_cover_every_row_once() {
        let doc = table_doc(10, 5);
        let every: Vec<Vec<String>> = doc.tables.iter().flat_map(|t| t.rows.clone()).collect();
        assert_eq!(every.len(), 50);
        assert_eq!(every[6], ["Row 1 of table 1", "$1,000"]);
        let (tables, pages) = all_pages(&doc, None, 600);
        assert_eq!(rows_of(&tables), every);
        assert!(pages >= 4, "{pages}");
        // One page when everything fits.
        let (tables, pages) = all_pages(&doc, None, 100_000);
        assert_eq!((rows_of(&tables), pages), (every, 1));
    }

    /// A table longer than max_chars comes in pieces that join up.
    #[test]
    fn long_tables_are_split_by_rows() {
        let doc = table_doc(2, 100);
        let page = table_page(&doc, None, at(0, 0, 0), 1000).unwrap();
        let [t] = &page.tables[..] else { panic!("{page:?}") };
        assert_eq!((t.first_row, t.total_rows), (Some(0), Some(100)));
        assert_eq!((page.next_offset, page.next_row, page.next_char), (Some(0), Some(t.rows.len()), None));
        let next = table_page(&doc, None, at(0, t.rows.len(), 0), 1000).unwrap();
        assert_eq!(next.tables[0].rows[0][0], format!("Row {} of table 0", t.rows.len()));
        assert_eq!(next.tables[0].first_row, Some(t.rows.len()));
        let (tables, _) = all_pages(&doc, None, 1000);
        let rows = rows_of(&tables);
        assert_eq!(rows.len(), 200);
        assert_eq!(rows[150], ["Row 50 of table 1", "$50,000"]);
        for max in [MIN_MAX_CHARS, 250, 333] {
            assert_eq!(rows_of(&all_pages(&doc, None, max).0), rows, "{max}");
        }
    }

    /// A row longer than max_chars on its own is cut inside a cell, and the
    /// parts join up again. Every page keeps within max_chars, also at the
    /// hard limit of 100,000.
    #[test]
    fn long_rows_are_cut_inside_cells() {
        // A layout table with one short line of cells keeps its long block rows.
        let layout = |paragraphs: usize| {
            let para = "<p>The Company recognizes revenue when control passes to the customer.</p>".repeat(paragraphs);
            parsed(
                &format!("<table><tr><td>Revenue</td><td>100</td></tr><tr><td>{para}</td><td>Note 2</td></tr></table>"),
                false,
            )
        };
        let (long, longest) = (layout(60), layout(1500));
        assert!(json_chars(&longest.tables[0].rows[1]) > MAX_MAX_CHARS);
        for (doc, max) in
            [(&long, MIN_MAX_CHARS), (&long, 1000), (&longest, DEFAULT_MAX_CHARS), (&longest, MAX_MAX_CHARS)]
        {
            let (tables, pages) = all_pages(doc, None, max);
            assert_eq!(tables, [(0, doc.tables[0].rows.clone())], "{max}");
            assert!(pages > 1, "{max}");
        }
        let doc = long;
        let page = table_page(&doc, None, at(0, 1, 0), 1000).unwrap();
        assert_eq!((page.next_offset, page.next_row), (Some(0), Some(1)));
        let cut = page.next_char.unwrap();
        assert!(cut > 500 && json_chars(&page.tables) <= 1000, "{cut}");
        let next = table_page(&doc, None, at(0, 1, cut), 1000).unwrap();
        assert_eq!((next.tables[0].first_row, next.tables[0].first_char), (Some(1), Some(cut)));
        assert_eq!(next.offset, 0);

        // Cells with characters JSON escapes, characters of several bytes, and
        // tabs and line breaks inside cells, cut at every possible budget.
        let row: Vec<String> =
            ["Résumé \"quoted\" \\ path", "a\tb\nc", "\u{1}\u{2} control", "€", "x", "y", "Total, all segments"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        let len = row_chars(&row);
        assert_eq!(row_piece(&row, 0, len), row);
        for from in 0..len {
            for budget in 0..=json_chars(&row) + 3 {
                let to = cut_row(&row, from, budget);
                assert!(to > from && to <= len, "{from} {budget} {to}");
                let piece = row_piece(&row, from, to);
                assert!(to == from + 1 || json_chars(&piece) <= budget, "{from} {budget} {piece:?}");
                // The rest of the row joins this part to give the row back.
                if to < len {
                    let mut joined = row_piece(&row, 0, to);
                    let mut rest = row_piece(&row, to, len).into_iter();
                    joined.last_mut().unwrap().push_str(&rest.next().unwrap());
                    joined.extend(rest);
                    assert_eq!(joined, row, "{to}");
                }
            }
        }
        // A part that ends between two cells ends with an empty cell.
        let first = row[0].chars().count();
        assert_eq!(row_piece(&row, 0, first), [row[0].as_str(), ""]);
        assert_eq!(row_piece(&row, first, first + 2), ["a\t"]);
    }

    /// The smallest page, at the smallest max_chars, holds one character of a
    /// row of a table whose numbers are as long as they get.
    #[test]
    fn smallest_pages_fit() {
        let piece = row_piece(&["\u{1}".to_owned(), "x".to_owned()], 0, 1);
        let out = TableOut {
            index: 99_999,
            section: Some("part_2_item_1a"),
            text_offset: 999_999_999,
            text_before: None,
            header_rows: vec![999_999],
            first_row: Some(999_999),
            total_rows: Some(999_999),
            first_char: Some(999_999_999),
            rows: vec![Cow::Owned(piece)],
        };
        let size = json_chars(&[out]);
        assert!(size <= MIN_MAX_CHARS, "{size}");
    }

    /// Continuation pages list only the header rows they show, so a table of
    /// header cells pages like any other.
    #[test]
    fn header_rows_of_the_rows_shown() {
        let table = |cell: &str| {
            let rows: String =
                (0..400).map(|r| format!("<tr><{cell}>Line {r}</{cell}><{cell}>{r}</{cell}></tr>")).collect();
            parsed(&format!("<table>{rows}</table>"), false)
        };
        let (th, td) = (table("th"), table("td"));
        assert_eq!(th.tables[0].header_rows.len(), 400);
        let (th_tables, th_pages) = all_pages(&th, None, 1000);
        let (td_tables, td_pages) = all_pages(&td, None, 1000);
        assert_eq!(th_tables, td_tables);
        assert!(th_pages <= td_pages * 2, "{th_pages} pages with <th>, {td_pages} without");
        let page = table_page(&th, None, at(0, 100, 0), 1000).unwrap();
        let t = &page.tables[0];
        let shown: Vec<usize> = (100..100 + t.rows.len()).collect();
        assert_eq!(t.header_rows, shown);
    }

    #[test]
    fn tables_of_one_section() {
        let doc = table_doc(6, 2);
        let item_8 = &doc.sections.as_ref().unwrap()[1];
        let page = table_page(&doc, Some(item_8), at(0, 0, 0), 100_000).unwrap();
        assert_eq!(page.total_tables, 3);
        // Indexes count every table of the document.
        assert_eq!(page.tables.iter().map(|t| t.index).collect::<Vec<_>>(), [3, 4, 5]);
        assert!(page.tables.iter().all(|t| t.section == Some("item_8")));
        let page = table_page(&doc, Some(item_8), at(0, 0, 0), 300).unwrap();
        assert_eq!((page.tables[0].index, page.next_offset), (3, Some(4)));
        let (tables, _) = all_pages(&doc, Some(item_8), 300);
        let rows = rows_of(&tables);
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0][0], "Row 0 of table 3");
    }

    /// A 10-K with a cover table, one table in each of Items 7, 8 and 9, and
    /// two tables after the last Item.
    fn ten_k_with_tables() -> Doc {
        let body = "<p>We make and sell things in many markets around the world, every year.</p>".repeat(30);
        let html = format!(
            "<table><tr><td>Cover</td><td>page</td></tr></table>\
             <p>Item 7. Management's Discussion and Analysis</p>{body}\
             <table><tr><td>Net sales</td><td>100</td></tr><tr><td>Cost</td><td>50</td></tr></table>{body}\
             <p>Item 8. Financial Statements and Supplementary Data</p>{body}\
             <table><tr><td>Total assets</td><td>500</td></tr><tr><td>Total liabilities</td><td>300</td></tr></table>\
             {body}<p>Item 9. Changes in and Disagreements with Accountants</p>{body}\
             <table><tr><td>Other</td><td>1</td></tr></table>{body}\
             <p>SIGNATURES</p><p>Pursuant to the requirements of the Act, the registrant has signed.</p>\
             <p>Consolidated Balance Sheets</p><table><tr><td>Cash</td><td>7</td></tr></table>\
             <p>Consolidated Statements of Income</p><table><tr><td>Revenue</td><td>9</td></tr></table>"
        );
        let doc = parsed(&html, true);
        let ids: Vec<&str> = doc.sections.iter().flatten().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["item_7", "item_8", "item_9"]);
        assert_eq!(doc.tables.len(), 6);
        doc
    }

    /// offset is an index among all tables of the document, also with a
    /// section, and row only applies to the table at offset.
    #[test]
    fn offsets_count_every_table() {
        let doc = ten_k_with_tables();
        let sections = doc.sections.as_deref().unwrap();
        let item_8 = &sections[1];
        let page = table_page(&doc, Some(item_8), at(2, 1, 0), 20_000).unwrap();
        assert_eq!(page.tables[0].rows[0].as_ref(), ["Total liabilities", "300"]);
        // row with an offset that is not a table of the section is refused.
        let err = table_page(&doc, Some(item_8), at(0, 1, 0), 20_000).unwrap_err();
        assert!(err.contains("table 0 is not in item_8") && err.contains("indexes 2 to 2"), "{err}");
        // An offset past the section's tables says so.
        let page = table_page(&doc, Some(item_8), at(3, 0, 0), 20_000).unwrap();
        assert!(page.tables.is_empty() && page.next_offset.is_none());
        assert_eq!(
            page.note.as_deref(),
            Some("no tables of item_8 at or after index 3: its tables are at indexes 2 to 2")
        );
        let err = table_page(&doc, Some(item_8), at(7, 0, 0), 20_000).unwrap_err();
        assert!(err.contains("has 6 tables (indexes 0 to 5)") && err.contains("also when a section"), "{err}");
        let err = table_page(&doc, None, at(6, 1, 0), 20_000).unwrap_err();
        assert!(err.contains("there is no table 6"), "{err}");
        let err = table_page(&doc, None, at(1, 0, 12), 20_000).unwrap_err();
        assert!(err.contains("char 12 is past the end of row 0 of table 1, which has 12"), "{err}");
    }

    /// Tables after the last Item are listed in the outline, and a section
    /// without tables says where they are.
    #[tokio::test]
    async fn tables_outside_items_are_found() {
        let doc = ten_k_with_tables();
        let sections = doc.sections.as_deref().unwrap();
        let runs = outside_runs(sections, &doc.tables);
        let run = |after, before, tables, first_index| OutsideRun { after, before, tables, first_index };
        assert_eq!(runs, [run(None, Some("item_7"), 1, 0), run(Some("item_9"), None, 2, 4)]);
        // A table between two Items that do not touch is between them.
        let mut gap = sections.to_vec();
        gap[1].end = doc.tables[2].start;
        let runs = outside_runs(&gap, &doc.tables);
        assert_eq!(runs[1], run(Some("item_8"), Some("item_9"), 1, 2));

        let mut no_8 = ten_k_with_tables();
        no_8.tables.remove(2);
        let server = pbf_server();
        server.lock().docs.put(key(PBF, ""), Arc::new(no_8));
        let out = json(server.read_tables(Parameters(tables_params(Some("item_8")))).await);
        assert_eq!(out["total_tables"], 0);
        let note = out["note"].as_str().unwrap();
        assert!(note.contains("1 before item_7 from index 0; 2 after item_9 from index 3"), "{note}");
        assert!(note.contains("without section and with offset"), "{note}");
        let mut p = tables_params(None);
        p.offset = Some(3);
        let out = json(server.read_tables(Parameters(p)).await);
        assert_eq!(out["tables"][0]["rows"][0][0], "Cash");
        assert_eq!(out["tables"][0]["text_before"], "Consolidated Balance Sheets");
        assert!(out["tables"][0].get("section").is_none(), "{out}");

        let out = json(server.get_filing_outline(Parameters(outline_params("1566011", Some(PBF), None))).await);
        let outside = serde_json::json!([
            { "before": "item_7", "tables": 1, "first_index": 0 },
            { "after": "item_9", "tables": 2, "first_index": 3 }
        ]);
        assert_eq!(out["tables_outside_items"], outside);
        assert!(out["next"].as_str().unwrap().contains("from offset=first_index"), "{out}");
    }

    /// A 10-K without Item headings: read_tables points to its tables, not away from them.
    #[tokio::test]
    async fn read_tables_without_item_headings() {
        let mut doc = table_doc(3, 2);
        doc.sections = Some(Vec::new());
        let server = pbf_server();
        server.lock().docs.put(key(PBF, ""), Arc::new(doc));
        let err = server.read_tables(Parameters(tables_params(Some("item_8")))).await.unwrap_err();
        assert_eq!(err, "no Item headings were found in this filing; leave out section to read all 3 tables");
        let out = json(server.read_tables(Parameters(tables_params(None))).await);
        assert_eq!(out["total_tables"], 3);
        let out = json(server.get_filing_outline(Parameters(outline_params("1566011", Some(PBF), None))).await);
        assert!(out["next"].as_str().unwrap().contains("read_tables without section for its 3 tables"), "{out}");
        assert!(out.get("tables_outside_items").is_none());
        let p = ReadSectionParams {
            company: "1566011".into(),
            accession: Some(PBF.into()),
            form: None,
            section: "item_8".into(),
            offset: None,
            max_chars: None,
        };
        let err = server.read_section(Parameters(p)).await.unwrap_err();
        assert_eq!(err, "no Item headings were found in this filing; use read_filing_text");
    }

    /// Plain-text filings keep their tables in the text, and the tools say so.
    #[tokio::test]
    async fn plain_text_documents_say_where_tables_are() {
        let raw = "<DOCUMENT>\n<TYPE>10-K\n<TEXT>\nITEM 8. FINANCIAL STATEMENTS\n\n<TABLE>\n<CAPTION>\n\
                   <S>                 <C>\nTotal assets        $ 1,234\n</TABLE>\n</TEXT>\n</DOCUMENT>\n";
        let doc = parsed(raw, true);
        assert!(!doc.html && doc.tables.is_empty());
        assert!(doc.text.contains("Total assets"));
        for section in [None, doc.sections.as_deref().and_then(|s| s.first())] {
            let page = table_page(&doc, section, at(0, 0, 0), 20_000).unwrap();
            assert_eq!(page.note.as_deref(), Some(PLAIN_TEXT_NOTE));
        }
        let server = pbf_server();
        server.lock().docs.put(key(PBF, ""), Arc::new(doc));
        let out = json(server.get_filing_outline(Parameters(outline_params("1566011", Some(PBF), None))).await);
        assert!(out["next"].as_str().unwrap().contains("plain-text document"), "{out}");
        // An HTML document says nothing of the kind.
        assert!(parsed("<p>Item 8.</p><table><tr><td>A</td><td>1</td></tr></table>", true).html);
    }

    #[test]
    fn tables_carry_their_place_and_title() {
        let doc = table_doc(3, 2);
        let page = table_page(&doc, None, at(1, 0, 0), 100_000).unwrap();
        let t = &page.tables[0];
        assert_eq!((t.index, t.text_before), (1, Some("Table 1 (in millions of €)")));
        // text_offset counts characters, as read_filing_text does.
        let at_offset: String = doc.text.chars().skip(t.text_offset).take(24).collect();
        assert_eq!(at_offset, "Row 0 of table 1\t$0,000\n");
        // A line that belongs to another table is not a title.
        let mut doc = table_doc(1, 2);
        let (text, tables) = parsec_edgar::html_to_text_and_tables(
            "<table><tr><td>A</td><td>1</td></tr></table><hr><table><tr><td>B</td><td>2</td></tr></table>",
        );
        (doc.text, doc.tables, doc.sections) = (text, tables, None);
        assert_eq!(table_page(&doc, None, at(1, 0, 0), 100_000).unwrap().tables[0].text_before, None);
    }

    #[test]
    fn table_page_errors_and_empty_documents() {
        let doc = table_doc(2, 3);
        assert!(table_page(&doc, None, at(3, 0, 0), 20_000).unwrap_err().contains("past the last table"));
        assert!(table_page(&doc, None, at(1, 3, 0), 20_000).unwrap_err().contains("past the last row"));
        let page = table_page(&doc, None, at(2, 0, 0), 20_000).unwrap();
        assert!(page.tables.is_empty() && page.next_offset.is_none());
        assert_eq!(page.note.as_deref(), Some("no tables at or after index 2: the last table is at index 1"));
        let none = Doc {
            document: None,
            text: "Text\n".into(),
            total_chars: 5,
            sections: None,
            tables: Vec::new(),
            html: true,
        };
        let page = table_page(&none, None, at(0, 0, 0), 20_000).unwrap();
        assert_eq!((page.total_tables, page.note.as_deref()), (0, Some("no tables in this document")));
    }

    fn tables_params(section: Option<&str>) -> ReadTablesParams {
        ReadTablesParams {
            company: "1566011".into(),
            accession: Some(PBF.into()),
            form: None,
            document: None,
            section: section.map(Into::into),
            offset: None,
            row: None,
            char: None,
            max_chars: None,
        }
    }

    /// read_tables through the server, with the document already cached.
    #[tokio::test]
    async fn read_tables_tool() {
        let server = pbf_server();
        server.lock().docs.put(key(PBF, ""), Arc::new(table_doc(4, 2)));
        let out = json(server.read_tables(Parameters(tables_params(Some("8")))).await);
        assert_eq!((out["section"].as_str(), out["total_tables"].as_u64()), (Some("item_8"), Some(2)));
        assert_eq!(out["tables"][0]["rows"][1], serde_json::json!(["Row 1 of table 2", "$1,000"]));
        assert_eq!(out["company"], "PBF Holding Co LLC");
        assert!(out["next_offset"].is_null());
        let err = server.read_tables(Parameters(tables_params(Some("item_9")))).await.unwrap_err();
        assert!(err.contains("available: item_7, item_8"), "{err}");
        let mut p = tables_params(Some("item_7"));
        p.document = Some("ex211.htm".into());
        let err = server.read_tables(Parameters(p)).await.unwrap_err();
        assert!(err.contains("exhibits have no Item sections"), "{err}");
        // The outline counts tables in all and per section.
        let out = json(server.get_filing_outline(Parameters(outline_params("1566011", Some(PBF), None))).await);
        assert_eq!(out["tables"], 4);
        assert_eq!(out["sections"][0]["tables"], 2);
        // A row cut on one page continues on the next.
        let mut p = tables_params(None);
        (p.offset, p.row, p.char) = (Some(2), Some(1), Some(4));
        let out = json(server.read_tables(Parameters(p)).await);
        assert_eq!(out["tables"][0]["rows"][0], serde_json::json!(["1 of table 2", "$1,000"]));
        assert_eq!(
            (out["tables"][0]["first_row"].as_u64(), out["tables"][0]["first_char"].as_u64()),
            (Some(1), Some(4))
        );
    }

    fn key(acc: &str, document: &str) -> DocKey {
        (acc.to_owned(), document.to_owned())
    }

    fn json(out: Result<String, String>) -> serde_json::Value {
        serde_json::from_str(&out.unwrap()).unwrap()
    }

    const PBF: &str = "0001566011-25-000004";

    /// A server that already holds PBF's 10-K, filed by two co-registrants, and its EX-21.1.
    fn pbf_server() -> EdgarServer {
        let server = offline_server();
        let mut cache = server.lock();
        let holding = filing_of(1566011, "PBF Holding Co LLC", PBF, "pbfh-20241231.htm");
        let finance = filing_of(1566097, "PBF Finance Corp", PBF, "pbfh-20241231.htm");
        cache.by_accession.put(("1566011".into(), PBF.replace('-', "")), holding);
        cache.by_accession.put(("1566097".into(), PBF.replace('-', "")), finance);
        cache.docs.put(key(PBF, ""), doc("Item 1. Business\n\nWe refine oil.\n", None));
        cache.docs.put(key(PBF, "ex211.htm"), doc("Subsidiaries of PBF Holding\n", Some("ex211.htm")));
        drop(cache);
        server
    }

    fn outline_params(company: &str, accession: Option<&str>, document: Option<&str>) -> FilingParams {
        FilingParams {
            company: company.into(),
            accession: accession.map(Into::into),
            form: None,
            document: document.map(Into::into),
        }
    }

    /// Co-registrants share one accession number; each sees its own name and CIK.
    #[tokio::test]
    async fn co_registrants_keep_their_own_metadata() {
        let server = pbf_server();
        for (cik, name) in [(1566011, "PBF Holding Co LLC"), (1566097, "PBF Finance Corp")] {
            let p = outline_params(&cik.to_string(), Some(PBF), None);
            let out = json(server.get_filing_outline(Parameters(p)).await);
            assert_eq!((out["company"].as_str(), out["cik"].as_u64()), (Some(name), Some(cik)));
            assert!(out["url"].as_str().unwrap().contains(&format!("/data/{cik}/")), "{out}");
        }
    }

    /// Exhibits from search hits are read by file name, from the same filing folder.
    #[tokio::test]
    async fn reads_exhibits_by_document() {
        let server = pbf_server();
        let p = outline_params("1566097", Some(PBF), Some("ex211.htm"));
        let out = json(server.get_filing_outline(Parameters(p)).await);
        assert_eq!(out["document"], "ex211.htm");
        assert_eq!(out["url"], "https://www.sec.gov/Archives/edgar/data/1566097/000156601125000004/ex211.htm");
        assert!(out.get("sections").is_none());
        assert!(out["next"].as_str().unwrap().contains("read_filing_text"), "{out}");

        let p = ReadTextParams {
            company: "1566011".into(),
            accession: Some(PBF.into()),
            form: None,
            document: Some("ex211.htm".into()),
            offset: None,
            max_chars: None,
        };
        let text = server.read_filing_text(Parameters(p)).await.unwrap();
        assert!(text.ends_with("\n\nSubsidiaries of PBF Holding\n"), "{text}");
        assert!(text.starts_with(r#"{"company":"PBF Holding Co LLC","cik":1566011,"#), "{text}");

        // The primary document named explicitly is the primary document, with its sections.
        let p = outline_params("1566011", Some(PBF), Some("PBFH-20241231.htm"));
        let out = json(server.get_filing_outline(Parameters(p)).await);
        assert!(out.get("document").is_none() && out["sections"].is_array(), "{out}");
    }

    #[tokio::test]
    async fn rejects_bad_document_names() {
        let server = pbf_server();
        for bad in ["../../../etc/passwd", "a/b.htm", "..htm", ".htaccess", "x.htm?y=1", "a b.htm", "ex%2F.htm"] {
            let err = server.get_filing_outline(Parameters(outline_params("1566011", Some(PBF), Some(bad)))).await;
            assert!(err.unwrap_err().contains("file name"), "{bad}");
        }
        let err = server.get_filing_outline(Parameters(outline_params("1566011", Some(PBF), Some("chart.jpg")))).await;
        assert!(err.unwrap_err().contains("not a text document"));
        let err = server.get_filing_outline(Parameters(outline_params("1566011", None, Some("ex211.htm")))).await;
        assert!(err.unwrap_err().contains("needs the accession"));
        assert!(check_document_name("d123456dex991.htm").is_ok());
        assert!(check_document_name("primary_doc.xml").is_ok());
        assert!(check_document_name("0001193125-09-012345.TXT").is_ok());
    }

    /// Calls that need the same document at once share one load.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_loads_share_one_download() {
        let server = offline_server();
        let loads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls = (0..6).map(|_| {
            let server = server.clone();
            let loads = loads.clone();
            tokio::spawn(async move {
                server
                    .shared_doc(key("acc", ""), move || {
                        loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(200));
                        Ok(Doc {
                            document: None,
                            text: "text".into(),
                            total_chars: 4,
                            sections: None,
                            tables: Vec::new(),
                            html: true,
                        })
                    })
                    .await
            })
        });
        let docs: Vec<Arc<Doc>> = futures_join(calls).await;
        assert_eq!(loads.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(docs.iter().all(|d| Arc::ptr_eq(d, &docs[0])));
        let cache = server.lock();
        assert!(cache.loading.is_empty());
        assert_eq!(cache.docs.entries.len(), 1);
    }

    async fn futures_join<T>(handles: impl Iterator<Item = tokio::task::JoinHandle<Result<T, String>>>) -> Vec<T> {
        let mut out = Vec::new();
        for h in handles.collect::<Vec<_>>() {
            out.push(h.await.unwrap().unwrap());
        }
        out
    }

    /// A failed load is not cached, and the next call tries again.
    #[tokio::test]
    async fn failed_loads_are_retried() {
        let server = offline_server();
        let err = server.shared_doc(key("acc", ""), || Err("HTTP 503".into())).await;
        assert_eq!(err.err().as_deref(), Some("HTTP 503"));
        let empty = || {
            let text = String::new();
            Ok(Doc { document: None, text, total_chars: 0, sections: None, tables: Vec::new(), html: true })
        };
        assert!(server.shared_doc(key("acc", ""), empty).await.is_ok());
        assert!(server.lock().loading.is_empty());
    }

    /// A load that counts itself in `loads`.
    fn counted_load(loads: Arc<AtomicUsize>) -> impl FnOnce() -> Result<Doc, String> + Send + 'static {
        move || {
            loads.fetch_add(1, Ordering::SeqCst);
            let text = "text".to_owned();
            Ok(Doc { document: None, text, total_chars: 4, sections: None, tables: Vec::new(), html: true })
        }
    }

    /// A counted load that says when it has started, then waits until `release` sends.
    fn held_load(
        loads: Arc<AtomicUsize>,
        started: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    ) -> impl FnOnce() -> Result<Doc, String> + Send + 'static {
        let load = counted_load(loads);
        move || {
            started.send(()).unwrap();
            release.recv().map_err(|e| e.to_string())?;
            load()
        }
    }

    /// Wait until `n` calls wait for the load of `key`.
    fn wait_for_waiting(server: &EdgarServer, key: &DocKey, n: usize) {
        for _ in 0..500 {
            if server.lock().loading.get(key).map(|loading| loading.waiting) == Some(n) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("{n} calls never waited for {key:?}");
    }

    /// A call cancelled during its load leaves nothing behind in `loading`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_loads_leave_no_entry() {
        let server = offline_server();
        let loads = Arc::new(AtomicUsize::new(0));
        let (started_tx, started) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let load = held_load(loads.clone(), started_tx, release_rx);
        let call = tokio::spawn({
            let server = server.clone();
            async move { server.shared_doc(key("acc", ""), load).await }
        });
        started.recv().unwrap();
        call.abort();
        assert!(matches!(call.await, Err(e) if e.is_cancelled()));
        assert!(server.lock().loading.is_empty());
        release.send(()).unwrap();
    }

    /// A cancelled call does not end a load that other calls still wait for.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_waiters_leave_the_load_shared() {
        let server = offline_server();
        let loads = Arc::new(AtomicUsize::new(0));
        let call = |load: Box<dyn FnOnce() -> Result<Doc, String> + Send>| {
            let server = server.clone();
            tokio::spawn(async move { server.shared_doc(key("acc", ""), load).await })
        };
        let (started_tx, started) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let first = call(Box::new(held_load(loads.clone(), started_tx, release_rx)));
        started.recv().unwrap();

        let cancelled = call(Box::new(counted_load(loads.clone())));
        wait_for_waiting(&server, &key("acc", ""), 2);
        cancelled.abort();
        assert!(matches!(cancelled.await, Err(e) if e.is_cancelled()));
        wait_for_waiting(&server, &key("acc", ""), 1);

        // A new call joins the first load instead of starting another.
        let last = call(Box::new(counted_load(loads.clone())));
        wait_for_waiting(&server, &key("acc", ""), 2);
        release.send(()).unwrap();
        let docs: Vec<Arc<Doc>> = futures_join([first, last].into_iter()).await;
        assert!(Arc::ptr_eq(&docs[0], &docs[1]));
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert!(server.lock().loading.is_empty());
    }

    #[test]
    fn doc_cache_keeps_the_most_recently_used() {
        let mut cache = Cache::default();
        for i in 0..DOC_CACHE_SIZE {
            cache.docs.put(key(&i.to_string(), ""), doc("x", None));
        }
        // Reading 0 makes 1 the least recently used.
        assert!(cache.docs.get(&key("0", "")).is_some());
        cache.docs.put(key("new", ""), doc("x", None));
        assert_eq!(cache.docs.entries.len(), DOC_CACHE_SIZE);
        assert!(cache.docs.get(&key("1", "")).is_none());
        assert!(cache.docs.get(&key("0", "")).is_some());
        // An attachment is cached apart from the primary document of its filing.
        cache.docs.put(key("0", "ex99-1.htm"), doc("y", Some("ex99-1.htm")));
        assert_eq!(cache.docs.get(&key("0", "")).unwrap().text, "x");
        assert_eq!(cache.docs.get(&key("0", "ex99-1.htm")).unwrap().text, "y");
        // Putting a key again replaces it instead of adding a second copy.
        cache.docs.put(key("0", ""), doc("z", None));
        assert_eq!(cache.docs.entries.len(), DOC_CACHE_SIZE);
        assert_eq!(cache.docs.get(&key("0", "")).unwrap().text, "z");
    }

    #[test]
    fn lru_drops_the_least_recently_used() {
        let mut lru = Lru::<u32, &str, 2>::default();
        lru.put(1, "a");
        lru.put(2, "b");
        // Reading 1 makes 2 the least recently used.
        assert_eq!(lru.get(&1), Some(&"a"));
        lru.put(3, "c");
        assert_eq!(lru.entries.len(), 2);
        assert_eq!([1, 2, 3].map(|k| lru.get(&k).copied()), [Some("a"), None, Some("c")]);
        // A key put again keeps one entry, and nothing else is dropped.
        lru.put(3, "d");
        assert_eq!(lru.entries.len(), 2);
        assert_eq!([1, 3].map(|k| lru.get(&k).copied()), [Some("a"), Some("d")]);
        assert_eq!(lru.get(&4), None);
        lru.remove(&1);
        assert_eq!((lru.get(&1).copied(), lru.entries.len()), (None, 1));
    }

    /// Filings by accession and latest-filing lookups stop at FILING_CACHE_SIZE.
    #[test]
    fn filing_caches_are_bounded() {
        let mut cache = Cache::default();
        let now = Instant::now();
        let accession = |i: usize| ("1".to_owned(), format!("{i:018}"));
        let latest = |i: usize| (i.to_string(), "10-K".to_owned());
        for i in 0..FILING_CACHE_SIZE + 5 {
            if i == FILING_CACHE_SIZE {
                // Read again, the first filing becomes the most recently used.
                assert!(cache.by_accession.get(&accession(0)).is_some());
            }
            let filing = filing_of(1, "A", &accession(i).1, "a.htm");
            cache.by_accession.put(accession(i), filing.clone());
            cache.latest.put(latest(i), (now, filing));
        }
        assert_eq!(cache.by_accession.entries.len(), FILING_CACHE_SIZE);
        assert_eq!(cache.latest.entries.len(), FILING_CACHE_SIZE);
        let kept = |i| cache.by_accession.entries.contains_key(&accession(i));
        assert_eq!((kept(0), kept(1), kept(5), kept(6)), (true, false, false, true));
        let kept = |i| cache.latest.entries.contains_key(&latest(i));
        assert_eq!((kept(0), kept(4), kept(5)), (false, false, true));
    }

    /// A latest filing is reused for LATEST_TTL, then dropped from the cache.
    #[test]
    fn latest_filings_expire() {
        let mut cache = Cache::default();
        let key = ("AAPL".to_owned(), "10-K".to_owned());
        let at = Instant::now();
        cache.latest.put(key.clone(), (at, filing_of(320193, "Apple Inc.", "0000320193-25-000079", "a.htm")));
        assert!(cache.fresh_latest(&key, at + LATEST_TTL - Duration::from_secs(1)).is_some());
        assert!(cache.fresh_latest(&key, at + LATEST_TTL).is_none());
        assert!(cache.latest.entries.is_empty());
    }

    #[test]
    fn unknown_ciks_point_to_find_company() {
        let url = "https://data.sec.gov/submissions/CIK0009999999.json".to_owned();
        let msg = explain(Error::Status { status: 404, url });
        assert_eq!(msg, "no EDGAR filer has CIK 9999999; use find_company to search by name or ticker");
        let url = "https://data.sec.gov/submissions/CIK0000320193-submissions-001.json".to_owned();
        assert!(explain(Error::Status { status: 404, url }).contains("no document at"));
        let url = "https://www.sec.gov/Archives/edgar/data/1/2/a.htm".to_owned();
        assert!(explain(Error::Status { status: 404, url }).contains("no document at"));
    }

    #[test]
    fn list_filings_reads_history_in_the_same_call() {
        let q = list_query(vec!["10-K".into()], Some("2010".into()), None, 20);
        assert_eq!(q.limit, Some(21));
        assert!(q.all_history);
        assert_eq!(list_note(21, 20), Some("more filings match: raise limit or narrow with since/until"));
        assert_eq!(list_note(20, 20), None);
        assert_eq!(list_note(0, 20), Some("no filings match these filters"));
    }

    fn results(total: u64, exact: bool, hits: usize, skipped: &[usize]) -> FullTextResults {
        let hit: FullTextHit = serde_json::from_value(serde_json::json!({
            "cik": 1, "company": "A", "form": "8-K", "filing_date": "2025-01-02",
            "accession_number": "0000000001-25-000001", "document": "a.htm", "document_type": "8-K",
            "document_url": "https://www.sec.gov/Archives/edgar/data/1/000000000125000001/a.htm"
        }))
        .unwrap();
        FullTextResults { total, total_is_exact: exact, hits: vec![hit; hits], skipped: skipped.to_vec() }
    }

    #[test]
    fn search_paging() {
        // A full page of 100, shown 20 at a time.
        assert_eq!(search_window(0, 20, &results(250, true, 100, &[])), (20, Some(20)));
        // The last hits: nothing after them.
        assert_eq!(search_window(240, 20, &results(250, true, 10, &[])), (10, None));
        assert_eq!(search_window(230, 20, &results(250, true, 20, &[])), (20, None));
        // Over 10,000 hits (not an exact count): pages stop at the 10,000th.
        assert_eq!(search_window(9_900, 20, &results(10_000, false, 100, &[])), (20, Some(9_920)));
        assert_eq!(search_window(9_980, 20, &results(10_000, false, 20, &[])), (20, None));
        // No hits at all.
        assert_eq!(search_window(0, 20, &results(0, true, 0, &[])), (0, None));
    }

    /// Hits EFTS returned but the client could not read still count for offsets,
    /// so the next page neither repeats nor skips hits.
    #[test]
    fn search_paging_counts_unreadable_hits() {
        // Raw positions A, B (unreadable), C, D with limit 2: show A and C, continue at D.
        assert_eq!(search_window(0, 2, &results(4, true, 3, &[1])), (2, Some(3)));
        // Unreadable hits right after the shown ones are stepped over.
        assert_eq!(search_window(0, 2, &results(10, true, 3, &[2, 3])), (2, Some(4)));
        // Only unreadable hits are left: the page is used up.
        assert_eq!(search_window(8, 2, &results(10, true, 0, &[0, 1])), (0, None));
    }

    fn hit(form: &str, document_type: &str, document: &str) -> FullTextHit {
        let mut h = results(1, true, 1, &[]).hits.remove(0);
        h.form = form.into();
        h.document_type = document_type.into();
        h.document = document.into();
        h
    }

    /// Main documents carry no document fields; exhibits name their type and file.
    #[test]
    fn search_rows_tell_exhibits_from_main_documents() {
        // The fixture's hits: Apple's 10-K, Windtree's EX-10.76 and the Immersion 13D/A's EX-99.1.
        let main = SearchRow::from(&hit("10-K", "10-K", "aapl-20260926.htm"));
        assert_eq!((main.document_type, main.document), (None, None));
        let ex = SearchRow::from(&hit("10-K", "EX-10.76", "ex_802014.htm"));
        assert_eq!(ex.document_type.as_deref(), Some("EX-10.76"));
        assert_eq!(ex.document.as_deref(), Some("ex_802014.htm"));
        let ex = SearchRow::from(&hit("SC 13D/A", "EX-99.1", "ex991to13da506297074_120409.htm"));
        assert_eq!(ex.document.as_deref(), Some("ex991to13da506297074_120409.htm"));
        // Amendments: the main document may carry the form with or without "/A".
        assert_eq!(SearchRow::from(&hit("10-K/A", "10-K/A", "a.htm")).document, None);
        assert_eq!(SearchRow::from(&hit("10-K/A", "10-K", "a.htm")).document, None);
        assert_eq!(SearchRow::from(&hit("8-K/A", "EX-99.1", "b.htm")).document.as_deref(), Some("b.htm"));
    }

    #[test]
    fn http_hosts_and_origins() {
        let local = http_access("127.0.0.1:8080".parse().unwrap(), &[], &[]);
        assert_eq!(local.hosts, ["localhost", "127.0.0.1", "::1"]);
        assert!(local.origins.contains(&"http://localhost:*".to_owned()));
        assert_eq!(local.url, "http://127.0.0.1:8080/mcp");
        assert_eq!(local.note, None);

        // A specific address is accepted as the Host and as an origin.
        let lan = http_access("192.168.1.5:8080".parse().unwrap(), &["mcp.example.com".into()], &[]);
        assert_eq!(lan.hosts, ["localhost", "127.0.0.1", "::1", "192.168.1.5", "mcp.example.com"]);
        assert!(lan.origins.contains(&"http://192.168.1.5:*".to_owned()));
        assert_eq!(lan.url, "http://192.168.1.5:8080/mcp");
        let v6 = http_access("[fd00::5]:8080".parse().unwrap(), &[], &[]);
        assert!(v6.hosts.contains(&"[fd00::5]".to_owned()));
        assert_eq!(v6.url, "http://[fd00::5]:8080/mcp");

        // All interfaces: print an address that works, and say how to add names.
        let any = http_access("0.0.0.0:8080".parse().unwrap(), &[], &["https://app.example.com".into()]);
        assert_eq!(any.hosts, ["localhost", "127.0.0.1", "::1"]);
        assert_eq!(any.url, "http://127.0.0.1:8080/mcp");
        assert!(any.note.unwrap().contains("--allow-host"));
        assert_eq!(any.origins.last().map(String::as_str), Some("https://app.example.com"));
    }

    #[test]
    fn allowed_origins_are_checked() {
        for ok in
            ["https://app.example.com", "https://app.example.com:8443", "http://localhost:*", "null", "http://[::1]:*"]
        {
            assert!(check_origin(ok).is_ok(), "{ok}");
        }
        // rmcp would skip these silently, so every browser request would get 403.
        for bad in [
            "app.example.com",
            "app.example.com:443",
            "https://",
            "https://app.example.com/path",
            "https://a.com:8443:*",
            "",
        ] {
            assert!(check_origin(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn tools_are_listed_with_schemas() {
        let router = EdgarServer::tool_router();
        let mut names: Vec<String> = router.list_all().into_iter().map(|t| t.name.to_string()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "find_company",
                "get_filing_outline",
                "list_filings",
                "read_filing_text",
                "read_section",
                "read_tables",
                "search_full_text"
            ]
        );
    }
}
