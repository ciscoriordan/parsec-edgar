//! A filing's `-index.htm` page: its documents with their types, its filers
//! with their company facts, the report period, acceptance time and 8-K items.
//!
//! The full index only gives each filing's complete submission (`.txt`). One
//! request for this small page (10 to 40 KB) finds the primary document, so a
//! crawl needs two requests per filing: this page and the document.

use std::sync::LazyLock;

use regex::Regex;
use scraper::{ElementRef, Html, Node, Selector};
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::filings::Filing;
use crate::index::{IndexEntry, civil_from_days, dashed_accession, days_from_civil};
use crate::profile::CompanyProfile;

/// The parsed `-index.htm` page of one filing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FilingIndex {
    /// Form type as shown in the page header, e.g. `10-K`.
    pub form: Option<String>,
    /// `YYYY-MM-DD`.
    pub filing_date: Option<String>,
    /// Acceptance time as printed, US Eastern time: `YYYY-MM-DD HH:MM:SS`.
    pub accepted: Option<String>,
    /// `YYYY-MM-DD`.
    pub period_of_report: Option<String>,
    /// 8-K item numbers, e.g. `["2.02", "9.01"]`.
    pub items: Vec<String>,
    /// Documents and data files, in page order. The complete submission file is not included.
    pub documents: Vec<IndexDocument>,
    pub filers: Vec<Filer>,
    /// Size in bytes of the complete submission text file.
    pub submission_size: Option<u64>,
}

/// One row of the document tables.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IndexDocument {
    pub seq: Option<u32>,
    pub description: String,
    /// File name in the filing folder. `None` for old filings whose documents
    /// exist only inside the complete submission.
    pub document: Option<String>,
    /// e.g. `10-K`, `EX-21.1`, `GRAPHIC`, `EX-101.SCH`.
    #[serde(rename = "type")]
    pub doc_type: String,
    pub size: Option<u64>,
    /// Marked as inline XBRL.
    pub inline_xbrl: bool,
}

/// A company or person named on the filing, with its role such as `Filer`,
/// `Subject` or `Filed by`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Filer {
    pub role: Option<String>,
    pub profile: CompanyProfile,
}

impl FilingIndex {
    /// The filer with this CIK.
    pub fn filer(&self, cik: u64) -> Option<&Filer> {
        self.filers.iter().find(|f| f.profile.cik == cik)
    }

    /// The main document: the first row whose type equals the form (HTML or text
    /// preferred over PDF), else sequence 1.
    pub fn primary_document(&self, form: &str) -> Option<&IndexDocument> {
        let is_pdf = |d: &&IndexDocument| d.document.as_deref().is_some_and(|n| n.to_ascii_lowercase().ends_with(".pdf"));
        let typed: Vec<&IndexDocument> =
            self.documents.iter().filter(|d| d.doc_type.trim().eq_ignore_ascii_case(form.trim())).collect();
        typed
            .iter()
            .find(|d| d.document.is_some() && !is_pdf(d))
            .or_else(|| typed.first())
            .copied()
            .or_else(|| self.documents.iter().find(|d| d.seq == Some(1)))
    }

    /// Build the [`Filing`] for a full-index row, so [`Client::filing_text`] and
    /// [`Client::filing_sections`] work on it.
    pub fn to_filing(&self, entry: &IndexEntry) -> Filing {
        let folder = entry.folder_url();
        let primary = self.primary_document(&entry.form);
        let document = primary.and_then(|d| d.document.clone());
        let company = self
            .filer(entry.cik)
            .map(|f| f.profile.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| entry.company.clone());
        Filing {
            cik: entry.cik,
            company,
            accession_number: entry.accession_number.clone(),
            form: entry.form.clone(),
            filing_date: entry.filing_date.clone(),
            report_date: self.period_of_report.clone(),
            acceptance_datetime: self.accepted.as_deref().and_then(eastern_to_utc),
            primary_doc_description: primary.map(|d| d.description.clone()).filter(|d| !d.is_empty()),
            primary_document_url: document.as_ref().map(|d| format!("{folder}/{d}")),
            primary_document: document,
            items: self.items.clone(),
            size: self.submission_size,
            is_xbrl: self.documents.iter().any(|d| d.doc_type.to_ascii_uppercase().starts_with("EX-101")),
            is_inline_xbrl: self.documents.iter().any(|d| d.inline_xbrl),
            index_url: entry.index_url(),
        }
    }
}

