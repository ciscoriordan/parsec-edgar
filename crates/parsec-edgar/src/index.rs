//! EDGAR full-index files: every filing of every company, one file per quarter.
//!
//! `https://www.sec.gov/Archives/edgar/full-index/YYYY/QTRn/` holds the same
//! list sorted three ways (`company`, `form`, `master`). `master` is the one
//! read here: it is pipe-delimited (the others are fixed-width columns that long
//! company names can break) and its gzip copy is the smallest download (about
//! 3 MB per quarter against 24 MB for `master.idx` and 42 MB for `form.idx`).

use std::fmt;
use std::io::Read;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::encoding;
use crate::error::{Error, Result};

/// The first quarter with an EDGAR full index.
const FIRST_YEAR: u16 = 1993;

/// A calendar quarter, e.g. `2024Q3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Quarter {
    pub year: u16,
    /// 1 to 4.
    pub quarter: u8,
}

impl Quarter {
    pub fn new(year: u16, quarter: u8) -> Result<Self> {
        if !(1..=4).contains(&quarter) {
            return Err(Error::Invalid(format!("quarters are 1 to 4, got {quarter}")));
        }
        if !(FIRST_YEAR..=9999).contains(&year) {
            return Err(Error::Invalid(format!("the EDGAR full index starts in {FIRST_YEAR}, got {year}")));
        }
        Ok(Self { year, quarter })
    }

    /// The quarter holding a date (`YYYY-MM-DD` or `YYYY-MM`; a bare `YYYY` means its first quarter).
    pub fn of_date(date: &str) -> Result<Self> {
        let (y, m, _) = parse_date(date)?;
        Self::new(y, ((m - 1) / 3 + 1) as u8)
    }

    /// The quarter holding today's date (UTC).
    pub fn current() -> Self {
        let (y, m, _) = civil_from_days(today_days());
        Self { year: y as u16, quarter: ((m - 1) / 3 + 1) as u8 }
    }

    /// Every quarter from `from` to `to`, inclusive.
    pub fn range(from: Quarter, to: Quarter) -> Vec<Quarter> {
        let mut out = Vec::new();
        let mut q = from;
        while q <= to {
            out.push(q);
            q = q.next();
        }
        out
    }

    pub fn next(self) -> Self {
        if self.quarter == 4 {
            Self { year: self.year + 1, quarter: 1 }
        } else {
            Self { year: self.year, quarter: self.quarter + 1 }
        }
    }

    /// First day, `YYYY-MM-DD`.
    pub fn first_day(&self) -> String {
        format!("{}-{:02}-01", self.year, (self.quarter - 1) * 3 + 1)
    }

    /// Last day, `YYYY-MM-DD`.
    pub fn last_day(&self) -> String {
        let day = if matches!(self.quarter, 1 | 4) { 31 } else { 30 };
        format!("{}-{:02}-{day}", self.year, self.quarter * 3)
    }

    /// URL of the gzipped master index.
    pub fn master_index_url(&self) -> String {
        format!("https://www.sec.gov/Archives/edgar/full-index/{}/QTR{}/master.gz", self.year, self.quarter)
    }

    /// The SEC rewrites a quarter's index every night until the quarter is over.
    /// Allow a few days after it ends before treating the file as final.
    fn is_final(&self, today_days: i64) -> bool {
        let (y, m, d) = parse_date(&self.last_day()).expect("valid date");
        today_days > days_from_civil(y as i64, m, d) + 3
    }
}

impl fmt::Display for Quarter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}Q{}", self.year, self.quarter)
    }
}

impl FromStr for Quarter {
    type Err = Error;

    /// `2024Q3`, `2024-Q3` or `2024q3`.
    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        let bad = || Error::Invalid(format!("expected a quarter like 2024Q3, got {s:?}"));
        let (y, q) = s.split_once(['Q', 'q']).ok_or_else(bad)?;
        let y = y.strip_suffix('-').unwrap_or(y);
        Self::new(y.parse().map_err(|_| bad())?, q.parse().map_err(|_| bad())?)
    }
}

/// One row of a master index: one filing by one filer. A filing with several
/// filers (co-registrants, or a subject company and a filer) has one row each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub cik: u64,
    pub company: String,
    pub form: String,
    /// `YYYY-MM-DD`.
    pub filing_date: String,
    /// Dashed form, e.g. `0000320193-24-000123`.
    pub accession_number: String,
}

impl IndexEntry {
    /// Folder holding every document of the filing.
    pub fn folder_url(&self) -> String {
        format!("https://www.sec.gov/Archives/edgar/data/{}/{}", self.cik, self.accession_number.replace('-', ""))
    }

    /// The filing's `-index.htm` page, which lists its documents and filers.
    pub fn index_url(&self) -> String {
        format!("{}/{}-index.htm", self.folder_url(), self.accession_number)
    }

