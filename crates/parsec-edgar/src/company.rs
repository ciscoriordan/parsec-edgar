use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::{Error, Result};

const TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers_exchange.json";

/// A company (or other filer) known to EDGAR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Company {
    pub cik: u64,
    pub name: String,
    #[serde(default)]
    pub ticker: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
}

#[derive(Deserialize)]
struct TickerFile {
    fields: Vec<String>,
    data: Vec<Vec<serde_json::Value>>,
}

impl Client {
    /// All companies with a listed ticker, from the SEC's `company_tickers_exchange.json`.
    /// Downloaded once per client. A company with several share classes appears once per ticker.
    pub fn companies(&self) -> Result<&[Company]> {
        if let Some(list) = self.tickers.get() {
            return Ok(list);
        }
        let file: TickerFile = self.get_json(TICKERS_URL)?;
        let col = |name: &str| file.fields.iter().position(|f| f == name);
        let (Some(ci), Some(ni), Some(ti)) = (col("cik"), col("name"), col("ticker")) else {
            return Err(Error::Invalid(format!("unexpected columns in {TICKERS_URL}: {:?}", file.fields)));
        };
        let ei = col("exchange");
        let text = |row: &[serde_json::Value], i: usize| row.get(i).and_then(|v| v.as_str()).map(str::to_owned);
        let list = file
            .data
            .iter()
            .filter_map(|row| {
                Some(Company {
                    cik: row.get(ci)?.as_u64()?,
                    name: text(row, ni).unwrap_or_default(),
                    ticker: text(row, ti),
                    exchange: ei.and_then(|i| text(row, i)),
                })
            })
            .collect();
        let _ = self.tickers.set(list);
        Ok(self.tickers.get().expect("set above"))
    }

    /// Look up a company by ticker (`AAPL`, `BRK.B`) or CIK (`320193`, `0000320193`, `CIK320193`).
    pub fn company(&self, query: &str) -> Result<Company> {
        let q = query.trim();
        if let Some(cik) = parse_cik(q) {
            if let Some(c) = self.companies()?.iter().find(|c| c.cik == cik) {
                return Ok(c.clone());
            }
            // Filers without a listed ticker are missing from the ticker file.
            let sub = self.submissions(cik)?;
            return Ok(Company {
                cik,
                name: sub.name,
                ticker: sub.tickers.into_iter().flatten().next(),
                exchange: sub.exchanges.into_iter().flatten().next(),
            });
        }
        let ticker = normalize_ticker(q);
        self.companies()?
            .iter()
            .find(|c| c.ticker.as_deref() == Some(ticker.as_str()))
            .cloned()
            .ok_or_else(|| Error::CompanyNotFound(q.to_owned()))
    }
}

/// Parse `320193`, `0000320193` or `CIK0000320193` as a CIK.
pub(crate) fn parse_cik(q: &str) -> Option<u64> {
    let digits = q.strip_prefix("CIK").or_else(|| q.strip_prefix("cik")).unwrap_or(q);
    if digits.is_empty() || digits.len() > 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The SEC writes share classes with a dash: `BRK.B` and `BRK/B` become `BRK-B`.
fn normalize_ticker(q: &str) -> String {
    q.to_ascii_uppercase().replace(['.', '/'], "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cik_forms() {
        assert_eq!(parse_cik("320193"), Some(320193));
        assert_eq!(parse_cik("0000320193"), Some(320193));
        assert_eq!(parse_cik("CIK0000320193"), Some(320193));
        assert_eq!(parse_cik("AAPL"), None);
        assert_eq!(parse_cik("12345678901"), None);
    }

    #[test]
    fn tickers() {
        assert_eq!(normalize_ticker("brk.b"), "BRK-B");
        assert_eq!(normalize_ticker("BRK/B"), "BRK-B");
    }
}
