use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::company::parse_cik;
use crate::error::{Error, Result};
use crate::html::document_to_text;
use crate::profile::SubmissionsProfile;
use crate::sections::{FormKind, Section, extract_sections};

/// One EDGAR filing, as listed by the SEC submissions API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filing {
    pub cik: u64,
    pub company: String,
    /// Dashed form, e.g. `0000320193-24-000123`.
    pub accession_number: String,
    pub form: String,
    /// `YYYY-MM-DD`.
    pub filing_date: String,
    #[serde(default)]
    pub report_date: Option<String>,
    #[serde(default)]
    pub acceptance_datetime: Option<String>,
    #[serde(default)]
    pub primary_document: Option<String>,
    #[serde(default)]
    pub primary_doc_description: Option<String>,
    /// 8-K item numbers, e.g. `["2.02", "9.01"]`.
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub is_xbrl: bool,
    #[serde(default)]
    pub is_inline_xbrl: bool,
    #[serde(default)]
    pub index_url: String,
    #[serde(default)]
    pub primary_document_url: Option<String>,
}

impl Filing {
    /// Folder holding every document of this filing.
    pub fn folder_url(&self) -> String {
        format!("https://www.sec.gov/Archives/edgar/data/{}/{}", self.cik, self.accession_number.replace('-', ""))
    }

    /// The complete submission text file (all documents, SGML-wrapped).
    pub fn full_submission_url(&self) -> String {
        format!("{}/{}.txt", self.folder_url(), self.accession_number)
    }

    fn fill_urls(&mut self) {
        self.index_url = format!("{}/{}-index.htm", self.folder_url(), self.accession_number);
        self.primary_document_url =
            self.primary_document.as_ref().filter(|d| !d.is_empty()).map(|d| format!("{}/{}", self.folder_url(), d));
    }
}

/// Filters for [`Client::filings`].
///
/// By default only the submissions API's "recent" block is read (the latest
/// 1,000 filings or one year, whichever is more). Older pages are fetched when
/// `since` reaches back to that block's oldest day or before it, or when
/// `all_history` is set.
#[derive(Debug, Clone, Default)]
pub struct FilingQuery {
    /// Exact form types to keep (case-insensitive), e.g. `10-K`, `10-K/A`. Empty keeps all.
    pub forms: Vec<String>,
    /// Earliest filing date, `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, inclusive.
    pub since: Option<String>,
    /// Latest filing date, `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, inclusive.
    pub until: Option<String>,
    /// Keep at most this many filings, newest first.
    pub limit: Option<usize>,
    /// Read every page of the company's filing history.
    pub all_history: bool,
}

impl FilingQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn forms(forms: &[&str]) -> Self {
        Self { forms: forms.iter().map(|f| f.to_string()).collect(), ..Self::default() }
    }

    pub fn since(mut self, date: impl Into<String>) -> Self {
        self.since = Some(date.into());
        self
    }

    pub fn until(mut self, date: impl Into<String>) -> Self {
        self.until = Some(date.into());
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    pub fn all_history(mut self, yes: bool) -> Self {
        self.all_history = yes;
        self
    }

    fn validate(&self) -> Result<()> {
        for d in [&self.since, &self.until].into_iter().flatten() {
            let ok = matches!(d.len(), 4 | 7 | 10)
                && d.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
            if !ok {
                return Err(Error::Invalid(format!("dates must be YYYY, YYYY-MM or YYYY-MM-DD, got {d:?}")));
            }
        }
        Ok(())
    }

    fn scope(&self) -> HistoryScope<'_> {
        HistoryScope {
            since: self.since.as_deref(),
            until: self.until.as_deref(),
            all_history: self.all_history,
            limit: self.limit,
        }
    }

    fn matches(&self, f: &Filing) -> bool {
        if !self.forms.is_empty() && !self.forms.iter().any(|x| x.eq_ignore_ascii_case(&f.form)) {
            return false;
        }
        if let Some(since) = &self.since
            && f.filing_date.as_str() < since.as_str()
        {
            return false;
        }
        if let Some(until) = &self.until {
            let prefix = f.filing_date.get(..until.len()).unwrap_or(&f.filing_date);
            if prefix > until.as_str() {
                return false;
            }
        }
        true
    }
}