    /// The complete submission text file, the only link the index itself gives.
    pub fn full_submission_url(&self) -> String {
        format!("{}/{}.txt", self.folder_url(), self.accession_number)
    }
}

/// Parse a master index (`master.idx`, quarterly or daily): `CIK|Company Name|Form Type|Date Filed|Filename`.
/// Header lines and malformed rows are skipped. Daily files write dates as
/// `YYYYMMDD`; they are returned as `YYYY-MM-DD`.
pub fn parse_master_index(text: &str) -> Vec<IndexEntry> {
    let body = match text.find("\n---") {
        Some(i) => text[i + 1..].split_once('\n').map_or("", |(_, rest)| rest),
        None => text,
    };
    body.lines().filter_map(parse_master_line).collect()
}

fn parse_master_line(line: &str) -> Option<IndexEntry> {
    let line = line.trim_end_matches('\r');
    let (cik, rest) = line.split_once('|')?;
    // The company name may itself hold a `|`, so take the last three fields from the right.
    let mut right = rest.rsplitn(4, '|');
    let path = right.next()?.trim();
    let date = right.next()?.trim();
    let form = right.next()?.trim();
    let company = right.next()?.trim();
    let cik: u64 = cik.trim().parse().ok()?;
    let stem = path.rsplit('/').next()?.strip_suffix(".txt")?;
    let accession_number = dashed_accession(stem)?;
    let filing_date = match date.len() {
        10 => date.to_owned(),
        8 if date.bytes().all(|b| b.is_ascii_digit()) => format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]),
        _ => return None,
    };
    parse_date(&filing_date).ok()?;
    Some(IndexEntry { cik, company: company.to_owned(), form: form.to_owned(), filing_date, accession_number })
}

/// `0000320193-24-000123` or `000032019324000123` to the dashed form.
pub(crate) fn dashed_accession(s: &str) -> Option<String> {
    let digits: String = s.chars().filter(|c| *c != '-').collect();
    (digits.len() == 18 && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| format!("{}-{}-{}", &digits[..10], &digits[10..12], &digits[12..]))
}

impl Client {
    /// Every filing in one quarter's master index, in index order (by CIK).
    ///
    /// One request. Finished quarters are cached; the current quarter is fetched
    /// fresh, since the SEC adds the previous business day's filings every night.
    pub fn full_index(&self, quarter: Quarter) -> Result<Vec<IndexEntry>> {
        let url = quarter.master_index_url();
        let bytes =
            if quarter.is_final(today_days()) { self.get_bytes(&url)? } else { self.get_bytes_fresh(&url)? };
        let raw = gunzip_if_needed(&bytes).map_err(|e| Error::Network { url, message: e.to_string() })?;
        Ok(parse_master_index(&encoding::decode(&raw)))
    }
}

/// `.gz` files are served without a `Content-Encoding` header, so decompress them here.
fn gunzip_if_needed(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(bytes.to_vec());
    }
    let mut out = Vec::with_capacity(bytes.len() * 8);
    flate2::read::MultiGzDecoder::new(bytes).read_to_end(&mut out)?;
    Ok(out)
}

