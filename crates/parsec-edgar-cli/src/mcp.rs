//! MCP server: `parsec-edgar mcp` over stdio, or Streamable HTTP with `--http ADDR`.
//!
//! Tools return compact JSON and page long text and lists of tables, so an
//! agent can read an outline first and then only the sections it needs.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use parsec_edgar::{
    Client, Company, Error, FULL_TEXT_MAX_RESULTS, FULL_TEXT_PAGE_SIZE, Filing, FilingQuery, FormKind, FullTextHit,
    FullTextQuery, FullTextResults, Section, Table, document_to_text_and_tables, extract_sections,
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

const DEFAULT_MAX_CHARS: usize = 20_000;
const MAX_MAX_CHARS: usize = 100_000;
const MIN_MAX_CHARS: usize = 200;
const DEFAULT_FILINGS: usize = 20;
const MAX_FILINGS: usize = 200;
const DEFAULT_HITS: usize = 20;
const MAX_COMPANY_MATCHES: usize = 10;
/// Parsed filings kept in memory, so paging through one filing parses it once.
const DOC_CACHE_SIZE: usize = 6;
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
}

/// Accession number, and the file name of an attachment or "" for the primary document.
type DocKey = (String, String);

#[derive(Default)]
struct Cache {
    /// (company, accession) to filing. Filings never change.
    by_accession: HashMap<(String, String), Filing>,
    /// (company, form) to the latest filing, with the time it was looked up.
    latest: HashMap<(String, String), (Instant, Filing)>,
    /// Parsed documents, most recently used last.
    docs: VecDeque<(DocKey, Arc<Doc>)>,
    /// Documents being loaded, so concurrent calls share one download and parse.
    loading: HashMap<DocKey, Arc<OnceCell<Arc<Doc>>>>,
}

impl Cache {
    /// A parsed document, which becomes the most recently used.
    fn get_doc(&mut self, key: &DocKey) -> Option<Arc<Doc>> {
        let i = self.docs.iter().position(|(k, _)| k == key)?;
        let entry = self.docs.remove(i)?;
        let doc = entry.1.clone();
        self.docs.push_back(entry);
        Some(doc)
    }

