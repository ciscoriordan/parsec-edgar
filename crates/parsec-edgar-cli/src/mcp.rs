//! MCP server: `parsec-edgar mcp` over stdio, or Streamable HTTP with `--http ADDR`.
//!
//! Tools return compact JSON and page long text, so an agent can read an
//! outline first and then only the sections it needs.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use parsec_edgar::{Client, Company, Error, Filing, FilingQuery, FormKind, FullTextQuery, Section, extract_sections};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

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

const INSTRUCTIONS: &str = "Tools for SEC EDGAR filings (10-K, 10-Q, 8-K, proxy statements and more). \
Typical flow: find_company (or use a ticker directly), then list_filings or search_full_text to pick a filing, \
then get_filing_outline to see its Item sections and their sizes, then read_section for only the parts you need. \
Text comes in pages of max_chars characters (default 20,000, about 5,000 tokens); follow next_offset only when \
you need more. Use read_filing_text for forms without Item sections.";

/// Run the server until the client disconnects (stdio) or Ctrl-C (HTTP).
pub fn serve(client: Client, http: Option<SocketAddr>, allow_hosts: Vec<String>) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let server = EdgarServer::new(client);
    runtime.block_on(async move {
        match http {
            None => {
                let running = server
                    .serve(rmcp::transport::stdio())
                    .await
                    .map_err(|e| Error::Invalid(format!("MCP initialization failed: {e}")))?;
                running.waiting().await.map_err(|e| Error::Invalid(format!("MCP server stopped: {e}")))?;
            }
            Some(addr) => serve_http(server, addr, allow_hosts).await?,
        }
        Ok(())
    })
}

