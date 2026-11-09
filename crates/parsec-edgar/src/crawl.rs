//! Bulk crawling: find filings by form and date across all companies (from the
//! full index) or for a list of companies (from the submissions API).
//!
//! Requests per filing:
//! - full index: one `master.gz` per quarter (about 3 MB), then for each filing
//!   its `-index.htm` page and its primary document, so 2 per filing;
//! - company list: one submissions request per company (more for old filings),
//!   which also gives the company profile, then the primary document, so 1 per filing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::company::parse_cik;
use crate::encoding;
use crate::error::{Error, Result};
use crate::filings::Filing;
use crate::html::document_to_text;
use crate::index::{FIRST_YEAR, IndexEntry, Quarter, parse_date, today};
use crate::profile::{CompanyProfile, SubmissionsProfile};

/// Which filings a crawl covers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CrawlQuery {
    /// Exact form types to keep (case-insensitive), e.g. `10-K`, `8-K/A`. Empty keeps all.
    pub forms: Vec<String>,
    /// Quarters to cover. When empty, the quarters from `since` to `until` (or today).
    pub quarters: Vec<Quarter>,
    /// Earliest filing date, `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, inclusive.
    pub since: Option<String>,
    /// Latest filing date, `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, inclusive.
    pub until: Option<String>,
}

impl CrawlQuery {
    pub fn new(forms: &[&str]) -> Self {
        Self { forms: forms.iter().map(|f| f.to_string()).collect(), ..Self::default() }
    }

    /// Add every quarter of the years `from` to `to`, inclusive. Years before
    /// 1993, when EDGAR starts, have no filings and are left out. A span with
    /// no EDGAR year in it (all before 1993, or `to` before `from`) narrows
    /// `since` and `until` to the span instead, so the crawl finds nothing
    /// rather than reading every date.
    pub fn years(mut self, from: u16, to: u16) -> Self {
        let first = from.max(FIRST_YEAR);
        if first > to {
            let (lo, hi) = (format!("{from:04}"), format!("{to:04}"));
            self.since = Some(self.since.map_or(lo.clone(), |s| s.max(lo)));
            self.until = Some(match self.until {
                Some(u) if hi.get(..u.len()).unwrap_or(&hi) > u.as_str() => u,
                _ => hi,
            });
            return self;
        }
        for year in first..=to {
            self.quarters.extend((1..=4).filter_map(|quarter| Quarter::new(year, quarter).ok()));
        }
        self
    }