/// A submissions API response: the company's profile, and its filings. The
/// latest filings are in the recent block; older ones are on history pages,
/// one JSON file each. A response without a filings block has no filings.
#[derive(Deserialize)]
pub(crate) struct Submissions {
    #[serde(flatten)]
    pub profile: SubmissionsProfile,
    #[serde(default)]
    pub filings: SubmissionFilings,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct SubmissionFilings {
    pub recent: Columns,
    pub files: Vec<HistoryPage>,
}

/// One history page: its file name and the filing dates it covers.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryPage {
    pub name: String,
    #[serde(default)]
    pub filing_from: Option<String>,
    #[serde(default)]
    pub filing_to: Option<String>,
}

/// The parts of a query that decide which history pages to read.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HistoryScope<'a> {
    /// Earliest filing date, inclusive.
    pub since: Option<&'a str>,
    /// Latest filing date, inclusive, compared by prefix.
    pub until: Option<&'a str>,
    /// Without `since`, read every page up to `until` instead of the recent block only.
    pub all_history: bool,
    /// Only the newest `limit` matches are kept.
    pub limit: Option<usize>,
}

impl HistoryScope<'_> {
    /// The history pages that can hold filings in the date range, in the order
    /// listed. Every page is older than the recent block, so none are needed
    /// when `since` is after the recent block's oldest day (`oldest_recent`).
    /// Without `since`, every page up to `until` with `all_history`, else none.
    fn pages<'p>(&self, pages: &'p [HistoryPage], oldest_recent: Option<&str>) -> Vec<&'p HistoryPage> {
        let needed = match self.since {
            // Filings of the oldest recent day can continue on a page, so a `since` on that day reads it.
            Some(since) => oldest_recent.is_none_or(|oldest| since <= oldest),
            None => self.all_history,
        };
        if !needed {
            return Vec::new();
        }
        pages
            .iter()
            .filter(|p| {
                let ends_before = self.since.zip(p.filing_to.as_deref()).is_some_and(|(s, to)| to < s);
                let starts_after = self
                    .until
                    .zip(p.filing_from.as_deref())
                    .is_some_and(|(u, from)| from.get(..u.len()).unwrap_or(from) > u);
                !ends_before && !starts_after
            })
            .collect()
    }
}

/// The submissions API stores filings column by column.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Columns {
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

impl Columns {
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
                f.fill_urls();
                f
            })
            .collect()
    }
}

impl Client {
    pub(crate) fn submissions(&self, cik: u64) -> Result<Submissions> {
        self.get_json(&format!("https://data.sec.gov/submissions/CIK{cik:010}.json"))
    }

    fn history_page(&self, page: &HistoryPage) -> Result<Columns> {
        self.get_json(&format!("https://data.sec.gov/submissions/{}", page.name))
    }

    pub(crate) fn cik_of(&self, company: &str) -> Result<u64> {
        match parse_cik(company.trim()) {
            Some(cik) => Ok(cik),
            None => Ok(self.company(company)?.cik),
        }
    }

    /// Filings of a company (ticker or CIK), newest first.
    pub fn filings(&self, company: &str, query: &FilingQuery) -> Result<Vec<Filing>> {
        self.filings_for_cik(self.cik_of(company)?, query)
    }

    /// Filings of a company by CIK, newest first.
    pub fn filings_for_cik(&self, cik: u64, query: &FilingQuery) -> Result<Vec<Filing>> {
        query.validate()?;
        let sub = self.submissions(cik)?;
        self.submission_filings(cik, &sub, query.scope(), |f| query.matches(f))
    }

    /// The filings of a submissions response that `keep` accepts, newest first:
    /// those in the recent block, then those on each history page `scope`
    /// selects, at most `scope.limit` of them. [`Client::filings`] and
    /// [`Client::crawl_company`] both list filings this way.
    pub(crate) fn submission_filings(
        &self,
        cik: u64,
        sub: &Submissions,
        scope: HistoryScope<'_>,
        keep: impl Fn(&Filing) -> bool,
    ) -> Result<Vec<Filing>> {
        let name = &sub.profile.name;
        let recent = sub.filings.recent.to_filings(cik, name);
        let oldest_recent = recent.iter().map(|f| f.filing_date.as_str()).min().map(str::to_owned);
        let mut out: Vec<Filing> = recent.into_iter().filter(|f| keep(f)).collect();
        sort_newest_first(&mut out);
        let enough = scope.limit.is_some_and(|n| out.len() >= n);
        if !enough {
            for page in scope.pages(&sub.filings.files, oldest_recent.as_deref()) {
                // With a limit, a page older than the limit-th newest match cannot change the result.
                if let (Some(n), Some(to)) = (scope.limit, &page.filing_to)
                    && n > 0
                    && out.len() >= n
                {
                    sort_newest_first(&mut out);
                    if out.len() >= n && to.as_str() < out[n - 1].filing_date.as_str() {
                        continue;
                    }
                }
                let cols = self.history_page(page)?;
                out.extend(cols.to_filings(cik, name).into_iter().filter(|f| keep(f)));
            }
        }
        sort_newest_first(&mut out);
        if let Some(n) = scope.limit {
            out.truncate(n);
        }
        Ok(out)
    }