    /// Keep a parsed document, dropping the least recently used beyond DOC_CACHE_SIZE.
    fn put_doc(&mut self, key: DocKey, doc: Arc<Doc>) {
        self.docs.retain(|(k, _)| *k != key);
        self.docs.push_back((key, doc));
        while self.docs.len() > DOC_CACHE_SIZE {
            self.docs.pop_front();
        }
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
    /// Omit for every table of the document.
    section: Option<String>,
    /// Index of the first table to return (default 0). Use next_offset from the previous page.
    offset: Option<usize>,
    /// Rows to skip in that first table (default 0). Use next_row from the previous page when it is set.
    row: Option<usize>,
    /// Maximum characters to return (default 20000, at most 100000).
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
            character counts and table counts. Call this first, then read only the sections you need with \
            read_section, or their tables with read_tables. Identify the filing by company plus accession, or \
            company plus form for the latest filing of that form (default 10-K). For an exhibit hit from \
            search_full_text, also pass its document.",
        annotations(title = "Filing outline", read_only_hint = true, open_world_hint = true)
    )]
    async fn get_filing_outline(&self, Parameters(p): Parameters<FilingParams>) -> Result<String, String> {
        let (filing, doc) =
            self.doc(&p.company, p.accession.as_deref(), p.form.as_deref(), p.document.as_deref()).await?;
        let next = match (&doc.document, &doc.sections) {
            (Some(name), _) => format!(
                "{name} is an exhibit or attachment without Item sections; read it with read_filing_text, \
                 passing the same accession and document"
            ),
            (None, Some(s)) if !s.is_empty() => {
                "read_section with one of these ids; long sections come in pages of max_chars".to_owned()
            }
            (None, Some(_)) => "no Item headings were found in this filing; use read_filing_text".to_owned(),
            (None, None) => format!(
                "Item sections are extracted for 10-K, 10-Q and 8-K only, not {}; use read_filing_text",
                filing.form
            ),
        };
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
            get_filing_outline, such as item_8 for a 10-K's financial statements) for only the tables of that \
            Item. Each table has its index, its section, text_before (the line just above it, often a title or \
            units such as \"(in millions)\", when that line is short and not part of another table), \
            text_offset (its character position for read_filing_text), header_rows when the filing marks them, \
            and rows. Returns up to max_chars characters of tables; when more remain, call again with \
            offset=next_offset, and row=next_row when it is set (a table longer than max_chars is split).",
        annotations(title = "Read tables", read_only_hint = true, open_world_hint = true)
    )]
    async fn read_tables(&self, Parameters(p): Parameters<ReadTablesParams>) -> Result<String, String> {
        let (filing, doc) =
            self.doc(&p.company, p.accession.as_deref(), p.form.as_deref(), p.document.as_deref()).await?;
        let section = match p.section.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => None,
            Some(wanted) => match &doc.sections {
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
        let page = table_page(&doc, section, p.offset.unwrap_or(0), p.row.unwrap_or(0), max_chars(p.max_chars))?;
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
            if let Some(doc) = cache.get_doc(&key) {
                return Ok(doc);
            }
            cache.loading.entry(key.clone()).or_default().clone()
        };
        let init = || async move { blocking(move || load().map(Arc::new)).await };
        let loaded = cell.get_or_try_init(init).await.cloned();
        let mut cache = self.lock();
        if let Ok(doc) = &loaded {
            cache.put_doc(key.clone(), doc.clone());
        }
        if cache.loading.get(&key).is_some_and(|c| Arc::ptr_eq(c, &cell)) {
            cache.loading.remove(&key);
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
            self.lock().by_accession.insert(key, filing.clone());
            return Ok(filing);
        }
        let form = form.map(str::trim).filter(|f| !f.is_empty()).unwrap_or("10-K").to_owned();
        let key = (company_key, form.to_ascii_uppercase());
        if let Some((at, f)) = self.lock().latest.get(&key)
            && at.elapsed() < LATEST_TTL
        {
            return Ok(f.clone());
        }
        let filing = blocking(move || client.latest(&company, &form).map_err(explain)).await?;
        self.lock().latest.insert(key, (Instant::now(), filing.clone()));
        Ok(filing)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Download and parse the primary document of a filing, or the attachment named `attachment`.
fn load_doc(client: &Client, filing: &Filing, attachment: Option<String>) -> Result<Doc, String> {
    let Some(name) = attachment else {
        let (text, tables) = document_to_text_and_tables(&client.primary_document(filing).map_err(explain)?);
        let sections = FormKind::from_form(&filing.form).map(|kind| extract_sections(&text, kind));
        return Ok(Doc { document: None, total_chars: text.chars().count(), text, sections, tables });
    };
    let raw = client.get_text(&format!("{}/{name}", filing.folder_url())).map_err(|e| match e {
        Error::Status { status: 404, .. } => format!(
            "filing {} has no document {name:?}; pass the document of a search_full_text hit, \
             or leave document out for the primary document",
            filing.accession_number
        ),
        e => explain(e),
    })?;
    let (text, tables) = document_to_text_and_tables(&raw);
    Ok(Doc { document: Some(name), total_chars: text.chars().count(), text, sections: None, tables })
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
}

#[derive(Serialize)]
struct SectionRow<'a> {
    id: &'a str,
    title: &'a str,
    chars: usize,
    tables: usize,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'static str>,
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
    /// Indexes of header rows in the whole table.
    #[serde(skip_serializing_if = "<[usize]>::is_empty")]
    header_rows: &'a [usize],
    /// Set when only part of the table is shown: the index of its first row shown...
    #[serde(skip_serializing_if = "Option::is_none")]
    first_row: Option<usize>,
    /// ...and its number of rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    total_rows: Option<usize>,
    rows: &'a [Vec<String>],
}