    pub fn quarter(mut self, q: Quarter) -> Self {
        self.quarters.push(q);
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

    /// Whether any date bound or quarter is set.
    pub fn has_range(&self) -> bool {
        !self.quarters.is_empty() || self.since.is_some() || self.until.is_some()
    }

    /// The quarters whose full index the crawl reads, oldest first. Quarters
    /// after the current one are left out, since they have no index yet.
    pub fn quarters_to_read(&self) -> Result<Vec<Quarter>> {
        for d in [&self.since, &self.until].into_iter().flatten() {
            parse_date(d)?;
        }
        // Nothing to read when the crawl ends before EDGAR starts.
        if let Some(u) = &self.until {
            if parse_date(u)?.0 < FIRST_YEAR {
                return Ok(Vec::new());
            }
        }
        let current = Quarter::current();
        let mut qs = if self.quarters.is_empty() {
            let since = self.since.as_deref().ok_or_else(|| {
                Error::Invalid("give years or quarters, or a since date, to crawl the full index".into())
            })?;
            let until = self.until.clone().unwrap_or_else(today);
            Quarter::range(Quarter::of_date(since)?, Quarter::of_date(&until)?.min(current))
        } else {
            self.quarters.clone()
        };
        qs.sort();
        qs.dedup();
        qs.retain(|q| *q <= current && self.overlaps(q));
        Ok(qs)
    }

    /// Filing date bounds for a company crawl: `since` and `until`, narrowed
    /// to the span of the quarters when there are any.
    fn date_bounds(&self) -> Result<(Option<String>, Option<String>)> {
        for d in [&self.since, &self.until].into_iter().flatten() {
            parse_date(d)?;
        }
        let mut since = self.since.clone();
        let mut until = self.until.clone();
        if let (Some(first), Some(last)) = (self.quarters.iter().min(), self.quarters.iter().max()) {
            let lo = first.first_day();
            let hi = last.last_day();
            since = Some(since.map_or(lo.clone(), |s| s.max(lo)));
            // `until` matches by prefix, so the quarter end is tighter when its prefix is within `u`.
            until = Some(match until {
                Some(u) if hi.get(..u.len()).unwrap_or(&hi) > u.as_str() => u,
                _ => hi,
            });
        }
        Ok((since, until))
    }

    fn overlaps(&self, q: &Quarter) -> bool {
        let after_since = self.since.as_deref().is_none_or(|s| q.last_day().as_str() >= s);
        let before_until = self.until.as_deref().is_none_or(|u| q.first_day().get(..u.len()).unwrap_or("") <= u);
        after_since && before_until
    }

    /// Whether a filing of this form and date is in the crawl.
    pub fn matches(&self, form: &str, filing_date: &str) -> bool {
        if !self.forms.is_empty() && !self.forms.iter().any(|f| f.trim().eq_ignore_ascii_case(form.trim())) {
            return false;
        }
        if self.since.as_deref().is_some_and(|s| filing_date < s) {
            return false;
        }
        if let Some(u) = &self.until {
            if filing_date.get(..u.len()).unwrap_or(filing_date) > u.as_str() {
                return false;
            }
        }
        self.quarters.is_empty() || Quarter::of_date(filing_date).is_ok_and(|q| self.quarters.contains(&q))
    }

    /// Keep the index rows in the crawl, one per accession number. A filing with
    /// several filers has a row for each; the row of the filer whose CIK starts
    /// the accession number (a company filing for itself) wins, else the first.
    pub fn select(&self, entries: impl IntoIterator<Item = IndexEntry>) -> Vec<IndexEntry> {
        let mut out: Vec<IndexEntry> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        for e in entries {
            if !self.matches(&e.form, &e.filing_date) {
                continue;
            }
            match seen.get(&e.accession_number) {
                Some(&i) => {
                    if self_filed(&e) && !self_filed(&out[i]) {
                        out[i] = e;
                    }
                }
                None => {
                    seen.insert(e.accession_number.clone(), out.len());
                    out.push(e);
                }
            }
        }
        out
    }
}

fn self_filed(e: &IndexEntry) -> bool {
    e.accession_number.get(..10).and_then(|p| p.parse::<u64>().ok()) == Some(e.cik)
}

/// A fetched filing document: the bytes as served, and where they came from.
#[derive(Debug, Clone)]
pub struct Document {
    pub url: String,
    pub bytes: Vec<u8>,
}

impl Document {
    /// The document decoded as text (UTF-8, falling back to Windows-1252).
    pub fn decoded(&self) -> String {
        encoding::decode(&self.bytes)
    }

    /// Clean plain text, as [`Client::filing_text`] returns it.
    pub fn text(&self) -> String {
        document_to_text(&self.decoded())
    }

    /// File extension from the URL, e.g. `htm` or `txt`. Documents in an `xsl...`
    /// folder are the SEC's HTML rendering of an XML form (Forms 3, 4 and 5, D,
    /// 13F and others), so they are `htm` although the URL ends in `.xml`.
    pub fn extension(&self) -> &str {
        let mut parts = self.url.rsplit('/');
        let name = parts.next().unwrap_or("");
        if parts.next().is_some_and(|dir| dir.starts_with("xsl")) {
            return "htm";
        }
        match name.rsplit_once('.') {
            Some((_, ext)) if !ext.is_empty() && ext.len() <= 5 => ext,
            _ => "txt",
        }
    }
}

impl Client {
    /// Filings across all companies from the full index, one row per filing,
    /// oldest quarter first. One request per quarter. Resolve each row with
    /// [`Client::resolve_index_entry`] to get a [`Filing`].
    pub fn crawl_index(&self, query: &CrawlQuery) -> Result<Vec<IndexEntry>> {
        // A filing appears in one quarter only, so each quarter is filtered on its own.
        let mut rows = Vec::new();
        for q in query.quarters_to_read()? {
            rows.extend(query.select(self.full_index(q)?));
        }
        Ok(rows)
    }

