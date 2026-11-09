//! EDGAR full-text search (efts.sec.gov), which covers documents filed since 2001.

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::{Error, Result};

const EFTS_URL: &str = "https://efts.sec.gov/LATEST/search-index";

/// EFTS returns at most this many hits per request.
pub const FULL_TEXT_PAGE_SIZE: usize = 100;

/// EFTS does not page past this many hits.
pub const FULL_TEXT_MAX_RESULTS: usize = 10_000;

/// A query for [`Client::full_text_search`].
///
/// ```no_run
/// use parsec_edgar::{Client, FullTextQuery};
///
/// let client = Client::builder("Example Corp admin@example.com").build()?;
/// let q = FullTextQuery::new("\"supply chain disruption\"").forms(&["10-K"]).since("2025");
/// for hit in client.full_text_search(&q)?.hits {
///     println!("{} {} {} {}", hit.company, hit.form, hit.filing_date, hit.document_url);
/// }
/// # Ok::<(), parsec_edgar::Error>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct FullTextQuery {
    /// Search terms. Words must all appear; `"double quotes"` match a phrase,
    /// `OR` between terms matches either, and `-word` excludes a word.
    pub query: String,
    /// Form types, e.g. `10-K` or `DEF 14A`. A form also matches its amendments (`10-K/A`).
    pub forms: Vec<String>,
    /// Earliest filing date, `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, inclusive.
    pub since: Option<String>,
    /// Latest filing date, `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, inclusive.
    pub until: Option<String>,
    /// Only filings by these filers.
    pub ciks: Vec<u64>,
    /// Number of hits to skip, for paging, below 10,000. One request returns up to 100 hits.
    pub offset: usize,
}

/// The largest `from` EFTS accepts: it serves no hit past the 10,000th.
const LAST_PAGE_START: usize = FULL_TEXT_MAX_RESULTS - FULL_TEXT_PAGE_SIZE;

impl FullTextQuery {
    pub fn new(query: impl Into<String>) -> Self {
        Self { query: query.into(), ..Self::default() }
    }

    pub fn forms(mut self, forms: &[&str]) -> Self {
        self.forms = forms.iter().map(|f| f.to_string()).collect();
        self
    }

    pub fn since(mut self, date: impl Into<String>) -> Self {
        self.since = Some(date.into());
        self
    }

    pub fn until(mut self, date: impl Into<String>) -> Self {
        self.until = Some(date.into());
        self
    }

    pub fn cik(mut self, cik: u64) -> Self {
        self.ciks.push(cik);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// The EFTS request URL for this query. EFTS refuses pages that reach past
    /// 10,000 hits, so an offset in the last 100 asks for the page at 9,900;
    /// [`Client::full_text_search`] then drops the hits before the offset.
    pub fn url(&self) -> Result<String> {
        let q = self.query.trim();
        if q.is_empty() {
            return Err(Error::Invalid("full-text search needs a query".into()));
        }
        if self.offset >= FULL_TEXT_MAX_RESULTS {
            return Err(Error::Invalid(format!(
                "full-text search pages stop at {FULL_TEXT_MAX_RESULTS} hits; narrow the query instead"
            )));
        }
        let mut url = format!("{EFTS_URL}?q={}", percent_encode(q));
        let forms: Vec<&str> = self.forms.iter().map(|f| f.trim()).filter(|f| !f.is_empty()).collect();
        if !forms.is_empty() {
            url.push_str("&forms=");
            url.push_str(&percent_encode(&forms.join(",")));
        }
        if !self.ciks.is_empty() {
            let ciks: Vec<String> = self.ciks.iter().map(|c| format!("{c:010}")).collect();
            url.push_str("&ciks=");
            url.push_str(&ciks.join(","));
        }
        // EFTS ignores a range unless both ends are given.
        if self.since.is_some() || self.until.is_some() {
            let start = match &self.since {
                Some(d) => expand_date(d, false)?,
                None => "1994-01-01".to_owned(),
            };
            let end = match &self.until {
                Some(d) => expand_date(d, true)?,
                None => "9999-12-31".to_owned(),
            };
            url.push_str(&format!("&dateRange=custom&startdt={start}&enddt={end}"));
        }
        let from = self.offset.min(LAST_PAGE_START);
        if from > 0 {
            url.push_str(&format!("&from={from}"));
        }
        Ok(url)
    }

    /// Hits at the start of the requested page that come before `offset`.
    fn hits_before_offset(&self) -> usize {
        self.offset.saturating_sub(LAST_PAGE_START)
    }
}

/// One page of full-text search results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullTextResults {
    /// Number of matching documents. EFTS stops counting at 10,000.
    pub total: u64,
    /// False when `total` is only a lower bound.
    pub total_is_exact: bool,
    /// Matching documents, best match first. A filing appears once per matching document.
    pub hits: Vec<FullTextHit>,
    /// Positions, counted from the query's offset, of hits left out of `hits`
    /// because they name no filer or no file. The next page starts at
    /// `offset + hits.len() + skipped.len()`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<usize>,
}

