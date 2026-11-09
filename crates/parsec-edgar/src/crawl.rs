//! Bulk crawling: find filings by form and date across all companies (from the
//! full index) or for a list of companies (from the submissions API).
//!
//! Requests per filing:
//! - full index: one `master.gz` per quarter (about 3 MB), then for each filing
//!   its `-index.htm` page and its primary document, so 2 per filing;
//! - company list: one submissions request per company (more for old filings),
//!   then the primary document, so 1 per filing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::encoding;
use crate::error::{Error, Result};
use crate::filings::{Filing, FilingQuery};
use crate::html::document_to_text;
use crate::index::{IndexEntry, Quarter, parse_date, today};

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

    /// Add every quarter of the years `from` to `to`, inclusive.
    pub fn years(mut self, from: u16, to: u16) -> Self {
        for year in from..=to {
            self.quarters.extend((1..=4).map(|quarter| Quarter { year, quarter }));
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
        let current = Quarter::current();
        let mut qs = if self.quarters.is_empty() {
            let since = self.since.as_deref().ok_or_else(|| {
                Error::Invalid("give years or quarters, or a since date, to crawl the full index".into())
            })?;
            let until = self.until.clone().unwrap_or_else(today);
            Quarter::range(Quarter::of_date(since)?, Quarter::of_date(&until)?.min(current))
        } else {
            let mut qs = Vec::with_capacity(self.quarters.len());
            for q in &self.quarters {
                qs.push(Quarter::new(q.year, q.quarter)?);
            }
            qs
        };
        qs.sort();
        qs.dedup();
        qs.retain(|q| *q <= current && self.overlaps(q));
        Ok(qs)
    }

    fn overlaps(&self, q: &Quarter) -> bool {
        let after_since = self.since.as_deref().is_none_or(|s| q.last_day().as_str() >= s);
        let before_until =
            self.until.as_deref().is_none_or(|u| q.first_day().get(..u.len()).unwrap_or("") <= u);
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

    /// File extension from the URL, e.g. `htm` or `txt`.
    pub fn extension(&self) -> &str {
        let name = self.url.rsplit('/').next().unwrap_or("");
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

    /// Filings of one company (ticker or CIK) through [`Client::filings`], newest
    /// first. Without any date range, reads the company's whole history.
    pub fn crawl_company(&self, company: &str, query: &CrawlQuery) -> Result<Vec<Filing>> {
        let mut since = query.since.clone();
        let mut until = query.until.clone();
        if let (Some(first), Some(last)) = (query.quarters.iter().min(), query.quarters.iter().max()) {
            let lo = first.first_day();
            let hi = last.last_day();
            since = Some(since.map_or(lo.clone(), |s| s.max(lo)));
            // `until` matches by prefix, so the quarter end is tighter when its prefix is within `u`.
            until = Some(match until {
                Some(u) if hi.get(..u.len()).unwrap_or(&hi) > u.as_str() => u,
                _ => hi,
            });
        }
        let fq = FilingQuery {
            forms: query.forms.clone(),
            all_history: !query.has_range(),
            since,
            until,
            limit: None,
        };
        let filings = self.filings(company, &fq)?;
        Ok(filings.into_iter().filter(|f| query.matches(&f.form, &f.filing_date)).collect())
    }

    /// The primary document of a filing as served (the complete submission for
    /// old filings without one). One request, cached.
    pub fn fetch_primary_document(&self, filing: &Filing) -> Result<Document> {
        let url = filing.primary_document_url.clone().unwrap_or_else(|| filing.full_submission_url());
        let bytes = self.get_bytes(&url)?;
        Ok(Document { url, bytes })
    }
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
        let future = CrawlQuery::new(&["10-K"]).quarter(Quarter { year: 9000, quarter: 1 });
        assert!(future.quarters_to_read().unwrap().is_empty());

        assert!(CrawlQuery::new(&["10-K"]).quarters_to_read().is_err());
        assert!(CrawlQuery::new(&["10-K"]).since("2024/01/01").quarters_to_read().is_err());
        assert!(CrawlQuery::new(&["10-K"]).quarter(Quarter { year: 2024, quarter: 7 }).quarters_to_read().is_err());
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
    }
}