impl Client {
    /// Fetch and parse a filing's `-index.htm` page. One request, cached.
    /// `accession` may be dashed or 18 digits.
    pub fn filing_index(&self, cik: u64, accession: &str) -> Result<FilingIndex> {
        let accession_number = dashed_accession(accession)
            .ok_or_else(|| Error::Invalid(format!("not an accession number: {accession:?}")))?;
        let entry = IndexEntry {
            cik,
            company: String::new(),
            form: String::new(),
            filing_date: String::new(),
            accession_number,
        };
        let url = entry.index_url();
        let index = parse_filing_index(&self.get_text(&url)?);
        if index.documents.is_empty() && index.filers.is_empty() {
            return Err(Error::Invalid(format!("no document table in {url}")));
        }
        Ok(index)
    }

    /// Resolve a full-index row to a [`Filing`] and its filing index page. One request, cached.
    pub fn resolve_index_entry(&self, entry: &IndexEntry) -> Result<(Filing, FilingIndex)> {
        let index = self.filing_index(entry.cik, &entry.accession_number)?;
        Ok((index.to_filing(entry), index))
    }
}

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("valid selector")
}

/// Parse a filing's `-index.htm` page.
pub fn parse_filing_index(html: &str) -> FilingIndex {
    static FORM: LazyLock<Selector> = LazyLock::new(|| sel("#formName strong"));
    static INFO: LazyLock<Selector> = LazyLock::new(|| sel(".formGrouping > div"));
    static ROW: LazyLock<Selector> = LazyLock::new(|| sel("table.tableFile tr"));
    static TD: LazyLock<Selector> = LazyLock::new(|| sel("td"));
    static LINK: LazyLock<Selector> = LazyLock::new(|| sel("a"));
    static FILER: LazyLock<Selector> = LazyLock::new(|| sel("div.filerDiv"));
    static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Item\s+(\d+(?:\.\d+)?)").unwrap());

    let doc = Html::parse_document(html);
    let mut out = FilingIndex {
        form: doc.select(&FORM).next().map(|e| {
            let t = text_of(e);
            t.strip_prefix("Form ").unwrap_or(&t).trim().to_owned()
        }),
        ..FilingIndex::default()
    };

    // Label and value divs alternate: `infoHead` then `info`.
    let mut label: Option<String> = None;
    for div in doc.select(&INFO) {
        let class = div.value().attr("class").unwrap_or("");
        if class.split_whitespace().any(|c| c == "infoHead") {
            label = Some(text_of(div));
        } else if class.split_whitespace().any(|c| c == "info") {
            let value = text_of(div);
            match label.take().as_deref() {
                Some("Filing Date") => out.filing_date = Some(value),
                Some("Accepted") => out.accepted = Some(value),
                Some("Period of Report") => out.period_of_report = Some(value),
                Some("Items") => out.items = ITEM.captures_iter(&value).map(|c| c[1].to_owned()).collect(),
                _ => {}
            }
        }
    }

    for row in doc.select(&ROW) {
        let cells: Vec<ElementRef> = row.select(&TD).collect();
        if cells.len() < 5 {
            continue;
        }
        let description = text_of(cells[1]);
        let size = text_of(cells[4]).parse().ok();
        if description == "Complete submission text file" {
            out.submission_size = size;
            continue;
        }
        let document = cells[2].select(&LINK).next().map(text_of).filter(|t| !t.is_empty());
        out.documents.push(IndexDocument {
            seq: text_of(cells[0]).parse().ok(),
            description,
            document,
            doc_type: text_of(cells[3]),
            size,
            inline_xbrl: text_of(cells[2]).contains("iXBRL"),
        });
    }

    out.filers = doc.select(&FILER).filter_map(parse_filer).collect();
    out
}