    /// Find one filing by company and accession number (dashed or 18 digits).
    /// Reads the submissions file once, then only as many history pages as needed,
    /// starting with those that cover the year in the accession number.
    pub fn filing(&self, company: &str, accession: &str) -> Result<Filing> {
        let acc = normalize_accession(accession)
            .ok_or_else(|| Error::Invalid(format!("not an accession number: {accession:?}")))?;
        let cik = self.cik_of(company)?;
        let sub = self.submissions(cik)?;
        let find =
            |cols: &Columns| cols.to_filings(cik, &sub.profile.name).into_iter().find(|f| f.accession_number == acc);
        if let Some(f) = find(&sub.filings.recent) {
            return Ok(f);
        }
        let year = accession_year(&acc);
        let (likely, others): (Vec<&HistoryPage>, Vec<&HistoryPage>) =
            sub.filings.files.iter().partition(|p| page_covers_years(p, year, year + 1));
        for page in likely.into_iter().chain(others) {
            if let Some(f) = find(&self.history_page(page)?) {
                return Ok(f);
            }
        }
        Err(Error::FilingNotFound(acc))
    }

    /// The most recent filing of a form type.
    pub fn latest(&self, company: &str, form: &str) -> Result<Filing> {
        // History pages are read only when the recent block has none.
        let q = FilingQuery::forms(&[form]).limit(1).all_history(true);
        self.filings(company, &q)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::FilingNotFound(format!("no {form} filings for {company}")))
    }

    /// Raw primary document (usually HTML) of a filing.
    pub fn primary_document(&self, filing: &Filing) -> Result<String> {
        let url = match &filing.primary_document_url {
            Some(u) => u.clone(),
            // Very old filings have no separate primary document: use the full submission.
            None => filing.full_submission_url(),
        };
        self.get_text(&url)
    }

    /// Clean plain text of a filing's primary document.
    pub fn filing_text(&self, filing: &Filing) -> Result<String> {
        Ok(document_to_text(&self.primary_document(filing)?))
    }

    /// Item sections of a 10-K, 10-Q or 8-K filing (and their amendments and variants).
    pub fn filing_sections(&self, filing: &Filing) -> Result<Vec<Section>> {
        let kind = FormKind::from_form(&filing.form).ok_or_else(|| {
            Error::Invalid(format!("sections are supported for 10-K, 10-Q and 8-K filings, not {}", filing.form))
        })?;
        Ok(extract_sections(&self.filing_text(filing)?, kind))
    }
}

/// Sort newest first, keeping one row per filing when one is listed twice.
fn sort_newest_first(filings: &mut Vec<Filing>) {
    filings.sort_by(|a, b| b.filing_date.cmp(&a.filing_date).then_with(|| b.accession_number.cmp(&a.accession_number)));
    filings.dedup_by(|a, b| a.accession_number == b.accession_number);
}

/// The year in a dashed accession number: `0000320193-24-000123` -> 2024.
fn accession_year(acc: &str) -> u32 {
    let yy: u32 = acc.get(11..13).and_then(|y| y.parse().ok()).unwrap_or(0);
    if yy >= 80 { 1900 + yy } else { 2000 + yy }
}

/// Whether a history page's date range overlaps the years `first..=last`.
/// A page without a range might hold anything.
fn page_covers_years(page: &HistoryPage, first: u32, last: u32) -> bool {
    let (Some(from), Some(to)) = (&page.filing_from, &page.filing_to) else {
        return true;
    };
    to.as_str() >= format!("{first:04}").as_str() && from.as_str() <= format!("{last:04}-12-31").as_str()
}