    /// Filings of one company (ticker or CIK) from the submissions API, newest
    /// first. Without a `since` date or quarters, reads the company's whole
    /// history (up to `until`).
    pub fn crawl_company(&self, company: &str, query: &CrawlQuery) -> Result<Vec<Filing>> {
        Ok(self.crawl_company_with_profile(company, query)?.0)
    }

    /// [`Client::crawl_company`] plus the company's current profile, both from
    /// the same submissions request (one request, more for older filings).
    pub fn crawl_company_with_profile(
        &self,
        company: &str,
        query: &CrawlQuery,
    ) -> Result<(Vec<Filing>, CompanyProfile)> {
        let cik = match parse_cik(company.trim()) {
            Some(cik) => cik,
            None => self.company(company)?.cik,
        };
        let (since, until) = query.date_bounds()?;
        let sub: CompanySubmissions = self.get_json(&format!("https://data.sec.gov/submissions/CIK{cik:010}.json"))?;
        let name = sub.profile.name.clone();
        let mut filings = sub.filings.recent.to_filings(cik, &name);
        let oldest_recent = filings.iter().map(|f| f.filing_date.clone()).min();
        for page in pages_to_read(&sub.filings.files, oldest_recent.as_deref(), since.as_deref(), until.as_deref()) {
            let cols: FilingColumns = self.get_json(&format!("https://data.sec.gov/submissions/{}", page.name))?;
            filings.extend(cols.to_filings(cik, &name));
        }
        filings.retain(|f| query.matches(&f.form, &f.filing_date));
        filings.sort_by(|a, b| {
            b.filing_date.cmp(&a.filing_date).then_with(|| b.accession_number.cmp(&a.accession_number))
        });
        filings.dedup_by(|a, b| a.accession_number == b.accession_number);
        Ok((filings, sub.profile.into_profile(cik)))
    }