fn parse_filer(div: ElementRef) -> Option<Filer> {
    static NAME: LazyLock<Selector> = LazyLock::new(|| sel("span.companyName"));
    static IDENT: LazyLock<Selector> = LazyLock::new(|| sel("p.identInfo"));
    static MAILER: LazyLock<Selector> = LazyLock::new(|| sel("div.mailer"));
    static STATE: LazyLock<Selector> = LazyLock::new(|| sel("span.mailerAddress span.mailerAddress"));
    static CIK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"CIK\s*:\s*0*(\d+)").unwrap());

    let name_text = text_of(div.select(&NAME).next()?);
    let caps = CIK.captures(&name_text)?;
    let cik: u64 = caps[1].parse().ok()?;
    let mut name = name_text[..caps.get(0).map_or(0, |m| m.start())].trim().to_owned();
    let mut role = None;
    if name.ends_with(')') {
        if let Some(open) = name.rfind('(') {
            role = Some(name[open + 1..name.len() - 1].trim().to_owned());
            name = name[..open].trim().to_owned();
        }
    }
    let mut profile = CompanyProfile { cik, name, ..CompanyProfile::default() };

    if let Some(ident) = div.select(&IDENT).next() {
        // "EIN.: 942404110 | State of Incorp.: CA | Fiscal Year End: 0928", then
        // "Type: 10-K | ..." and "SIC: 3571 Electronic Computers" on later lines.
        for field in lines_of(ident).split(['\n', '|']) {
            let Some((key, value)) = field.split_once(':') else { continue };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key.trim() {
                "EIN." | "EIN" | "IRS No." => profile.ein = Some(value.to_owned()),
                "State of Incorp." => profile.state_of_incorporation = Some(value.to_owned()),
                "Fiscal Year End" => profile.fiscal_year_end = Some(value.to_owned()),
                "SIC" => {
                    let (code, desc) = value.split_once(char::is_whitespace).unwrap_or((value, ""));
                    profile.sic = Some(code.to_owned());
                    profile.sic_description = Some(desc.trim().to_owned()).filter(|d| !d.is_empty());
                }
                _ => {}
            }
        }
    }

    profile.state_location = div
        .select(&MAILER)
        .find(|m| text_of(*m).starts_with("Business Address"))
        .and_then(|m| m.select(&STATE).next())
        .map(text_of)
        .filter(|s| !s.is_empty());
    Some(Filer { role, profile })
}