/// Whether a section holds a table: a table belongs to the section its start is in.
fn holds(section: &Section, table: &Table) -> bool {
    section.start <= table.start && table.start < section.end
}

/// The tables of `doc` (those in `section` when given) from index `offset`,
/// skipping `row` rows of the first one, up to `max` characters of JSON. A
/// table that does not fit waits for the next page, unless it is the first
/// one, which is then split after as many rows as fit (at least one).
fn table_page<'a>(
    doc: &'a Doc,
    section: Option<&'a Section>,
    offset: usize,
    row: usize,
    max: usize,
) -> Result<TablePage<'a>, String> {
    let total = doc.tables.len();
    if offset > total {
        return Err(format!("offset {offset} is past the last table (the document has {total})"));
    }
    let selected: Vec<usize> = (0..total).filter(|&i| section.is_none_or(|s| holds(s, &doc.tables[i]))).collect();
    let mut page = TablePage {
        total_tables: selected.len(),
        offset,
        next_offset: None,
        next_row: None,
        note: None,
        tables: Vec::new(),
    };
    if selected.is_empty() {
        page.note = Some(if section.is_some() { "no tables in this section" } else { "no tables in this document" });
        return Ok(page);
    }
    let (mut used, mut chars, mut char_at) = (0, 0, 0);
    for &i in selected.iter().filter(|&&i| i >= offset) {
        let t = &doc.tables[i];
        let first = if page.tables.is_empty() { row } else { 0 };
        if first > 0 && first >= t.rows.len() {
            return Err(format!("row {first} is past the last row of table {i}, which has {}", t.rows.len()));
        }
        chars += doc.text[char_at..t.start].chars().count();
        char_at = t.start;
        let mut out = TableOut {
            index: i,
            section: doc.sections.as_deref().and_then(|s| s.iter().find(|s| holds(s, t))).map(|s| s.id.as_str()),
            text_offset: chars,
            text_before: text_before(doc, t),
            header_rows: &t.header_rows,
            first_row: (first > 0).then_some(first),
            total_rows: (first > 0).then_some(t.rows.len()),
            rows: &[],
        };
        let rows = &t.rows[first..];
        // Each row adds its JSON and a comma.
        let row_sizes: Vec<usize> = rows.iter().map(|r| json_chars(r) + 1).collect();
        let whole = json_chars(&out) + row_sizes.iter().sum::<usize>();
        if used + whole <= max {
            out.rows = rows;
            page.tables.push(out);
            used += whole;
            continue;
        }
        if !page.tables.is_empty() {
            page.next_offset = Some(i);
            break;
        }
        out.first_row = Some(first);
        out.total_rows = Some(t.rows.len());
        let mut fit = json_chars(&out) + row_sizes[0];
        let mut take = 1;
        while take < rows.len() && fit + row_sizes[take] <= max {
            fit += row_sizes[take];
            take += 1;
        }
        out.rows = &rows[..take];
        page.tables.push(out);
        used = fit;
        if take < rows.len() {
            page.next_offset = Some(i);
            page.next_row = Some(first + take);
            break;
        }
    }
    Ok(page)
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

fn find_section<'a>(sections: &'a [Section], wanted: &str) -> Result<&'a Section, String> {
    let id = normalize_section_id(wanted);
    if let Some(s) = sections.iter().find(|s| s.id == id) {
        return Ok(s);
    }
    // "1A" in a 10-Q: accept it when only one part has that Item.
    if id.starts_with("item_") {
        let suffix = format!("_{id}");
        let found: Vec<&Section> = sections.iter().filter(|s| s.id.ends_with(&suffix)).collect();
        match found.as_slice() {
            [one] => return Ok(one),
            [] => {}
            many => {
                let ids: Vec<&str> = many.iter().map(|s| s.id.as_str()).collect();
                return Err(format!("section {wanted:?} is ambiguous; use one of {}", ids.join(", ")));
            }
        }
    }
    let ids: Vec<&str> = sections.iter().map(|s| s.id.as_str()).collect();
    if ids.is_empty() {
        return Err("no Item sections were found in this filing; use read_filing_text".into());
    }
    Err(format!("no section {wanted:?} in this filing; available: {}", ids.join(", ")))
}