    /// The primary document of a filing as served (the complete submission for
    /// old filings without one). One request, cached.
    pub fn fetch_primary_document(&self, filing: &Filing) -> Result<Document> {
        let url = filing.primary_document_url.clone().unwrap_or_else(|| filing.full_submission_url());
        let bytes = self.get_bytes(&url)?;
        Ok(Document { url, bytes })
    }
}

/// A submissions API response: the company's profile, and its filings stored
/// column by column. Read here rather than through [`Client::filings`] so that
/// one request gives both.
#[derive(Deserialize)]
struct CompanySubmissions {
    #[serde(flatten)]
    profile: SubmissionsProfile,
    #[serde(default)]
    filings: SubmissionFilings,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SubmissionFilings {
    recent: FilingColumns,
    /// Older filings, one JSON file per page.
    files: Vec<HistoryPage>,
}

#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", default)]
struct HistoryPage {
    name: String,
    filing_from: Option<String>,
    filing_to: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct FilingColumns {
    accession_number: Vec<String>,
    filing_date: Vec<String>,
    report_date: Vec<Option<String>>,
    acceptance_date_time: Vec<Option<String>>,
    form: Vec<String>,
    items: Vec<Option<String>>,
    size: Vec<Option<u64>>,
    #[serde(rename = "isXBRL")]
    is_xbrl: Vec<serde_json::Value>,
    #[serde(rename = "isInlineXBRL")]
    is_inline_xbrl: Vec<serde_json::Value>,
    primary_document: Vec<Option<String>>,
    primary_doc_description: Vec<Option<String>>,
}

impl FilingColumns {
    /// One [`Filing`] per row, built the way [`Client::filings`] builds them.
    fn to_filings(&self, cik: u64, company: &str) -> Vec<Filing> {
        let text = |v: &[Option<String>], i: usize| v.get(i).cloned().flatten().filter(|s| !s.is_empty());
        let flag = |v: &[serde_json::Value], i: usize| {
            v.get(i).is_some_and(|x| x.as_u64() == Some(1) || x.as_bool() == Some(true))
        };
        (0..self.accession_number.len())
            .map(|i| {
                let mut f = Filing {
                    cik,
                    company: company.to_owned(),
                    accession_number: self.accession_number[i].clone(),
                    form: self.form.get(i).cloned().unwrap_or_default(),
                    filing_date: self.filing_date.get(i).cloned().unwrap_or_default(),
                    report_date: text(&self.report_date, i),
                    acceptance_datetime: text(&self.acceptance_date_time, i),
                    primary_document: text(&self.primary_document, i),
                    primary_doc_description: text(&self.primary_doc_description, i),
                    items: text(&self.items, i)
                        .map(|s| s.split(',').map(|x| x.trim().to_owned()).filter(|x| !x.is_empty()).collect())
                        .unwrap_or_default(),
                    size: self.size.get(i).copied().flatten(),
                    is_xbrl: flag(&self.is_xbrl, i),
                    is_inline_xbrl: flag(&self.is_inline_xbrl, i),
                    index_url: String::new(),
                    primary_document_url: None,
                };
                let folder = f.folder_url();
                f.index_url = format!("{folder}/{}-index.htm", f.accession_number);
                f.primary_document_url = f.primary_document.as_ref().map(|d| format!("{folder}/{d}"));
                f
            })
            .collect()
    }
}

/// The older history pages a company crawl reads. None when the recent block
/// already reaches back past `since`; otherwise every page that overlaps
/// `since` to `until`, so a crawl with no lower bound reads the whole history.
fn pages_to_read<'a>(
    pages: &'a [HistoryPage],
    oldest_recent: Option<&str>,
    since: Option<&str>,
    until: Option<&str>,
) -> Vec<&'a HistoryPage> {
    // Filings of the oldest recent day can continue on a page, so a `since` on that day reads it.
    if let (Some(s), Some(oldest)) = (since, oldest_recent) {
        if s > oldest {
            return Vec::new();
        }
    }
    pages
        .iter()
        .filter(|p| {
            let ends_before = since.zip(p.filing_to.as_deref()).is_some_and(|(s, to)| to < s);
            let starts_after =
                until.zip(p.filing_from.as_deref()).is_some_and(|(u, from)| from.get(..u.len()).unwrap_or(from) > u);
            !ends_before && !starts_after
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cik: u64, form: &str, date: &str, acc: &str) -> IndexEntry {
        IndexEntry {
            cik,
            company: format!("CO {cik}"),
            form: form.into(),
            filing_date: date.into(),
            accession_number: acc.into(),
        }
    }

    fn q(y: u16, n: u8) -> Quarter {
        Quarter::new(y, n).unwrap()
    }

    #[test]
    fn quarters_from_years_and_dates() {
        let years = CrawlQuery::new(&["10-K"]).years(2022, 2023);
        assert_eq!(years.quarters_to_read().unwrap().len(), 8);

        let days = CrawlQuery::new(&["8-K"]).since("2024-11-05").until("2024-11-06");
        assert_eq!(days.quarters_to_read().unwrap(), [q(2024, 4)]);

        let span = CrawlQuery::new(&["8-K"]).since("2023-12").until("2024-04");
        assert_eq!(span.quarters_to_read().unwrap(), [q(2023, 4), q(2024, 1), q(2024, 2)]);

        // Quarters outside the date bounds are not read.
        let narrowed = CrawlQuery::new(&["10-K"]).years(2024, 2024).since("2024-05-01");
        assert_eq!(narrowed.quarters_to_read().unwrap(), [q(2024, 2), q(2024, 3), q(2024, 4)]);

        // Future quarters have no index yet.
        let future = CrawlQuery::new(&["10-K"]).quarter(q(9000, 1));
        assert!(future.quarters_to_read().unwrap().is_empty());

        // EDGAR starts in 1993.
        assert_eq!(CrawlQuery::new(&["10-K"]).years(1990, 1993).quarters.len(), 4);

        assert!(CrawlQuery::new(&["10-K"]).quarters_to_read().is_err());
        assert!(CrawlQuery::new(&["10-K"]).since("2024/01/01").quarters_to_read().is_err());
        let bad = r#"{"forms":["10-K"],"quarters":[{"year":2024,"quarter":7}],"since":null,"until":null}"#;
        assert!(serde_json::from_str::<CrawlQuery>(bad).is_err());
    }

    #[test]
    fn matches_forms_dates_and_quarters() {
        let query = CrawlQuery::new(&["10-k", "10-K/A"]).since("2024-02-01").until("2024-11");
        assert!(query.matches("10-K", "2024-02-01"));
        assert!(query.matches("10-K/A", "2024-11-30"));
        assert!(!query.matches("10-K", "2024-01-31"));
        assert!(!query.matches("10-K", "2024-12-01"));
        assert!(!query.matches("10-KT", "2024-06-01"));

        let q1_q3 = CrawlQuery::new(&["8-K"]).quarter(q(2024, 1)).quarter(q(2024, 3));
        assert!(q1_q3.matches("8-K", "2024-08-15"));
        assert!(!q1_q3.matches("8-K", "2024-05-15"));
    }

    #[test]
    fn select_filters_and_dedupes_co_registrants() {
        let query = CrawlQuery::new(&["10-K"]).years(2024, 2024);
        let rows = vec![
            row(1001, "10-K", "2024-03-01", "0000002000-24-000001"),
            row(2000, "10-K", "2024-03-01", "0000002000-24-000001"),
            row(1002, "10-K", "2024-03-01", "0000950170-24-000002"),
            row(1003, "10-K", "2024-03-01", "0000950170-24-000002"),
            row(1004, "8-K", "2024-03-01", "0000950170-24-000003"),
        ];
        let kept = query.select(rows);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].cik, 2000);
        assert_eq!(kept[1].cik, 1002);
    }

