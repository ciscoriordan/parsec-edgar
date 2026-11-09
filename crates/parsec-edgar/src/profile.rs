//! Company profile: SIC code, state of incorporation, fiscal year end and so on.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

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
    /// Tickers of the company's listed securities, e.g. `["AAPL"]`.
    #[serde(default)]
    pub tickers: Vec<String>,
    /// The exchange of each ticker, in the same order as `tickers`, e.g.
    /// `[Some("Nasdaq")]`; `None` where the SEC gives none. Up to version 0.1.1
    /// this was a `Vec<String>` that left out missing exchanges, so it could
    /// fall out of step with `tickers`.
    #[serde(default)]
    pub exchanges: Vec<Option<String>>,
}

/// The company fields of a submissions API response. Every filing lookup
/// reads them, so a field of an unexpected type is read leniently instead of
/// failing the lookup: a number becomes text, and any other value counts as missing.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubmissionsProfile {
    #[serde(deserialize_with = "text_or_empty")]
    pub(crate) name: String,
    #[serde(deserialize_with = "text_or_none")]
    sic: Option<String>,
    #[serde(deserialize_with = "text_or_none")]
    sic_description: Option<String>,
    #[serde(deserialize_with = "text_or_none")]
    state_of_incorporation: Option<String>,
    #[serde(deserialize_with = "text_or_none")]
    fiscal_year_end: Option<String>,
    #[serde(deserialize_with = "text_or_none")]
    ein: Option<String>,
    #[serde(deserialize_with = "texts")]
    tickers: Vec<Option<String>>,
    #[serde(deserialize_with = "texts")]
    exchanges: Vec<Option<String>>,
    #[serde(deserialize_with = "object_or_none")]
    addresses: Option<Addresses>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Addresses {
    #[serde(deserialize_with = "object_or_none")]
    business: Option<Address>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Address {
    #[serde(deserialize_with = "text_or_none")]
    state_or_country: Option<String>,
}

/// A JSON string as is, a number written out, anything else none.
fn text(value: Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// A text field read leniently, see [`text`].
pub(crate) fn text_or_none<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    Ok(text(Value::deserialize(d)?))
}

/// A text field read leniently, empty when it is not text.
pub(crate) fn text_or_empty<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(text(Value::deserialize(d)?).unwrap_or_default())
}

/// A list of text read leniently, item by item (see [`text`]), so that the
/// positions stay; a value that is not a list is empty.
fn texts<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Vec<Option<String>>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(items) => items.into_iter().map(text).collect(),
        _ => Vec::new(),
    })
}

/// A JSON object read as `T`, or none when the value does not fit.
fn object_or_none<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> std::result::Result<Option<T>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).ok())
}

impl SubmissionsProfile {
    pub(crate) fn into_profile(self, cik: u64) -> CompanyProfile {
        let some = |s: Option<String>| s.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        // The SEC lists each ticker's exchange at the same position. A position
        // without a ticker is left out of both lists.
        let mut exchanges = self.exchanges.into_iter();
        let (tickers, exchanges) = self
            .tickers
            .into_iter()
            .filter_map(|ticker| {
                let exchange = exchanges.next().flatten();
                Some((some(ticker)?, some(exchange)))
            })
            .unzip();
        CompanyProfile {
            cik,
            name: self.name,
            sic: some(self.sic),
            sic_description: some(self.sic_description),
            state_of_incorporation: some(self.state_of_incorporation),
            state_location: some(self.addresses.and_then(|a| a.business).and_then(|b| b.state_or_country)),
            fiscal_year_end: some(self.fiscal_year_end),
            ein: some(self.ein),
            tickers,
            exchanges,
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
        assert_eq!(p.exchanges, [Some("Nasdaq".to_owned())]);
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

    /// Numbers become text, and values of other unexpected types count as missing.
    #[test]
    fn reads_fields_of_unexpected_types() {
        let json = r#"{"name":null,"sic":3571,"ein":942404110,"fiscalYearEnd":true,"stateOfIncorporation":{},
            "tickers":["AAPL",null,{}],"exchanges":"Nasdaq","addresses":{"business":"CA"}}"#;
        let p = serde_json::from_str::<SubmissionsProfile>(json).unwrap().into_profile(42);
        assert_eq!(p.name, "");
        assert_eq!((p.sic.as_deref(), p.ein.as_deref()), (Some("3571"), Some("942404110")));
        assert_eq!((p.fiscal_year_end, p.state_of_incorporation, p.state_location), (None, None, None));
        assert_eq!(p.tickers, ["AAPL"]);
        assert_eq!(p.exchanges, [None]);
        let p = serde_json::from_str::<SubmissionsProfile>(r#"{"addresses":[1]}"#).unwrap().into_profile(42);
        assert_eq!(p.state_location, None);
    }

    /// The SEC gives tickers and exchanges as lists in step, and they stay in
    /// step whatever is missing from either.
    #[test]
    fn tickers_and_exchanges_stay_in_step() {
        let check = |tickers: &str, exchanges: &str, expected: &[(&str, Option<&str>)]| {
            let json = format!(r#"{{"tickers": {tickers}, "exchanges": {exchanges}}}"#);
            let p = serde_json::from_str::<SubmissionsProfile>(&json).unwrap().into_profile(1);
            assert_eq!(p.tickers.len(), p.exchanges.len(), "{json}");
            let got: Vec<(&str, Option<&str>)> =
                p.tickers.iter().map(String::as_str).zip(p.exchanges.iter().map(Option::as_deref)).collect();
            assert_eq!(got, expected, "{json}");
        };
        check(r#"["AAA", "AAAW"]"#, r#"[null, "Nasdaq"]"#, &[("AAA", None), ("AAAW", Some("Nasdaq"))]);
        check(r#"[null, "AAAW"]"#, r#"["NYSE", "Nasdaq"]"#, &[("AAAW", Some("Nasdaq"))]);
        check(r#"[" AAA ", "", 7]"#, r#"[" NYSE ", "Nasdaq", ""]"#, &[("AAA", Some("NYSE")), ("7", None)]);
        check(r#"["AAA", "BBB"]"#, r#"["NYSE"]"#, &[("AAA", Some("NYSE")), ("BBB", None)]);
        check(r#"["AAA"]"#, r#"["NYSE", "Nasdaq"]"#, &[("AAA", Some("NYSE"))]);
        check(r#""AAA""#, r#"["NYSE"]"#, &[]);
        check(r#"["AAA"]"#, r#"{}"#, &[("AAA", None)]);
    }
}