async fn serve_http(server: EdgarServer, addr: SocketAddr, allow_hosts: Vec<String>) -> Result<(), Error> {
    let mut config = StreamableHttpServerConfig::default();
    if !allow_hosts.is_empty() {
        let mut hosts = config.allowed_hosts.clone();
        hosts.extend(allow_hosts);
        config = config.with_allowed_hosts(hosts);
    }
    let cancel = config.cancellation_token.clone();
    let service =
        StreamableHttpService::new(move || Ok(server.clone()), Arc::new(LocalSessionManager::default()), config);
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("parsec-edgar MCP server listening on http://{}/mcp", listener.local_addr()?);
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

/// A filing's text and sections, parsed once.
struct Doc {
    filing: Filing,
    text: String,
    total_chars: usize,
    /// `None` for forms without Item sections.
    sections: Option<Vec<Section>>,
}

#[derive(Default)]
struct Cache {
    /// (company, accession) to filing. Filings never change.
    by_accession: HashMap<(String, String), Filing>,
    /// (company, form) to the latest filing, with the time it was looked up.
    latest: HashMap<(String, String), (Instant, Filing)>,
    /// Most recently used last.
    docs: VecDeque<Arc<Doc>>,
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
    /// Character offset to start at (default 0). Use next_offset from the previous page.
    offset: Option<usize>,
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
            Ok(search_companies(all, &query, MAX_COMPANY_MATCHES))
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
        // Ask for one extra row to know whether more exist.
        let mut query = FilingQuery::new().limit(limit + 1);
        query.forms = p.forms;
        query.since = p.since;
        query.until = p.until;
        let filings = blocking(move || {
            let mut filings = client.filings(&company, &query).map_err(explain)?;
            if filings.len() <= limit {
                // The recent block ran out: read the older history pages too.
                filings = client.filings(&company, &query.all_history(true)).map_err(explain)?;
            }
            Ok(filings)
        })
        .await?;
        let rows: Vec<FilingRow> = filings.iter().take(limit).map(FilingRow::from).collect();
        let note = if filings.len() > limit {
            Some("more filings match: raise limit or narrow with since/until")
        } else if rows.is_empty() {
            Some("no filings match these filters")
        } else {
            None
        };
        Ok(to_json(&FilingList {
            company: filings.first().map(|f| f.company.clone()),
            cik: filings.first().map(|f| f.cik),
            count: rows.len(),
            note,
            filings: rows,
        }))
    }

    #[tool(
        description = "Outline of one filing without its text: filing metadata, the total character count and, \
            for 10-K, 10-Q and 8-K (and amendments), the Item sections with ids, titles and character counts. \
            Call this first, then read only the sections you need with read_section. Identify the filing by \
            company plus accession, or company plus form for the latest filing of that form (default 10-K).",
        annotations(title = "Filing outline", read_only_hint = true, open_world_hint = true)
    )]
    async fn get_filing_outline(&self, Parameters(p): Parameters<FilingParams>) -> Result<String, String> {
        let doc = self.doc(&p.company, p.accession.as_deref(), p.form.as_deref()).await?;
        let next = match &doc.sections {
            Some(s) if !s.is_empty() => {
                "read_section with one of these ids; long sections come in pages of max_chars".to_owned()
            }
            Some(_) => "no Item headings were found in this filing; use read_filing_text".to_owned(),
            None => format!(
                "Item sections are extracted for 10-K, 10-Q and 8-K only, not {}; use read_filing_text",
                doc.filing.form
            ),
        };
        let sections = doc.sections.as_ref().map(|sections| {
            sections.iter().map(|s| SectionRow { id: &s.id, title: &s.title, chars: s.text.chars().count() }).collect()
        });
        Ok(to_json(&Outline { filing: FilingMeta::from(&doc.filing), total_chars: doc.total_chars, next, sections }))
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
        let doc = self.doc(&p.company, p.accession.as_deref(), p.form.as_deref()).await?;
        let Some(sections) = &doc.sections else {
            return Err(format!("{} filings have no Item sections; use read_filing_text instead", doc.filing.form));
        };
        let section = find_section(sections, &p.section)?;
        let page = paginate(&section.text, p.offset.unwrap_or(0), max_chars(p.max_chars))?;
        Ok(page_response(FilingMeta::from(&doc.filing), Some(section), &page, "read_section"))
    }

    #[tool(
        description = "Read the full clean text of a filing's primary document in pages: a JSON header line, then up \
            to max_chars characters starting at offset, with next_offset set when more remains. Use it for forms \
            without Item sections (S-1, DEF 14A, 20-F, 6-K, ...) or when get_filing_outline finds none; for 10-K, \
            10-Q and 8-K prefer read_section.",
        annotations(title = "Read filing text", read_only_hint = true, open_world_hint = true)
    )]
    async fn read_filing_text(&self, Parameters(p): Parameters<ReadTextParams>) -> Result<String, String> {
        let doc = self.doc(&p.company, p.accession.as_deref(), p.form.as_deref()).await?;
        let page = paginate(&doc.text, p.offset.unwrap_or(0), max_chars(p.max_chars))?;
        Ok(page_response(FilingMeta::from(&doc.filing), None, &page, "read_filing_text"))
    }

    #[tool(
        description = "Search the text of all EDGAR filings since 2001 with the SEC's full-text search. Words must \
            all appear; \"double quotes\" match a phrase, OR between terms matches either, -word excludes. Filter by \
            forms (a form also matches its amendments), filing dates and company. Returns matching documents (main \
            documents and exhibits), best match first, with company, CIK, form, filing date, accession and \
            document URL. To read a hit, pass its cik as company and its accession to get_filing_outline, then \
            read_section (or read_filing_text). Page with offset and next_offset.",
        annotations(title = "Full-text search", read_only_hint = true, open_world_hint = true)
    )]
    async fn search_full_text(&self, Parameters(p): Parameters<SearchParams>) -> Result<String, String> {
        let limit = p.limit.unwrap_or(DEFAULT_HITS).clamp(1, parsec_edgar::FULL_TEXT_PAGE_SIZE);
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
        let hits: Vec<_> = results.hits.iter().take(limit).map(SearchRow::from).collect();
        let shown_end = offset + hits.len();
        let next_offset =
            (shown_end < results.total as usize && shown_end < parsec_edgar::FULL_TEXT_MAX_RESULTS && !hits.is_empty())
                .then_some(shown_end);
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
    /// Resolve a filing and load its text, from the cache when possible.
    async fn doc(&self, company: &str, accession: Option<&str>, form: Option<&str>) -> Result<Arc<Doc>, String> {
        let company = company.trim().to_owned();
        if company.is_empty() {
            return Err("company is empty: pass a ticker or CIK (find_company looks them up)".into());
        }
        let filing = self.filing(&company, accession, form).await?;
        if let Some(doc) = self.cached_doc(&filing.accession_number) {
            return Ok(doc);
        }
        let client = self.client.clone();
        let doc = blocking(move || {
            let text = client.filing_text(&filing).map_err(explain)?;
            let sections = FormKind::from_form(&filing.form).map(|kind| extract_sections(&text, kind));
            let total_chars = text.chars().count();
            Ok(Arc::new(Doc { filing, text, total_chars, sections }))
        })
        .await?;
        let mut cache = self.lock();
        cache.docs.retain(|d| d.filing.accession_number != doc.filing.accession_number);
        cache.docs.push_back(doc.clone());
        while cache.docs.len() > DOC_CACHE_SIZE {
            cache.docs.pop_front();
        }
        Ok(doc)
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

    fn cached_doc(&self, accession: &str) -> Option<Arc<Doc>> {
        let mut cache = self.lock();
        let i = cache.docs.iter().position(|d| d.filing.accession_number == accession)?;
        let doc = cache.docs.remove(i)?;
        cache.docs.push_back(doc.clone());
        Some(doc)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
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
        Error::Status { status: 404, url } => format!("the SEC has no document at {url} (HTTP 404)"),
        other => other.to_string(),
    }
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
    url: String,
}

impl From<&parsec_edgar::FullTextHit> for SearchRow {
    fn from(h: &parsec_edgar::FullTextHit) -> Self {
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
    url: String,
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
            url: f.primary_document_url.clone().unwrap_or_else(|| f.full_submission_url()),
        }
    }
}

#[derive(Serialize)]
struct Outline<'a> {
    #[serde(flatten)]
    filing: FilingMeta<'a>,
    total_chars: usize,
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
                "search_full_text"
            ]
        );
    }
}