/// `0000320193-24-000123` or `000032019324000123` to the dashed form.
fn normalize_accession(s: &str) -> Option<String> {
    let digits: String = s.trim().chars().filter(|c| *c != '-').collect();
    if digits.len() != 18 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{}-{}", &digits[..10], &digits[10..12], &digits[12..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl::CrawlQuery;

    fn filing(form: &str, date: &str) -> Filing {
        let mut f = Filing {
            cik: 320193,
            company: "Apple Inc.".into(),
            accession_number: "0000320193-24-000123".into(),
            form: form.into(),
            filing_date: date.into(),
            report_date: None,
            acceptance_datetime: None,
            primary_document: Some("aapl-20240928.htm".into()),
            primary_doc_description: None,
            items: vec![],
            size: None,
            is_xbrl: false,
            is_inline_xbrl: false,
            index_url: String::new(),
            primary_document_url: None,
        };
        f.fill_urls();
        f
    }

    #[test]
    fn urls() {
        let f = filing("10-K", "2024-11-01");
        assert_eq!(
            f.primary_document_url.as_deref(),
            Some("https://www.sec.gov/Archives/edgar/data/320193/000032019324000123/aapl-20240928.htm")
        );
        assert_eq!(
            f.index_url,
            "https://www.sec.gov/Archives/edgar/data/320193/000032019324000123/0000320193-24-000123-index.htm"
        );
    }

    #[test]
    fn query_filters() {
        let q = FilingQuery::forms(&["10-k"]).since("2024").until("2024-11");
        assert!(q.matches(&filing("10-K", "2024-11-01")));
        assert!(q.matches(&filing("10-K", "2024-01-01")));
        assert!(!q.matches(&filing("10-K", "2024-12-01")));
        assert!(!q.matches(&filing("10-K", "2023-12-31")));
        assert!(!q.matches(&filing("10-K/A", "2024-06-01")));
    }

    #[test]
    fn date_validation() {
        assert!(FilingQuery::new().since("2024-01-01").validate().is_ok());
        assert!(FilingQuery::new().since("2024/01/01").validate().is_err());
        assert!(FilingQuery::new().until("24").validate().is_err());
    }

    #[test]
    fn accessions() {
        assert_eq!(normalize_accession("000032019324000123").as_deref(), Some("0000320193-24-000123"));
        assert_eq!(normalize_accession("0000320193-24-000123").as_deref(), Some("0000320193-24-000123"));
        assert_eq!(normalize_accession("123"), None);
    }

    #[test]
    fn columns_parse() {
        let json = r#"{"accessionNumber":["0000320193-24-000123"],"filingDate":["2024-11-01"],
            "reportDate":["2024-09-28"],"acceptanceDateTime":["2024-11-01T06:01:36.000Z"],"form":["8-K"],
            "items":["2.02,9.01"],"size":[12345],"isXBRL":[1],"isInlineXBRL":[0],
            "primaryDocument":["a.htm"],"primaryDocDescription":["8-K"]}"#;
        let cols: Columns = serde_json::from_str(json).unwrap();
        let f = &cols.to_filings(320193, "Apple Inc.")[0];
        assert_eq!(f.items, vec!["2.02", "9.01"]);
        assert!(f.is_xbrl && !f.is_inline_xbrl);
        assert_eq!(f.size, Some(12345));
    }

    const SUBMISSIONS: &str = "https://data.sec.gov/submissions/CIK0000000001.json";
    const PAGE_1: &str = "https://data.sec.gov/submissions/CIK0000000001-submissions-001.json";
    const PAGE_2: &str = "https://data.sec.gov/submissions/CIK0000000001-submissions-002.json";

    fn columns(rows: &[(&str, &str, &str)]) -> String {
        serde_json::json!({
            "accessionNumber": rows.iter().map(|r| r.0).collect::<Vec<_>>(),
            "form": rows.iter().map(|r| r.1).collect::<Vec<_>>(),
            "filingDate": rows.iter().map(|r| r.2).collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// A filer whose recent block has 8-Ks and one 10-Q, with 10-Ks only in
    /// two older history pages (2015-2025 and 2010-2014). The newer page ends
    /// with an 8-K filed on the recent block's oldest day.
    fn stubbed_client() -> Client {
        let c = Client::builder("a@b.c").build().unwrap();
        let recent = columns(&[
            ("0000000001-25-000003", "8-K", "2025-06-01"),
            ("0000000001-25-000002", "10-Q", "2025-05-01"),
            ("0000000001-25-000001", "8-K", "2025-02-01"),
        ]);
        let files = r#"[
            {"name": "CIK0000000001-submissions-001.json", "filingFrom": "2015-01-01", "filingTo": "2025-02-01"},
            {"name": "CIK0000000001-submissions-002.json", "filingFrom": "2010-01-01", "filingTo": "2014-12-31"}]"#;
        c.stub(SUBMISSIONS, format!(r#"{{"name": "Test Co", "filings": {{"recent": {recent}, "files": {files}}}}}"#));
        let page1 = columns(&[
            ("0000000001-25-000000", "8-K", "2025-02-01"),
            ("0000000001-20-000002", "10-K", "2020-03-01"),
            ("0000000001-19-000001", "10-K", "2019-03-01"),
            ("0000000001-16-000001", "10-K", "2016-03-01"),
        ]);
        c.stub(PAGE_1, page1);
        let page2 = columns(&[
            ("0000000001-13-000001", "10-K", "2013-03-01"),
            // Accession numbers from late December can be filed in January.
            ("0000000001-11-000009", "8-K", "2012-01-03"),
        ]);
        c.stub(PAGE_2, page2);
        c
    }

    #[test]
    fn latest_reads_submissions_once() {
        let c = stubbed_client();
        assert_eq!(c.latest("1", "10-Q").unwrap().accession_number, "0000000001-25-000002");
        assert_eq!(c.take_fetched(), [SUBMISSIONS]);
        // Not in the recent block: the newest history page has one, so the older one is not read.
        assert_eq!(c.latest("1", "10-K").unwrap().accession_number, "0000000001-20-000002");
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1]);
        assert!(matches!(c.latest("1", "S-1"), Err(Error::FilingNotFound(_))));
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1, PAGE_2]);
    }

    #[test]
    fn filing_reads_submissions_once() {
        let c = stubbed_client();
        assert_eq!(c.filing("1", "0000000001-25-000002").unwrap().form, "10-Q");
        assert_eq!(c.take_fetched(), [SUBMISSIONS]);
        // The page covering the accession number's year is read first.
        assert_eq!(c.filing("1", "000000000113000001").unwrap().filing_date, "2013-03-01");
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_2]);
        assert_eq!(c.filing("1", "0000000001-11-000009").unwrap().filing_date, "2012-01-03");
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_2]);
        assert_eq!(c.filing("1", "0000000001-19-000001").unwrap().filing_date, "2019-03-01");
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1]);
        assert!(matches!(c.filing("1", "0000000001-05-000001"), Err(Error::FilingNotFound(_))));
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1, PAGE_2]);
    }

    #[test]
    fn history_stops_at_limit() {
        let c = stubbed_client();
        let q = FilingQuery::forms(&["10-K"]).limit(2).all_history(true);
        let got: Vec<String> = c.filings("1", &q).unwrap().into_iter().map(|f| f.filing_date).collect();
        assert_eq!(got, ["2020-03-01", "2019-03-01"]);
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1]);
        // Without a limit every page is read.
        let q = FilingQuery::forms(&["10-K"]).all_history(true);
        assert_eq!(c.filings("1", &q).unwrap().len(), 4);
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1, PAGE_2]);
    }

    /// The year and sequence part of each accession number, e.g. `25-000003`.
    fn short_accessions(filings: &[Filing]) -> Vec<&str> {
        filings.iter().map(|f| &f.accession_number[11..]).collect()
    }

    /// filings() and crawl_company() read the same history pages and return
    /// the same filings for the same date range.
    #[test]
    fn filings_and_company_crawls_agree() {
        let c = stubbed_client();
        let all = [
            "25-000003",
            "25-000002",
            "25-000001",
            "25-000000",
            "20-000002",
            "19-000001",
            "16-000001",
            "13-000001",
            "11-000009",
        ];
        // since, until, the history pages read, and the filings found.
        type Case<'a> = (Option<&'a str>, Option<&'a str>, &'a [&'a str], &'a [&'a str]);
        let cases: [Case; 6] = [
            // `since` on the recent block's oldest day: the newer page continues that day.
            (Some("2025-02-01"), None, &[PAGE_1], &["25-000003", "25-000002", "25-000001", "25-000000"]),
            (Some("2025-03"), None, &[], &["25-000003", "25-000002"]),
            (Some("2016"), Some("2019"), &[PAGE_1], &["19-000001", "16-000001"]),
            (Some("2012"), Some("2016-06"), &[PAGE_1, PAGE_2], &["16-000001", "13-000001", "11-000009"]),
            // Only an upper bound.
            (None, Some("2014"), &[PAGE_2], &["13-000001", "11-000009"]),
            (None, None, &[PAGE_1, PAGE_2], &all),
        ];
        for (since, until, pages, expected) in cases {
            let fetched: Vec<&str> = [SUBMISSIONS].into_iter().chain(pages.iter().copied()).collect();
            let crawl =
                CrawlQuery { since: since.map(String::from), until: until.map(String::from), ..Default::default() };
            let crawled = c.crawl_company("1", &crawl).unwrap();
            assert_eq!(c.take_fetched(), fetched, "{crawl:?}");
            assert_eq!(short_accessions(&crawled), expected, "{crawl:?}");
            // Without a lower bound, filings() reads older pages only with all_history.
            let query = FilingQuery { since: crawl.since, until: crawl.until, all_history: true, ..Default::default() };
            let mut queries = vec![query.clone()];
            if since.is_some() {
                queries.push(FilingQuery { all_history: false, ..query });
            }
            for query in queries {
                assert_eq!(c.filings("1", &query).unwrap(), crawled, "{query:?}");
                assert_eq!(c.take_fetched(), fetched, "{query:?}");
            }
        }
        // By default filings() reads only the recent block when there is no lower bound.
        assert!(c.filings("1", &FilingQuery::new().until("2014")).unwrap().is_empty());
        assert_eq!(c.take_fetched(), [SUBMISSIONS]);

        // A crawl by years reads the pages for those years' dates, and both filter forms alike.
        let crawled = c.crawl_company("1", &CrawlQuery::new(&["10-K"]).years(2019, 2020)).unwrap();
        assert_eq!(short_accessions(&crawled), ["20-000002", "19-000001"]);
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1]);
        let query = FilingQuery::forms(&["10-K"]).since("2019-01-01").until("2020-12-31");
        assert_eq!(c.filings("1", &query).unwrap(), crawled);
        assert_eq!(c.take_fetched(), [SUBMISSIONS, PAGE_1]);
    }

    #[test]
    fn history_pages() {
        let page = |name: &str, from: &str, to: &str| HistoryPage {
            name: name.into(),
            filing_from: Some(from.into()),
            filing_to: Some(to.into()),
        };
        let pages = [page("p1.json", "2014-02-01", "2025-11-01"), page("p2.json", "1994-01-01", "2014-01-31")];
        let read = |since: Option<&str>, until: Option<&str>, all_history: bool, oldest_recent: Option<&str>| {
            let scope = HistoryScope { since, until, all_history, limit: None };
            scope.pages(&pages, oldest_recent).iter().map(|p| p.name.clone()).collect::<Vec<_>>()
        };
        let names = |since: Option<&str>, until: Option<&str>| read(since, until, true, Some("2025-11-02"));
        // Only an upper bound: every page up to it. Company crawls used to read none.
        assert_eq!(names(None, Some("2015")), ["p1.json", "p2.json"]);
        assert_eq!(names(None, Some("2013")), ["p2.json"]);
        // No bounds at all: the whole history.
        assert_eq!(names(None, None), ["p1.json", "p2.json"]);
        // A lower bound inside the recent block needs no pages.
        assert!(names(Some("2026"), None).is_empty());
        // The oldest recent day may continue on a page.
        assert!(names(Some("2025-11-02"), None).is_empty());
        assert_eq!(read(Some("2025-11-01"), None, true, Some("2025-11-01")), ["p1.json"]);
        assert_eq!(names(Some("2010"), Some("2015")), ["p1.json", "p2.json"]);
        assert_eq!(names(Some("2015"), Some("2016")), ["p1.json"]);
        // Without all_history, no lower bound means the recent block only.
        assert!(read(None, Some("2015"), false, Some("2025-11-02")).is_empty());
        assert_eq!(read(Some("2015"), Some("2016"), false, Some("2025-11-02")), ["p1.json"]);
        // With no recent filings, the pages' dates decide.
        assert_eq!(read(Some("2015"), None, false, None), ["p1.json"]);
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
        let sub: Submissions = serde_json::from_str(json).unwrap();
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

        // A response without a filings block has no filings.
        let sub: Submissions = serde_json::from_str(r#"{"name":"Some Trust"}"#).unwrap();
        assert!(sub.filings.recent.to_filings(42, &sub.profile.name).is_empty() && sub.filings.files.is_empty());
    }
}