    #[test]
    fn document_extension() {
        let d = |url: &str| Document { url: url.into(), bytes: vec![] };
        assert_eq!(d("https://www.sec.gov/Archives/edgar/data/1/2/a-20240928.htm").extension(), "htm");
        assert_eq!(d("https://www.sec.gov/Archives/edgar/data/1/2/0001-24-1.txt").extension(), "txt");
        assert_eq!(d("https://www.sec.gov/Archives/edgar/data/1/2/noext").extension(), "txt");
        // The SEC serves XSL-rendered forms as HTML.
        assert_eq!(d("https://www.sec.gov/Archives/edgar/data/1/2/xslF345X05/wk-form4_1.xml").extension(), "htm");
        assert_eq!(d("https://www.sec.gov/Archives/edgar/data/1/2/wk-form4_1.xml").extension(), "xml");
    }

    #[test]
    fn company_date_bounds() {
        let bounds = |q: CrawlQuery| q.date_bounds().unwrap();
        let s = |v: &str| Some(v.to_owned());
        assert_eq!(bounds(CrawlQuery::new(&["10-K"]).until("2015")), (None, s("2015")));
        assert_eq!(bounds(CrawlQuery::new(&["10-K"]).since("2010")), (s("2010"), None));
        assert_eq!(bounds(CrawlQuery::new(&["10-K"]).years(2015, 2016)), (s("2015-01-01"), s("2016-12-31")));
        assert_eq!(
            bounds(CrawlQuery::new(&["10-K"]).years(2015, 2016).since("2015-06-01").until("2016-03")),
            (s("2015-06-01"), s("2016-03"))
        );
        assert!(CrawlQuery::new(&["10-K"]).until("2015/01").date_bounds().is_err());
    }

    #[test]
    fn years_with_no_edgar_filings_match_nothing() {
        // Every year before EDGAR starts: no quarters, but bounds that exclude
        // every filing instead of no bounds at all.
        let q = CrawlQuery::new(&["10-K"]).years(1990, 1992);
        assert!(q.quarters.is_empty());
        assert!(q.has_range());
        assert!(!q.matches("10-K", "1994-12-13"));
        assert!(!q.matches("10-K", "2026-11-03"));
        assert!(q.quarters_to_read().unwrap().is_empty());
        let (since, until) = q.date_bounds().unwrap();
        assert_eq!((since.as_deref(), until.as_deref()), (Some("1990"), Some("1992")));
        // A span that ends before it starts.
        let q = CrawlQuery::new(&["10-K"]).years(2020, 2019);
        assert!(!q.matches("10-K", "2019-06-01") && !q.matches("10-K", "2020-06-01"));
        assert!(q.quarters_to_read().unwrap().is_empty());
        // A span that starts before EDGAR keeps its EDGAR years.
        let q = CrawlQuery::new(&["10-K"]).years(1991, 1993);
        assert_eq!(q.quarters.len(), 4);
        assert!(q.since.is_none() && q.until.is_none());
    }