/// `YYYY`, `YYYY-MM` or `YYYY-MM-DD` to (year, month, day); missing parts are 1.
pub(crate) fn parse_date(s: &str) -> Result<(u16, u32, u32)> {
    let bad = || Error::Invalid(format!("dates must be YYYY, YYYY-MM or YYYY-MM-DD, got {s:?}"));
    let mut parts = s.trim().split('-');
    let y: u16 = parts.next().filter(|p| p.len() == 4).and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    let mut num = |max: u32| -> Result<u32> {
        match parts.next() {
            None => Ok(1),
            Some(p) if p.len() == 2 => p.parse().ok().filter(|v| (1..=max).contains(v)).ok_or_else(bad),
            Some(_) => Err(bad()),
        }
    };
    let m = num(12)?;
    let d = num(31)?;
    if parts.next().is_some() {
        return Err(bad());
    }
    Ok((y, m, d))
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

pub(crate) fn today_days() -> i64 {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    (secs / 86_400) as i64
}

/// Today's date (UTC), `YYYY-MM-DD`.
pub(crate) fn today() -> String {
    let (y, m, d) = civil_from_days(today_days());
    format!("{y}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const MASTER: &str = "Description:           Master Index of EDGAR Dissemination Feed\n\
Last Data Received:    December 31, 2025\n\
Comments:              webmaster@sec.gov\n\
Anonymous FTP:         ftp://ftp.sec.gov/edgar/\n\
Cloud HTTP:            https://www.sec.gov/Archives/\n\
 \n \n \n\
CIK|Company Name|Form Type|Date Filed|Filename\n\
--------------------------------------------------------------------------------\n\
1000045|NICHOLAS FINANCIAL INC|10-Q|2025-11-07|edgar/data/1000045/0000950170-25-143092.txt\n\
1022899|Protagenic Therapeutics, Inc.\\new|8-K|2025-10-02|edgar/data/1022899/0001493152-25-050420.txt\r\n\
320193|A | B HOLDINGS|10-K/A|2025-12-31|edgar/data/320193/0000320193-25-000001.txt\n\
bad line\n\
42|NO DATE CO|10-K|soon|edgar/data/42/0000000042-25-000001.txt\n";

    #[test]
    fn parses_master_index() {
        let rows = parse_master_index(MASTER);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0],
            IndexEntry {
                cik: 1000045,
                company: "NICHOLAS FINANCIAL INC".into(),
                form: "10-Q".into(),
                filing_date: "2025-11-07".into(),
                accession_number: "0000950170-25-143092".into(),
            }
        );
        assert_eq!(rows[1].company, "Protagenic Therapeutics, Inc.\\new");
        assert_eq!(rows[1].accession_number, "0001493152-25-050420");
        assert_eq!(rows[2].company, "A | B HOLDINGS");
        assert_eq!(rows[2].form, "10-K/A");
        assert_eq!(
            rows[0].index_url(),
            "https://www.sec.gov/Archives/edgar/data/1000045/000095017025143092/0000950170-25-143092-index.htm"
        );
        assert_eq!(
            rows[0].full_submission_url(),
            "https://www.sec.gov/Archives/edgar/data/1000045/000095017025143092/0000950170-25-143092.txt"
        );
    }

    #[test]
    fn parses_daily_index_dates() {
        let daily = "CIK|Company Name|Form Type|Date Filed|File Name\n\
--------------------------------------------------------------------------------\n\
1005101|Marygold Companies, Inc.|8-K|20261106|edgar/data/1005101/0001493152-26-050463.txt\n";
        let rows = parse_master_index(daily);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].filing_date, "2026-11-06");
    }

    #[test]
    fn decompresses_gzip() {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(MASTER.as_bytes()).unwrap();
        let gz = enc.finish().unwrap();
        assert_eq!(gunzip_if_needed(&gz).unwrap(), MASTER.as_bytes());
        assert_eq!(gunzip_if_needed(b"plain").unwrap(), b"plain");
    }

    #[test]
    fn quarters() {
        let q = Quarter::new(2024, 3).unwrap();
        assert_eq!(q.to_string(), "2024Q3");
        assert_eq!(q.first_day(), "2024-07-01");
        assert_eq!(q.last_day(), "2024-09-30");
        assert_eq!(Quarter::new(2024, 4).unwrap().last_day(), "2024-12-31");
        assert_eq!(Quarter::new(2024, 1).unwrap().last_day(), "2024-03-31");
        assert_eq!(q.master_index_url(), "https://www.sec.gov/Archives/edgar/full-index/2024/QTR3/master.gz");
        assert_eq!(Quarter::of_date("2024-11-06").unwrap(), Quarter::new(2024, 4).unwrap());
        assert_eq!(Quarter::of_date("2024").unwrap(), Quarter::new(2024, 1).unwrap());
        assert_eq!("2023q4".parse::<Quarter>().unwrap(), Quarter::new(2023, 4).unwrap());
        assert_eq!("2023-Q4".parse::<Quarter>().unwrap(), Quarter::new(2023, 4).unwrap());
        assert!("2023".parse::<Quarter>().is_err());
        assert!(Quarter::new(2024, 5).is_err());
        assert!(Quarter::new(1992, 4).is_err());
        let r = Quarter::range(Quarter::new(2023, 3).unwrap(), Quarter::new(2024, 2).unwrap());
        assert_eq!(r.iter().map(|q| q.to_string()).collect::<Vec<_>>(), ["2023Q3", "2023Q4", "2024Q1", "2024Q2"]);
    }

    #[test]
    fn quarter_finality() {
        let q = Quarter::new(2026, 3).unwrap();
        assert!(!q.is_final(days_from_civil(2026, 9, 30)));
        assert!(!q.is_final(days_from_civil(2026, 10, 3)));
        assert!(q.is_final(days_from_civil(2026, 10, 4)));
    }

    #[test]
    fn civil_dates_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        for z in [-1000, 0, 11_016, 11_017, 20_000, 20_765] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
    }

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2024-02-29").unwrap(), (2024, 2, 29));
        assert_eq!(parse_date("2024-02").unwrap(), (2024, 2, 1));
        assert!(parse_date("2024/02/29").is_err());
        assert!(parse_date("2024-13").is_err());
        assert!(parse_date("24").is_err());
    }
}