/// Text of an element with whitespace runs collapsed.
fn text_of(e: ElementRef) -> String {
    e.text().collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text of an element with `<br>` as a line break.
fn lines_of(e: ElementRef) -> String {
    let mut s = String::new();
    for node in e.descendants() {
        match node.value() {
            Node::Text(t) => s.push_str(&t.replace(['\n', '\r'], " ")),
            Node::Element(el) if el.name() == "br" => s.push('\n'),
            _ => {}
        }
    }
    s
}

/// `2024-11-01 06:01:36` (US Eastern, as EDGAR prints it) to `2024-11-01T10:01:36.000Z`,
/// the format the submissions API uses.
pub(crate) fn eastern_to_utc(s: &str) -> Option<String> {
    let s = s.trim();
    let (date, time) = s.split_once([' ', 'T'])?;
    let (y, m, d) = crate::index::parse_date(date).ok()?;
    let mut hms = time.split(':').map(|p| p.parse::<i64>().ok());
    let (h, mi, sec) = (hms.next()??, hms.next()??, hms.next().unwrap_or(Some(0))?);
    let offset = if eastern_dst(y as i64, m, d, h) { 4 } else { 5 };
    let total = days_from_civil(y as i64, m, d) * 86_400 + (h + offset) * 3600 + mi * 60 + sec;
    let (yy, mm, dd) = civil_from_days(total.div_euclid(86_400));
    let t = total.rem_euclid(86_400);
    Some(format!("{yy}-{mm:02}-{dd:02}T{:02}:{:02}:{:02}.000Z", t / 3600, t % 3600 / 60, t % 60))
}

/// Whether US Eastern daylight saving time is in effect at a local date and hour.
fn eastern_dst(y: i64, m: u32, d: u32, hour: i64) -> bool {
    // Since 2007: second Sunday of March to first Sunday of November.
    // 1987 to 2006: first Sunday of April to last Sunday of October.
    let (start, end) = if y >= 2007 {
        ((3, nth_sunday(y, 3, 2)), (11, nth_sunday(y, 11, 1)))
    } else {
        ((4, nth_sunday(y, 4, 1)), (10, last_sunday(y, 10)))
    };
    let day = days_from_civil(y, m, d);
    let start_day = days_from_civil(y, start.0, start.1);
    let end_day = days_from_civil(y, end.0, end.1);
    (day > start_day || (day == start_day && hour >= 2)) && (day < end_day || (day == end_day && hour < 2))
}

/// 0 is Sunday.
fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

fn nth_sunday(y: i64, m: u32, n: u32) -> u32 {
    let first = weekday(days_from_civil(y, m, 1));
    1 + ((7 - first) % 7) as u32 + 7 * (n - 1)
}

fn last_sunday(y: i64, m: u32) -> u32 {
    let last = 31; // only used for October
    last - weekday(days_from_civil(y, m, last)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from the -index.htm pages of real filings.
    const TEN_K: &str = r##"<html><head><title>EDGAR Filing Documents for 0000320193-24-000123</title></head><body>
<div id="formName"><strong>Form 10-K</strong> - Annual report [Section 13 and 15(d), not S-K Item 405]</div>
<div class="formContent">
  <div class="formGrouping">
    <div class="infoHead">Filing Date</div><div class="info">2024-11-01</div>
    <div class="infoHead">Accepted</div><div class="info">2024-11-01 06:01:36</div>
    <div class="infoHead">Documents</div><div class="info">103</div>
  </div>
  <div class="formGrouping">
    <div class="infoHead">Period of Report</div><div class="info">2024-09-28</div>
  </div>
</div>
<table class="tableFile" summary="Document Format Files">
  <tr><th>Seq</th><th>Description</th><th>Document</th><th>Type</th><th>Size</th></tr>
  <tr><td scope="row">1</td><td scope="row">10-K</td>
    <td scope="row"><a href="/ix?doc=/Archives/edgar/data/320193/000032019324000123/aapl-20240928.htm">aapl-20240928.htm</a> &nbsp;&nbsp;<span style="color: green">iXBRL</span></td>
    <td scope="row">10-K</td><td scope="row">1503780</td></tr>
  <tr class="evenRow"><td scope="row">8</td><td scope="row">EX-21.1</td>
    <td scope="row"><a href="/Archives/edgar/data/320193/000032019324000123/a10-kexhibit21109282024.htm">a10-kexhibit21109282024.htm</a></td>
    <td scope="row">EX-21.1</td><td scope="row">11229</td></tr>
  <tr><td scope="row">&nbsp;</td><td scope="row">Complete submission text file</td>
    <td scope="row"><a href="/Archives/edgar/data/320193/000032019324000123/0000320193-24-000123.txt">0000320193-24-000123.txt</a></td>
    <td scope="row">&nbsp;</td><td scope="row">9759333</td></tr>
</table>
<table class="tableFile" summary="Data Files">
  <tr><th>Seq</th><th>Description</th><th>Document</th><th>Type</th><th>Size</th></tr>
  <tr><td scope="row">14</td><td scope="row">XBRL TAXONOMY EXTENSION SCHEMA DOCUMENT</td>
    <td scope="row"><a href="/Archives/edgar/data/320193/000032019324000123/aapl-20240928.xsd">aapl-20240928.xsd</a></td>
    <td scope="row">EX-101.SCH</td><td scope="row">58200</td></tr>
</table>
<div class="filerDiv">
  <div class="mailer">Mailing Address
    <span class="mailerAddress">ONE APPLE PARK WAY</span>
    <span class="mailerAddress">CUPERTINO <span class="mailerAddress">CA</span> 95014</span>
  </div>
  <div class="mailer">Business Address
    <span class="mailerAddress">ONE APPLE PARK WAY</span>
    <span class="mailerAddress">CUPERTINO <span class="mailerAddress">CA</span> 95014</span>
    <span class="mailerAddress">(408) 996-1010</span>
  </div>
  <div class="companyInfo">
    <span class="companyName">Apple Inc. (Filer)
 <acronym title="Central Index Key">CIK</acronym>: <a href="/cgi-bin/browse-edgar?CIK=0000320193&amp;action=getcompany">0000320193 (see all company filings)</a></span>
    <p class="identInfo"><acronym title="Internal Revenue Service Number">EIN.</acronym>: <strong>942404110</strong> | State of Incorp.: <strong>CA</strong> | Fiscal Year End: <strong>0928</strong><br />Type: <strong>10-K</strong> | Act: <strong>34</strong> | File No.: <a href="#"><strong>001-36743</strong></a> | Film No.: <strong>241416806</strong><br /><acronym title="Standard Industrial Code">SIC</acronym>: <b><a href="#">3571</a></b> Electronic Computers<br />(CF Office: 06 Technology)</p>
  </div>
</div>
</body></html>"##;

    const EIGHT_K: &str = r##"<html><body>
<div id="formName"><strong>Form 8-K</strong> - Current report</div>
<div class="formGrouping">
  <div class="infoHead">Filing Date</div><div class="info">2026-11-06</div>
  <div class="infoHead">Accepted</div><div class="info">2026-11-06 16:06:30</div>
</div>
<div class="formGrouping">
  <div class="infoHead">Items</div>
  <div class="info">Item 2.02: Results of Operations and Financial Condition<br />Item 9.01: Financial Statements and Exhibits<br /></div>
</div>
<table class="tableFile" summary="Document Format Files">
  <tr><td>1</td><td>8-K</td><td><a href="/ix?doc=/Archives/edgar/data/1005101/000149315226050463/form8-k.htm">form8-k.htm</a> <span>iXBRL</span></td><td>8-K</td><td>48700</td></tr>
  <tr><td>2</td><td>PRESS RELEASE</td><td><a href="/Archives/edgar/data/1005101/000149315226050463/ex99-1.htm">ex99-1.htm</a></td><td>EX-99.1</td><td>20100</td></tr>
</table>
<div class="filerDiv">
  <div class="companyInfo">
    <span class="companyName">Marygold Companies, Inc. (Filer)
 <acronym>CIK</acronym>: <a href="#">0001005101 (see all company filings)</a></span>
    <p class="identInfo"><acronym>EIN.</acronym>: <strong>954442384</strong> | State of Incorp.: <strong>NV</strong> | Fiscal Year End: <strong>0630</strong><br /><acronym>SIC</acronym>: <b><a href="#">6199</a></b> Finance Services<br /></p>
  </div>
</div>
<div class="filerDiv">
  <div class="mailer">Business Address
    <span class="mailerAddress">1 MAIN ST</span>
    <span class="mailerAddress">LONDON <span class="mailerAddress">X0</span> EC1</span>
  </div>
  <div class="companyInfo">
    <span class="companyName">Marygold Sub (UK) Ltd (Co-Registrant)
 <acronym>CIK</acronym>: <a href="#">0002000001 (see all company filings)</a></span>
    <p class="identInfo">State of Incorp.: <strong>X0</strong> | Fiscal Year End: <strong>1231</strong></p>
  </div>
</div>
</body></html>"##;

    // A 1997 filing: documents exist only inside the complete submission.
    const OLD: &str = r##"<html><body>
<div id="formName"><strong>Form 10-K</strong> - Annual report</div>
<div class="formGrouping">
  <div class="infoHead">Filing Date</div><div class="info">1997-09-29</div>
  <div class="infoHead">Accepted</div><div class="info">1997-09-29 00:00:00</div>
</div>
<div class="formGrouping"><div class="infoHead">Period of Report</div><div class="info">1997-06-30</div></div>
<table class="tableFile" summary="Document Format Files">
  <tr><td>1</td><td>FORM 10-K FOR FISCAL YEAR ENDED JUNE 30, 1997</td><td><a href="/Archives/edgar/data/789019/"></a></td><td>10-K</td><td>69132</td></tr>
  <tr><td>2</td><td>MICROSOFT CORPORATION 1991 STOCK OPTION PLAN</td><td><a href="/Archives/edgar/data/789019/"></a></td><td>EX-10.1</td><td>28980</td></tr>
  <tr><td>&nbsp;</td><td>Complete submission text file</td><td><a href="/Archives/edgar/data/789019/0001017062-97-001764.txt">0001017062-97-001764.txt</a></td><td>&nbsp;</td><td>198534</td></tr>
</table>
</body></html>"##;

    fn entry(cik: u64, company: &str, form: &str, date: &str, acc: &str) -> IndexEntry {
        IndexEntry {
            cik,
            company: company.into(),
            form: form.into(),
            filing_date: date.into(),
            accession_number: acc.into(),
        }
    }

    #[test]
    fn parses_ten_k_index() {
        let idx = parse_filing_index(TEN_K);
        assert_eq!(idx.form.as_deref(), Some("10-K"));
        assert_eq!(idx.filing_date.as_deref(), Some("2024-11-01"));
        assert_eq!(idx.accepted.as_deref(), Some("2024-11-01 06:01:36"));
        assert_eq!(idx.period_of_report.as_deref(), Some("2024-09-28"));
        assert_eq!(idx.submission_size, Some(9759333));
        assert_eq!(idx.documents.len(), 3);
        let main = idx.primary_document("10-K").unwrap();
        assert_eq!(main.document.as_deref(), Some("aapl-20240928.htm"));
        assert!(main.inline_xbrl);
        assert_eq!(idx.documents[2].doc_type, "EX-101.SCH");

        let filer = idx.filer(320193).unwrap();
        assert_eq!(filer.role.as_deref(), Some("Filer"));
        let p = &filer.profile;
        assert_eq!(p.name, "Apple Inc.");
        assert_eq!(p.sic.as_deref(), Some("3571"));
        assert_eq!(p.sic_description.as_deref(), Some("Electronic Computers"));
        assert_eq!(p.state_of_incorporation.as_deref(), Some("CA"));
        assert_eq!(p.state_location.as_deref(), Some("CA"));
        assert_eq!(p.fiscal_year_end.as_deref(), Some("0928"));
        assert_eq!(p.ein.as_deref(), Some("942404110"));

        let f = idx.to_filing(&entry(320193, "Apple Inc.", "10-K", "2024-11-01", "0000320193-24-000123"));
        assert_eq!(f.report_date.as_deref(), Some("2024-09-28"));
        assert_eq!(f.acceptance_datetime.as_deref(), Some("2024-11-01T10:01:36.000Z"));
        assert_eq!(f.primary_document.as_deref(), Some("aapl-20240928.htm"));
        assert_eq!(f.primary_doc_description.as_deref(), Some("10-K"));
        assert_eq!(
            f.primary_document_url.as_deref(),
            Some("https://www.sec.gov/Archives/edgar/data/320193/000032019324000123/aapl-20240928.htm")
        );
        assert_eq!(
            f.index_url,
            "https://www.sec.gov/Archives/edgar/data/320193/000032019324000123/0000320193-24-000123-index.htm"
        );
        assert_eq!(f.size, Some(9759333));
        assert!(f.is_xbrl && f.is_inline_xbrl);
    }

    #[test]
    fn parses_eight_k_items_and_co_registrants() {
        let idx = parse_filing_index(EIGHT_K);
        assert_eq!(idx.items, ["2.02", "9.01"]);
        assert_eq!(idx.filers.len(), 2);
        let sub = idx.filer(2000001).unwrap();
        assert_eq!(sub.role.as_deref(), Some("Co-Registrant"));
        assert_eq!(sub.profile.name, "Marygold Sub (UK) Ltd");
        assert_eq!(sub.profile.state_location.as_deref(), Some("X0"));
        assert_eq!(sub.profile.sic, None);
        let main = idx.filer(1005101).unwrap();
        assert_eq!(main.profile.sic_description.as_deref(), Some("Finance Services"));
        assert_eq!(main.profile.state_location, None);

        let f = idx.to_filing(&entry(1005101, "MARYGOLD", "8-K", "2026-11-06", "0001493152-26-050463"));
        assert_eq!(f.company, "Marygold Companies, Inc.");
        assert_eq!(f.items, ["2.02", "9.01"]);
        assert_eq!(f.primary_document.as_deref(), Some("form8-k.htm"));
        assert_eq!(f.acceptance_datetime.as_deref(), Some("2026-11-06T21:06:30.000Z"));
        assert!(!f.is_xbrl && f.is_inline_xbrl);
    }

    #[test]
    fn old_filing_falls_back_to_full_submission() {
        let idx = parse_filing_index(OLD);
        let f = idx.to_filing(&entry(789019, "MICROSOFT CORP", "10-K", "1997-09-29", "0001017062-97-001764"));
        assert_eq!(f.primary_document, None);
        assert_eq!(f.primary_document_url, None);
        assert_eq!(f.primary_doc_description.as_deref(), Some("FORM 10-K FOR FISCAL YEAR ENDED JUNE 30, 1997"));
        assert_eq!(f.acceptance_datetime.as_deref(), Some("1997-09-29T04:00:00.000Z"));
        // No filer block: the index row's name is kept.
        assert_eq!(f.company, "MICROSOFT CORP");
        assert_eq!(f.size, Some(198534));
    }

    #[test]
    fn primary_document_prefers_html_over_pdf_and_falls_back_to_seq_1() {
        let doc = |seq, name: &str, ty: &str| IndexDocument {
            seq: Some(seq),
            document: Some(name.into()),
            doc_type: ty.into(),
            ..IndexDocument::default()
        };
        let idx = FilingIndex {
            documents: vec![doc(1, "a.pdf", "10-K"), doc(2, "a.htm", "10-K"), doc(3, "b.htm", "EX-99")],
            ..FilingIndex::default()
        };
        assert_eq!(idx.primary_document("10-k").unwrap().document.as_deref(), Some("a.htm"));
        assert_eq!(idx.primary_document("8-K12B").unwrap().seq, Some(1));
    }

    /// A filing resolved from the full index should match the submissions API's view of it.
    #[test]
    #[ignore = "network"]
    fn index_and_submissions_agree() {
        let ua = std::env::var("PARSEC_EDGAR_USER_AGENT").unwrap_or_else(|_| "parsec-edgar tests dev@pars.ec".into());
        let client = Client::builder(ua).requests_per_second(3.0).build().unwrap();
        let cases = [
            (320193, "AAPL", "10-K", "2024-11-01", "0000320193-24-000123"),
            (1045810, "NVDA", "8-K", "2025-02-26", "0001045810-25-000021"),
            (789019, "MSFT", "10-K", "1997-09-29", "0001017062-97-001764"),
        ];
        for (cik, ticker, form, date, acc) in cases {
            let (from_index, _) = client.resolve_index_entry(&entry(cik, ticker, form, date, acc)).unwrap();
            let from_api = client.filing(ticker, acc).unwrap();
            assert_eq!(from_index.primary_document, from_api.primary_document, "{acc}");
            assert_eq!(from_index.primary_document_url, from_api.primary_document_url, "{acc}");
            assert_eq!(from_index.report_date, from_api.report_date, "{acc}");
            assert_eq!(from_index.acceptance_datetime, from_api.acceptance_datetime, "{acc}");
            assert_eq!(from_index.items, from_api.items, "{acc}");
            assert_eq!(from_index.index_url, from_api.index_url, "{acc}");
        }
    }

    #[test]
    fn eastern_time_to_utc() {
        // Winter (EST, UTC-5) and summer (EDT, UTC-4).
        assert_eq!(eastern_to_utc("2025-01-15 17:30:00").as_deref(), Some("2025-01-15T22:30:00.000Z"));
        assert_eq!(eastern_to_utc("2025-07-01 20:15:05").as_deref(), Some("2025-07-02T00:15:05.000Z"));
        // 2007 rules: DST from 2025-03-09 02:00 to 2025-11-02 02:00.
        assert_eq!(eastern_to_utc("2025-03-08 12:00:00").as_deref(), Some("2025-03-08T17:00:00.000Z"));
        assert_eq!(eastern_to_utc("2025-03-09 12:00:00").as_deref(), Some("2025-03-09T16:00:00.000Z"));
        assert_eq!(eastern_to_utc("2025-11-01 12:00:00").as_deref(), Some("2025-11-01T16:00:00.000Z"));
        assert_eq!(eastern_to_utc("2025-11-02 12:00:00").as_deref(), Some("2025-11-02T17:00:00.000Z"));
        // 1987-2006 rules: DST from 2006-04-02 to 2006-10-29.
        assert_eq!(eastern_to_utc("2006-03-20 12:00:00").as_deref(), Some("2006-03-20T17:00:00.000Z"));
        assert_eq!(eastern_to_utc("2006-04-02 12:00:00").as_deref(), Some("2006-04-02T16:00:00.000Z"));
        assert_eq!(eastern_to_utc("2006-10-30 12:00:00").as_deref(), Some("2006-10-30T17:00:00.000Z"));
        assert_eq!(eastern_to_utc("2024-12-31 22:00:00").as_deref(), Some("2025-01-01T03:00:00.000Z"));
        assert_eq!(eastern_to_utc("garbage"), None);
    }
}