/// One matching document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullTextHit {
    /// First filer listed on the filing.
    pub cik: u64,
    pub company: String,
    #[serde(default)]
    pub tickers: Vec<String>,
    /// Further filers, e.g. co-registrants, or the filer of a Schedule 13D about `company`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_filers: Vec<FullTextFiler>,
    /// Form of the filing, e.g. `10-K`.
    pub form: String,
    /// `YYYY-MM-DD`.
    pub filing_date: String,
    /// Period of report, `YYYY-MM-DD`.
    #[serde(default)]
    pub report_date: Option<String>,
    /// Dashed form, e.g. `0000320193-24-000123`.
    pub accession_number: String,
    /// File name of the matching document within the filing.
    pub document: String,
    /// Type of the matching document: the form for the main document, or e.g. `EX-10.1`.
    pub document_type: String,
    #[serde(default)]
    pub document_description: Option<String>,
    pub document_url: String,
    #[serde(default)]
    pub score: Option<f64>,
}

/// A filer named on a full-text search hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullTextFiler {
    pub cik: u64,
    pub name: String,
    #[serde(default)]
    pub tickers: Vec<String>,
}

impl Client {
    /// Search the text of EDGAR filings (2001 onward) with the SEC's full-text search.
    /// Returns one page of up to 100 hits starting at `query.offset`.
    pub fn full_text_search(&self, query: &FullTextQuery) -> Result<FullTextResults> {
        let url = query.url()?;
        parse_response(&self.get_bytes(&url)?, &url, query.hits_before_offset())
    }
}