/// `Item 1A.` -> `item_1a`, `Part II, Item 1A` -> `part_2_item_1a`, `2.02` -> `item_2.02`.
fn normalize_section_id(s: &str) -> String {
    let s = s.trim().trim_end_matches('.').to_ascii_lowercase().replace(',', " ");
    let mut id = s.split_whitespace().collect::<Vec<_>>().join("_");
    // One underscore after each keyword, so "item1a" and "item_1a" agree.
    for word in ["item", "part"] {
        id = id.replace(&format!("{word}_"), word).replace(word, &format!("{word}_"));
    }
    for (roman, n) in [("part_ii_", "part_2_"), ("part_i_", "part_1_")] {
        if let Some(rest) = id.strip_prefix(roman) {
            id = format!("{n}{rest}");
        }
    }
    if id.starts_with("item_") || id.starts_with("part_") { id } else { format!("item_{id}") }
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

    #[test]
    fn section_ids() {
        assert_eq!(normalize_section_id("1A"), "item_1a");
        assert_eq!(normalize_section_id("Item 7."), "item_7");
        assert_eq!(normalize_section_id("item_7a"), "item_7a");
        assert_eq!(normalize_section_id("ITEM 2.02"), "item_2.02");
        assert_eq!(normalize_section_id("2.02"), "item_2.02");
        assert_eq!(normalize_section_id("Part II, Item 1A"), "part_2_item_1a");
        assert_eq!(normalize_section_id("part_1_item_2"), "part_1_item_2");
        assert_eq!(normalize_section_id("part 1 item 2"), "part_1_item_2");
        assert_eq!(normalize_section_id("item1a"), "item_1a");
    }

    fn section(id: &str) -> Section {
        let s = serde_json::json!({
            "id": id, "item": "", "title": "Risk Factors", "heading": "", "start": 0, "end": 0, "text": ""
        });
        serde_json::from_value(s).unwrap()
    }

    #[test]
    fn finds_sections() {
        let q =
            [section("part_1_item_1"), section("part_1_item_2"), section("part_2_item_1"), section("part_2_item_1a")];
        assert_eq!(find_section(&q, "part_1_item_2").unwrap().id, "part_1_item_2");
        assert_eq!(find_section(&q, "1A").unwrap().id, "part_2_item_1a");
        assert_eq!(find_section(&q, "Item 2").unwrap().id, "part_1_item_2");
        let err = find_section(&q, "1").unwrap_err();
        assert!(err.contains("ambiguous") && err.contains("part_2_item_1"), "{err}");
        let err = find_section(&q, "item_9").unwrap_err();
        assert!(err.contains("available: part_1_item_1"), "{err}");
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
        Arc::new(Doc { document, total_chars: text.chars().count(), text: text.to_owned(), sections, tables })
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
        Doc { document: None, total_chars: text.chars().count(), text, sections: Some(sections), tables }
    }

    /// Rows shown on every page from `offset`, following next_offset and next_row.
    fn all_pages(doc: &Doc, section: Option<&Section>, max: usize) -> (Vec<Vec<String>>, usize) {
        let (mut rows, mut pages, mut offset, mut row) = (Vec::new(), 0, 0, 0);
        loop {
            let page = table_page(doc, section, offset, row, max).unwrap();
            pages += 1;
            // Each page stays within max_chars, give or take the commas between tables.
            assert!(json_chars(&page.tables) <= max + page.tables.len() + 1, "{page:?}");
            for t in &page.tables {
                rows.extend(t.rows.iter().cloned());
            }
            match page.next_offset {
                Some(next) => (offset, row) = (next, page.next_row.unwrap_or(0)),
                None => break,
            }
        }
        (rows, pages)
    }

    #[test]
    fn table_pages_cover_every_row_once() {
        let doc = table_doc(10, 5);
        let every: Vec<Vec<String>> = doc.tables.iter().flat_map(|t| t.rows.clone()).collect();
        assert_eq!(every.len(), 50);
        assert_eq!(every[6], ["Row 1 of table 1", "$1,000"]);
        let (rows, pages) = all_pages(&doc, None, 600);
        assert_eq!(rows, every);
        assert!(pages >= 4, "{pages}");
        // One page when everything fits.
        assert_eq!(all_pages(&doc, None, 100_000), (every, 1));
    }

    /// A table longer than max_chars comes in pieces that join up.
    #[test]
    fn long_tables_are_split_by_rows() {
        let doc = table_doc(2, 100);
        let page = table_page(&doc, None, 0, 0, 1000).unwrap();
        let [t] = &page.tables[..] else { panic!("{page:?}") };
        assert_eq!((t.first_row, t.total_rows), (Some(0), Some(100)));
        assert_eq!((page.next_offset, page.next_row), (Some(0), Some(t.rows.len())));
        let next = table_page(&doc, None, 0, t.rows.len(), 1000).unwrap();
        assert_eq!(next.tables[0].rows[0][0], format!("Row {} of table 0", t.rows.len()));
        assert_eq!(next.tables[0].first_row, Some(t.rows.len()));
        let (rows, _) = all_pages(&doc, None, 1000);
        assert_eq!(rows.len(), 200);
        assert_eq!(rows[150], ["Row 50 of table 1", "$50,000"]);
        // Even a page too small for one row shows one.
        assert_eq!(table_page(&doc, None, 0, 0, 10).unwrap().tables[0].rows.len(), 1);
    }

    #[test]
    fn tables_of_one_section() {
        let doc = table_doc(6, 2);
        let item_8 = &doc.sections.as_ref().unwrap()[1];
        let page = table_page(&doc, Some(item_8), 0, 0, 100_000).unwrap();
        assert_eq!(page.total_tables, 3);
        // Indexes count every table of the document.
        assert_eq!(page.tables.iter().map(|t| t.index).collect::<Vec<_>>(), [3, 4, 5]);
        assert!(page.tables.iter().all(|t| t.section == Some("item_8")));
        let page = table_page(&doc, Some(item_8), 0, 0, 300).unwrap();
        assert_eq!((page.tables[0].index, page.next_offset), (3, Some(4)));
        let (rows, _) = all_pages(&doc, Some(item_8), 300);
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0][0], "Row 0 of table 3");
    }

    #[test]
    fn tables_carry_their_place_and_title() {
        let doc = table_doc(3, 2);
        let page = table_page(&doc, None, 1, 0, 100_000).unwrap();
        let t = &page.tables[0];
        assert_eq!((t.index, t.text_before), (1, Some("Table 1 (in millions of €)")));
        // text_offset counts characters, as read_filing_text does.
        let at: String = doc.text.chars().skip(t.text_offset).take(24).collect();
        assert_eq!(at, "Row 0 of table 1\t$0,000\n");
        // A line that belongs to another table is not a title.
        let mut doc = table_doc(1, 2);
        let (text, tables) = parsec_edgar::html_to_text_and_tables(
            "<table><tr><td>A</td><td>1</td></tr></table><hr><table><tr><td>B</td><td>2</td></tr></table>",
        );
        (doc.text, doc.tables, doc.sections) = (text, tables, None);
        assert_eq!(table_page(&doc, None, 1, 0, 100_000).unwrap().tables[0].text_before, None);
    }

    #[test]
    fn table_page_errors_and_empty_documents() {
        let doc = table_doc(2, 3);
        assert!(table_page(&doc, None, 3, 0, 20_000).unwrap_err().contains("past the last table"));
        assert!(table_page(&doc, None, 1, 3, 20_000).unwrap_err().contains("past the last row"));
        let page = table_page(&doc, None, 2, 0, 20_000).unwrap();
        assert!(page.tables.is_empty() && page.next_offset.is_none());
        let none = Doc { document: None, text: "Text\n".into(), total_chars: 5, sections: None, tables: Vec::new() };
        let page = table_page(&none, None, 0, 0, 20_000).unwrap();
        assert_eq!((page.total_tables, page.note), (0, Some("no tables in this document")));
    }

    /// read_tables through the server, with the document already cached.
    #[tokio::test]
    async fn read_tables_tool() {
        let server = pbf_server();
        server.lock().put_doc(key(PBF, ""), Arc::new(table_doc(4, 2)));
        let params = |section: Option<&str>| ReadTablesParams {
            company: "1566011".into(),
            accession: Some(PBF.into()),
            form: None,
            document: None,
            section: section.map(Into::into),
            offset: None,
            row: None,
            max_chars: None,
        };
        let out = json(server.read_tables(Parameters(params(Some("8")))).await);
        assert_eq!((out["section"].as_str(), out["total_tables"].as_u64()), (Some("item_8"), Some(2)));
        assert_eq!(out["tables"][0]["rows"][1], serde_json::json!(["Row 1 of table 2", "$1,000"]));
        assert_eq!(out["company"], "PBF Holding Co LLC");
        assert!(out["next_offset"].is_null());
        let err = server.read_tables(Parameters(params(Some("item_9")))).await.unwrap_err();
        assert!(err.contains("available: item_7, item_8"), "{err}");
        let mut p = params(Some("item_7"));
        p.document = Some("ex211.htm".into());
        let err = server.read_tables(Parameters(p)).await.unwrap_err();
        assert!(err.contains("exhibits have no Item sections"), "{err}");
        // The outline counts tables in all and per section.
        let out = json(server.get_filing_outline(Parameters(outline_params("1566011", Some(PBF), None))).await);
        assert_eq!(out["tables"], 4);
        assert_eq!(out["sections"][0]["tables"], 2);
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
        cache.by_accession.insert(("1566011".into(), PBF.replace('-', "")), holding);
        cache.by_accession.insert(("1566097".into(), PBF.replace('-', "")), finance);
        cache.put_doc(key(PBF, ""), doc("Item 1. Business\n\nWe refine oil.\n", None));
        cache.put_doc(key(PBF, "ex211.htm"), doc("Subsidiaries of PBF Holding\n", Some("ex211.htm")));
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
        assert_eq!(cache.docs.len(), 1);
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
        let empty =
            || Ok(Doc { document: None, text: String::new(), total_chars: 0, sections: None, tables: Vec::new() });
        assert!(server.shared_doc(key("acc", ""), empty).await.is_ok());
        assert!(server.lock().loading.is_empty());
    }

    #[test]
    fn doc_cache_keeps_the_most_recently_used() {
        let mut cache = Cache::default();
        for i in 0..DOC_CACHE_SIZE {
            cache.put_doc(key(&i.to_string(), ""), doc("x", None));
        }
        // Reading 0 makes 1 the least recently used.
        assert!(cache.get_doc(&key("0", "")).is_some());
        cache.put_doc(key("new", ""), doc("x", None));
        assert_eq!(cache.docs.len(), DOC_CACHE_SIZE);
        assert!(cache.get_doc(&key("1", "")).is_none());
        assert!(cache.get_doc(&key("0", "")).is_some());
        // An attachment is cached apart from the primary document of its filing.
        cache.put_doc(key("0", "ex99-1.htm"), doc("y", Some("ex99-1.htm")));
        assert_eq!(cache.get_doc(&key("0", "")).unwrap().text, "x");
        assert_eq!(cache.get_doc(&key("0", "ex99-1.htm")).unwrap().text, "y");
        // Putting a key again replaces it instead of adding a second copy.
        cache.put_doc(key("0", ""), doc("z", None));
        assert_eq!(cache.docs.iter().filter(|(k, _)| *k == key("0", "")).count(), 1);
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
