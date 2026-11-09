//! Company profile: SIC code, state of incorporation, fiscal year end and so on.

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Result;

/// Company facts the SEC keeps with each filer. A filing's `-index.htm` page
/// shows them as they were on the filing date (see [`crate::FilingIndex`]);
/// [`Client::company_profile`] returns the current values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompanyProfile {
    pub cik: u64,
    pub name: String,
    /// Standard Industrial Classification code, e.g. `3571`.
    #[serde(default)]
    pub sic: Option<String>,
    /// e.g. `Electronic Computers`.
    #[serde(default)]
    pub sic_description: Option<String>,
    /// State (or country code) of incorporation, e.g. `DE`.
    #[serde(default)]
    pub state_of_incorporation: Option<String>,
    /// State (or country code) of the business address.
    #[serde(default)]
    pub state_location: Option<String>,
    /// `MMDD`, e.g. `0928`.
    #[serde(default)]
    pub fiscal_year_end: Option<String>,
    /// IRS employer identification number.
    #[serde(default)]
    pub ein: Option<String>,
    #[serde(default)]
    pub tickers: Vec<String>,
    #[serde(default)]
    pub exchanges: Vec<String>,
}

/// The company fields of a submissions API response.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubmissionsProfile {
    pub(crate) name: String,
    sic: Option<String>,
    sic_description: Option<String>,
    state_of_incorporation: Option<String>,
    fiscal_year_end: Option<String>,
    ein: Option<String>,
    tickers: Vec<Option<String>>,
    exchanges: Vec<Option<String>>,
    addresses: Option<Addresses>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Addresses {
    business: Option<Address>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Address {
    state_or_country: Option<String>,
}

impl SubmissionsProfile {
    pub(crate) fn into_profile(self, cik: u64) -> CompanyProfile {
        let some = |s: Option<String>| s.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        CompanyProfile {
            cik,
            name: self.name,
            sic: some(self.sic),
            sic_description: some(self.sic_description),
            state_of_incorporation: some(self.state_of_incorporation),
            state_location: some(self.addresses.and_then(|a| a.business).and_then(|b| b.state_or_country)),
            fiscal_year_end: some(self.fiscal_year_end),
            ein: some(self.ein),
            tickers: self.tickers.into_iter().flatten().collect(),
            exchanges: self.exchanges.into_iter().flatten().collect(),
        }
    }
}

impl Client {
    /// Current profile of a company from the submissions API. One request.
    /// [`Client::crawl_company_with_profile`] returns it along with the filings
    /// from the same request.
    pub fn company_profile(&self, cik: u64) -> Result<CompanyProfile> {
        let raw: SubmissionsProfile = self.get_json(&format!("https://data.sec.gov/submissions/CIK{cik:010}.json"))?;
        Ok(raw.into_profile(cik))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_submissions_profile() {
        let json = r#"{"cik":"0000320193","entityType":"operating","sic":"3571",
            "sicDescription":"Electronic Computers","name":"Apple Inc.","tickers":["AAPL"],
            "exchanges":["Nasdaq"],"ein":"942404110","fiscalYearEnd":"0926",
            "stateOfIncorporation":"CA","addresses":{"mailing":{"stateOrCountry":"CA"},
            "business":{"street1":"ONE APPLE PARK WAY","stateOrCountry":"CA"}},
            "filings":{"recent":{}}}"#;
        let raw: SubmissionsProfile = serde_json::from_str(json).unwrap();
        let p = raw.into_profile(320193);
        assert_eq!(p.name, "Apple Inc.");
        assert_eq!(p.sic.as_deref(), Some("3571"));
        assert_eq!(p.sic_description.as_deref(), Some("Electronic Computers"));
        assert_eq!(p.state_of_incorporation.as_deref(), Some("CA"));
        assert_eq!(p.state_location.as_deref(), Some("CA"));
        assert_eq!(p.fiscal_year_end.as_deref(), Some("0926"));
        assert_eq!(p.tickers, ["AAPL"]);
    }

    #[test]
    fn tolerates_missing_and_empty_fields() {
        let json =
            r#"{"name":"Some Trust","sic":"","stateOfIncorporation":null,"tickers":[],"addresses":{"business":null}}"#;
        let p = serde_json::from_str::<SubmissionsProfile>(json).unwrap().into_profile(42);
        assert_eq!(p.name, "Some Trust");
        assert_eq!(p.sic, None);
        assert_eq!(p.state_of_incorporation, None);
        assert_eq!(p.state_location, None);
    }
}
