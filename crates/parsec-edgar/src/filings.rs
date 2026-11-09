use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::company::parse_cik;
use crate::error::{Error, Result};
use crate::html::document_to_text;
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
/// `since` reaches back before that block, or when `all_history` is set.
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

    fn matches(&self, f: &Filing) -> bool {
        if !self.forms.is_empty() && !self.forms.iter().any(|x| x.eq_ignore_ascii_case(&f.form)) {
            return false;
        }
        if let Some(since) = &self.since {
            if f.filing_date.as_str() < since.as_str() {
                return false;
            }
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

#[derive(Deserialize)]
pub(crate) struct Submissions {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tickers: Vec<Option<String>>,
    #[serde(default)]
    pub exchanges: Vec<Option<String>>,
    pub filings: SubmissionFilings,
}

#[derive(Deserialize)]
pub(crate) struct SubmissionFilings {
    pub recent: Columns,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

#[derive(Deserialize)]
pub(crate) struct FileRef {
    pub name: String,
    #[serde(rename = "filingFrom", default)]
    pub filing_from: Option<String>,
    #[serde(rename = "filingTo", default)]
    pub filing_to: Option<String>,
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

    /// Filings of a company (ticker or CIK), newest first.
    pub fn filings(&self, company: &str, query: &FilingQuery) -> Result<Vec<Filing>> {
        let cik = match parse_cik(company.trim()) {
            Some(cik) => cik,
            None => self.company(company)?.cik,
        };
        self.filings_for_cik(cik, query)
    }

    /// Filings of a company by CIK, newest first.
    pub fn filings_for_cik(&self, cik: u64, query: &FilingQuery) -> Result<Vec<Filing>> {
        query.validate()?;
        let sub = self.submissions(cik)?;
        let all_recent = sub.filings.recent.to_filings(cik, &sub.name);
        let oldest_recent = all_recent.iter().map(|f| f.filing_date.as_str()).min().unwrap_or("").to_owned();
        let mut out: Vec<Filing> = all_recent.into_iter().filter(|f| query.matches(f)).collect();

        let reaches_back = query.since.as_deref().is_some_and(|s| s < oldest_recent.as_str());
        let enough = query.limit.is_some_and(|n| out.len() >= n);
        if (query.all_history || reaches_back) && !enough {
            for page in &sub.filings.files {
                // Each page covers a date range: skip pages entirely outside the query.
                if let (Some(since), Some(to)) = (&query.since, &page.filing_to) {
                    if to.as_str() < since.as_str() {
                        continue;
                    }
                }
                if let (Some(until), Some(from)) = (&query.until, &page.filing_from) {
                    if from.get(..until.len()).unwrap_or(from) > until.as_str() {
                        continue;
                    }
                }
                let cols: Columns = self.get_json(&format!("https://data.sec.gov/submissions/{}", page.name))?;
                out.extend(cols.to_filings(cik, &sub.name).into_iter().filter(|f| query.matches(f)));
            }
        }
        out.sort_by(|a, b| b.filing_date.cmp(&a.filing_date).then_with(|| b.accession_number.cmp(&a.accession_number)));
        if let Some(n) = query.limit {
            out.truncate(n);
        }
        Ok(out)
    }

    /// Find one filing by company and accession number (dashed or 18 digits).
    pub fn filing(&self, company: &str, accession: &str) -> Result<Filing> {
        let acc = normalize_accession(accession)
            .ok_or_else(|| Error::Invalid(format!("not an accession number: {accession:?}")))?;
        let find = |q: &FilingQuery| -> Result<Option<Filing>> {
            Ok(self.filings(company, q)?.into_iter().find(|f| f.accession_number == acc))
        };
        if let Some(f) = find(&FilingQuery::new())? {
            return Ok(f);
        }
        find(&FilingQuery::new().all_history(true))?.ok_or(Error::FilingNotFound(acc))
    }

    /// The most recent filing of a form type.
    pub fn latest(&self, company: &str, form: &str) -> Result<Filing> {
        let q = FilingQuery::forms(&[form]).limit(1);
        if let Some(f) = self.filings(company, &q)?.into_iter().next() {
            return Ok(f);
        }
        self.filings(company, &q.all_history(true))?
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
}