    #[test]
    fn company_history_pages() {
        let page = |name: &str, from: &str, to: &str| HistoryPage {
            name: name.into(),
            filing_from: Some(from.into()),
            filing_to: Some(to.into()),
        };
        let pages = [page("p1.json", "2014-02-01", "2025-11-01"), page("p2.json", "1994-01-01", "2014-01-31")];
        let names = |since: Option<&str>, until: Option<&str>| -> Vec<String> {
            pages_to_read(&pages, Some("2025-11-02"), since, until).iter().map(|p| p.name.clone()).collect()
        };
        // Only an upper bound: every page up to it. This used to read none.
        assert_eq!(names(None, Some("2015")), ["p1.json", "p2.json"]);
        assert_eq!(names(None, Some("2013")), ["p2.json"]);
        // No bounds at all: the whole history.
        assert_eq!(names(None, None), ["p1.json", "p2.json"]);
        // A lower bound inside the recent block needs no pages.
        assert!(names(Some("2026"), None).is_empty());
        // The oldest recent day may continue on a page.
        assert_eq!(names(Some("2025-11-02"), None), Vec::<String>::new());
        assert_eq!(pages_to_read(&pages, Some("2025-11-01"), Some("2025-11-01"), None).len(), 1);
        assert_eq!(names(Some("2010"), Some("2015")), ["p1.json", "p2.json"]);
        assert_eq!(names(Some("2015"), Some("2016")), ["p1.json"]);
    }

    #[test]
    fn parses_submissions_with_profile() {
        let json = r#"{"cik":"0000320193","name":"Apple Inc.","sic":"3571","sicDescription":"Electronic Computers",
            "stateOfIncorporation":"CA","fiscalYearEnd":"0926","tickers":["AAPL"],
            "addresses":{"business":{"stateOrCountry":"CA"}},
            "filings":{"recent":{"accessionNumber":["0000320193-25-000079","0001214156-25-000011"],
                "filingDate":["2025-10-31","2025-10-03"],"reportDate":["2025-09-27","2025-10-01"],
                "acceptanceDateTime":["2025-10-31T10:01:26.000Z","2025-10-03T22:33:53.000Z"],
                "form":["10-K","4"],"items":["",""],"size":[1,2],"isXBRL":[1,0],"isInlineXBRL":[1,0],
                "primaryDocument":["aapl-20250927.htm","xslF345X05/wk-form4_1759530830.xml"],
                "primaryDocDescription":["10-K","FORM 4"]},
                "files":[{"name":"CIK0000320193-submissions-001.json","filingCount":1000,
                    "filingFrom":"1994-01-26","filingTo":"2015-01-01"}]}}"#;
        let sub: CompanySubmissions = serde_json::from_str(json).unwrap();
        let filings = sub.filings.recent.to_filings(320193, &sub.profile.name);
        assert_eq!(filings.len(), 2);
        assert_eq!(filings[0].company, "Apple Inc.");
        assert_eq!(
            filings[1].primary_document_url.as_deref(),
            Some(
                "https://www.sec.gov/Archives/edgar/data/320193/000121415625000011/xslF345X05/wk-form4_1759530830.xml"
            )
        );
        assert_eq!(
            filings[0].index_url,
            "https://www.sec.gov/Archives/edgar/data/320193/000032019325000079/0000320193-25-000079-index.htm"
        );
        assert!(filings[0].is_xbrl && filings[0].is_inline_xbrl && filings[0].items.is_empty());
        assert_eq!(sub.filings.files[0].filing_to.as_deref(), Some("2015-01-01"));
        let profile = sub.profile.into_profile(320193);
        assert_eq!(profile.sic.as_deref(), Some("3571"));
        assert_eq!(profile.state_location.as_deref(), Some("CA"));
        assert_eq!(profile.fiscal_year_end.as_deref(), Some("0926"));
    }
}