#[derive(Deserialize)]
struct RawResponse {
    hits: Option<RawHits>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

#[derive(Deserialize)]
struct RawHits {
    total: RawTotal,
    #[serde(default)]
    hits: Vec<RawHit>,
}

#[derive(Deserialize)]
struct RawTotal {
    value: u64,
    #[serde(default)]
    relation: String,
}

#[derive(Deserialize)]
struct RawHit {
    #[serde(rename = "_id")]
    id: String,
    #[serde(rename = "_score")]
    score: Option<f64>,
    #[serde(rename = "_source")]
    source: RawSource,
}

#[derive(Deserialize)]
struct RawSource {
    #[serde(default)]
    ciks: Vec<String>,
    #[serde(default)]
    display_names: Vec<String>,
    #[serde(default)]
    form: String,
    #[serde(default)]
    file_date: String,
    period_ending: Option<String>,
    #[serde(default)]
    adsh: String,
    file_type: Option<String>,
    file_description: Option<String>,
}

/// Parse one EFTS page, dropping its first `skip` hits.
fn parse_response(bytes: &[u8], url: &str, skip: usize) -> Result<FullTextResults> {
    let raw: RawResponse =
        serde_json::from_slice(bytes).map_err(|source| Error::Json { url: url.to_owned(), source })?;
    // EFTS reports query errors with HTTP 200 and an error message.
    if let Some(msg) = raw.error_message {
        return Err(Error::Invalid(format!("full-text search failed: {msg}")));
    }
    let raw_hits =
        raw.hits.ok_or_else(|| Error::Invalid(format!("no hits in full-text search response from {url}")))?;
    let mut hits = Vec::new();
    let mut skipped = Vec::new();
    for (i, raw) in raw_hits.hits.into_iter().skip(skip).enumerate() {
        match convert_hit(raw) {
            Some(hit) => hits.push(hit),
            None => skipped.push(i),
        }
    }
    Ok(FullTextResults { total: raw_hits.total.value, total_is_exact: raw_hits.total.relation != "gte", hits, skipped })
}

fn convert_hit(raw: RawHit) -> Option<FullTextHit> {
    let s = raw.source;
    // `_id` is `<accession>:<file name>`.
    let (id_acc, document) = raw.id.split_once(':')?;
    let accession_number = if s.adsh.is_empty() { id_acc.to_owned() } else { s.adsh };
    let mut filers = s.ciks.iter().enumerate().filter_map(|(i, cik)| {
        let cik: u64 = cik.trim().parse().ok()?;
        let (name, tickers) = s.display_names.get(i).map(|d| split_display_name(d)).unwrap_or_default();
        Some(FullTextFiler { cik, name, tickers })
    });
    let first = filers.next()?;
    let other_filers = filers.collect();
    let document_url = format!(
        "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
        first.cik,
        accession_number.replace('-', ""),
        document
    );
    Some(FullTextHit {
        cik: first.cik,
        company: first.name,
        tickers: first.tickers,
        other_filers,
        document_type: s.file_type.filter(|t| !t.is_empty()).unwrap_or_else(|| s.form.clone()),
        form: s.form,
        filing_date: s.file_date,
        report_date: s.period_ending.filter(|d| !d.is_empty()),
        accession_number,
        document: document.to_owned(),
        document_description: s.file_description.filter(|d| !d.is_empty()),
        document_url,
        score: raw.score,
    })
}

/// `"WINDTREE THERAPEUTICS INC /DE/  (WINT, WINTW)  (CIK 0000946486)"` to the name and tickers.
/// EFTS puts two spaces before each parenthesized group.
fn split_display_name(display: &str) -> (String, Vec<String>) {
    let mut parts: Vec<&str> = display.split("  (").collect();
    if parts.len() > 1 && parts.last().is_some_and(|p| p.starts_with("CIK ")) {
        parts.pop();
    }
    let mut tickers = Vec::new();
    if parts.len() > 1 {
        let last = parts.last().map(|p| p.trim_end_matches(')')).unwrap_or_default();
        let looks_like_tickers = !last.is_empty()
            && last.split(", ").all(|t| {
                !t.is_empty()
                    && t.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
            });
        if looks_like_tickers {
            tickers = last.split(", ").map(str::to_owned).collect();
            parts.pop();
        }
    }
    (parts.join("  (").trim().to_owned(), tickers)
}

/// `YYYY`, `YYYY-MM` or `YYYY-MM-DD` to a full date: the first day of the period,
/// or the last when `end` is set.
fn expand_date(d: &str, end: bool) -> Result<String> {
    let bad = || Error::Invalid(format!("dates must be YYYY, YYYY-MM or YYYY-MM-DD, got {d:?}"));
    let d = d.trim();
    let well_formed = matches!(d.len(), 4 | 7 | 10)
        && d.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
    if !well_formed {
        return Err(bad());
    }
    let year: u32 = d[..4].parse().map_err(|_| bad())?;
    let month: u32 = if d.len() >= 7 {
        d[5..7].parse().map_err(|_| bad())?
    } else if end {
        12
    } else {
        1
    };
    if !(1..=12).contains(&month) {
        return Err(bad());
    }
    let last_day = match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    let day: u32 = if d.len() == 10 {
        d[8..10].parse().map_err(|_| bad())?
    } else if end {
        last_day
    } else {
        1
    };
    if !(1..=last_day).contains(&day) {
        return Err(bad());
    }
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// Percent-encode everything except RFC 3986 unreserved characters.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/efts_search.json");

    #[test]
    fn parses_fixture() {
        // Hits copied from real EFTS responses (November 2026).
        let r = parse_response(FIXTURE.as_bytes(), "fixture", 0).unwrap();
        assert_eq!(r.total, 10_000);
        assert!(!r.total_is_exact);
        assert_eq!(r.hits.len(), 6);

        let apple = &r.hits[0];
        assert_eq!(apple.cik, 320193);
        assert_eq!(apple.company, "Apple Inc.");
        assert_eq!(apple.tickers, vec!["AAPL"]);
        assert_eq!(apple.form, "10-K");
        assert_eq!(apple.document_type, "10-K");
        assert_eq!(apple.filing_date, "2026-11-03");
        assert_eq!(apple.report_date.as_deref(), Some("2026-09-26"));
        assert_eq!(apple.accession_number, "0000320193-26-000026");
        assert_eq!(apple.document, "aapl-20260926.htm");
        assert_eq!(
            apple.document_url,
            "https://www.sec.gov/Archives/edgar/data/320193/000032019326000026/aapl-20260926.htm"
        );
        assert!(apple.other_filers.is_empty());

        let exhibit = &r.hits[1];
        assert_eq!(exhibit.company, "WINDTREE THERAPEUTICS INC /DE/");
        assert_eq!(exhibit.tickers, vec!["WINT", "WINTW"]);
        assert_eq!(exhibit.form, "10-K");
        assert_eq!(exhibit.document_type, "EX-10.76");

        let no_ticker = &r.hits[2];
        assert_eq!(no_ticker.company, "ACRO BIOMEDICAL CO., LTD.");
        assert!(no_ticker.tickers.is_empty());

        let co_registrants = &r.hits[3];
        assert_eq!(co_registrants.company, "PBF Holding Co LLC");
        assert_eq!(co_registrants.other_filers.len(), 1);
        assert_eq!(co_registrants.other_filers[0].cik, 1566097);
        assert_eq!(co_registrants.other_filers[0].name, "PBF Finance Corp");

        let schedule_13d = &r.hits[4];
        assert_eq!(schedule_13d.form, "SC 13D/A");
        assert_eq!(schedule_13d.report_date, None);
        assert_eq!(schedule_13d.other_filers[0].name, "Ramius LLC");
        assert_eq!(schedule_13d.document_description.as_deref(), Some("PRESS RELEASE DATED DECEMBER 2, 2009"));

        let xml = &r.hits[5];
        assert_eq!(xml.document, "primary_doc.xml");
        assert_eq!(xml.document_description, None);
    }

    #[test]
    fn reports_search_errors() {
        let body = br#"{"errorType":"ResponseError","errorMessage":"failed to parse date field [bad]"}"#;
        let err = parse_response(body, "fixture", 0).unwrap_err().to_string();
        assert!(err.contains("failed to parse date field"), "{err}");
    }

    #[test]
    fn display_names() {
        assert_eq!(
            split_display_name("Apple Inc.  (AAPL)  (CIK 0000320193)"),
            ("Apple Inc.".into(), vec!["AAPL".into()])
        );
        assert_eq!(
            split_display_name("Fundamental Global Inc.  (FGF, FGFPP)  (CIK 0001591890)"),
            ("Fundamental Global Inc.".into(), vec!["FGF".into(), "FGFPP".into()])
        );
        assert_eq!(split_display_name("Ramius LLC  (CIK 0001475770)"), ("Ramius LLC".into(), vec![]));
        // A parenthesized part of the name is not a ticker list.
        assert_eq!(split_display_name("Foo (UK) Ltd  (CIK 0000000001)"), ("Foo (UK) Ltd".into(), vec![]));
    }

    #[test]
    fn urls() {
        let q = FullTextQuery::new("\"supply chain\" OR tariffs").forms(&["10-K", "DEF 14A"]).cik(320193);
        assert_eq!(
            q.url().unwrap(),
            "https://efts.sec.gov/LATEST/search-index?q=%22supply%20chain%22%20OR%20tariffs\
             &forms=10-K%2CDEF%2014A&ciks=0000320193"
        );
        let q = FullTextQuery::new("tariffs").since("2025").offset(100);
        assert!(q.url().unwrap().ends_with("&dateRange=custom&startdt=2025-01-01&enddt=9999-12-31&from=100"));
        let q = FullTextQuery::new("tariffs").until("2024-02");
        assert!(q.url().unwrap().ends_with("&startdt=1994-01-01&enddt=2024-02-29"));
        assert!(FullTextQuery::new("  ").url().is_err());
        assert!(FullTextQuery::new("x").offset(10_000).url().is_err());
        assert!(FullTextQuery::new("x").since("2025/01").url().is_err());
    }

    /// EFTS refuses `from` + 100 past 10,000, so offsets in the last page ask for
    /// the page at 9,900 and drop the hits before the offset.
    #[test]
    fn last_page_offsets() {
        assert!(FullTextQuery::new("x").offset(9_900).url().unwrap().ends_with("&from=9900"));
        assert!(FullTextQuery::new("x").offset(9_950).url().unwrap().ends_with("&from=9900"));
        assert!(FullTextQuery::new("x").offset(9_999).url().unwrap().ends_with("&from=9900"));
        assert!(FullTextQuery::new("x").offset(9_899).url().unwrap().ends_with("&from=9899"));
        let client = Client::builder("a@b.c").build().unwrap();
        let q = FullTextQuery::new("x").offset(9_904);
        client.stub(&q.url().unwrap(), FIXTURE);
        let r = client.full_text_search(&q).unwrap();
        assert_eq!(r.hits.len(), 2);
        assert_eq!(r.hits[0].form, "SC 13D/A");
    }

    /// A hit without a CIK or a file name is left out, and its position is kept
    /// so that paging does not drift.
    #[test]
    fn unreadable_hits_are_counted() {
        let hit = |id: &str, ciks: &str| {
            format!(
                r#"{{"_id": "{id}", "_source": {{"ciks": [{ciks}], "display_names": ["A  (CIK 0000000001)"],
                "form": "8-K", "file_date": "2025-01-02", "adsh": "0000000001-25-000001"}}}}"#
            )
        };
        let body = format!(
            r#"{{"hits": {{"total": {{"value": 5, "relation": "eq"}}, "hits": [{}, {}, {}, {}, {}]}}}}"#,
            hit("0000000001-25-000001:a.htm", r#""0000000001""#),
            hit("0000000001-25-000001:b.htm", ""),
            hit("0000000001-25-000001", r#""0000000001""#),
            hit("0000000001-25-000001:d.htm", r#""0000000001""#),
            hit("0000000001-25-000001:e.htm", r#""x""#),
        );
        let r = parse_response(body.as_bytes(), "test", 0).unwrap();
        let docs: Vec<&str> = r.hits.iter().map(|h| h.document.as_str()).collect();
        assert_eq!(docs, ["a.htm", "d.htm"]);
        assert_eq!(r.skipped, [1, 2, 4]);
        let r = parse_response(body.as_bytes(), "test", 2).unwrap();
        assert_eq!(r.hits.len(), 1);
        assert_eq!(r.skipped, [0, 2]);
    }

    #[test]
    fn dates() {
        assert_eq!(expand_date("2025", false).unwrap(), "2025-01-01");
        assert_eq!(expand_date("2025", true).unwrap(), "2025-12-31");
        assert_eq!(expand_date("2023-02", true).unwrap(), "2023-02-28");
        assert_eq!(expand_date("2000-02", true).unwrap(), "2000-02-29");
        assert_eq!(expand_date("1900-02", true).unwrap(), "1900-02-28");
        assert_eq!(expand_date("2025-06-15", true).unwrap(), "2025-06-15");
        assert!(expand_date("2025-13", false).is_err());
        assert!(expand_date("2025-04-31", false).is_err());
    }
}
