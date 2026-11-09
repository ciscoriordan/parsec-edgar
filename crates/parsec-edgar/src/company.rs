use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::{Error, Result};

const TICKERS_URL: &str = "https://www.sec.gov/files/company_tickers_exchange.json";

/// Age at which the ticker list is downloaded again, unless the builder sets another.
pub(crate) const DEFAULT_TICKERS_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Wait after a failed refresh before the next try. It doubles with each failure in a row, up to `MAX_RETRY_WAIT`.
const FIRST_RETRY_WAIT: Duration = Duration::from_secs(60);
const MAX_RETRY_WAIT: Duration = Duration::from_secs(60 * 60);

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

/// A client's ticker list, downloaded again once it is older than `max_age`.
pub(crate) struct Tickers {
    max_age: Duration,
    state: Mutex<TickerState>,
    /// Held during a download, so one caller downloads while the others wait for it or use the old list.
    download: Mutex<()>,
}

#[derive(Default)]
struct TickerState {
    /// The list in use, and when it was downloaded.
    list: Option<(Arc<[Company]>, Instant)>,
    /// Downloads finished so far, failed ones included, so a caller that waited for one can tell it ended.
    attempts: u64,
    /// Why the last download failed, while there is no list to fall back on.
    error: Option<Error>,
    /// Failed refreshes in a row, and the earliest time for the next one.
    failures: u32,
    retry_at: Option<Instant>,
}

impl Tickers {
    pub(crate) fn new(max_age: Duration) -> Self {
        Self { max_age, state: Mutex::default(), download: Mutex::new(()) }
    }

    fn state(&self) -> MutexGuard<'_, TickerState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Client {
    /// All companies with a listed ticker, from the SEC's `company_tickers_exchange.json`.
    /// A company with several share classes appears once per ticker.
    ///
    /// The list is downloaded at first use and again once it is older than the client's
    /// [`tickers_max_age`](crate::ClientBuilder::tickers_max_age) (one day by default). The old
    /// list stays in use until a new one has arrived: one caller downloads it while the others
    /// keep using the old list, and after a failed download the next try waits one minute,
    /// doubling with each failure up to an hour. An error is returned only while there is no list yet.
    pub fn companies(&self) -> Result<Arc<[Company]>> {
        self.companies_at(Instant::now())
    }

    /// [`Client::companies`] as of `now`, so tests can move the clock.
    fn companies_at(&self, now: Instant) -> Result<Arc<[Company]>> {
        let tickers = &self.tickers;
        let (old, seen) = {
            let state = tickers.state();
            if let Some((list, loaded)) = &state.list {
                let fresh = now.saturating_duration_since(*loaded) < tickers.max_age;
                let backing_off = state.retry_at.is_some_and(|at| now < at);
                if fresh || backing_off {
                    return Ok(list.clone());
                }
            }
            (state.list.as_ref().map(|(list, _)| list.clone()), state.attempts)
        };
        // With a list to fall back on, a caller does not wait for a download under way.
        let _download = match (tickers.download.try_lock(), &old) {
            (Ok(guard), _) => guard,
            (Err(TryLockError::WouldBlock), Some(list)) => return Ok(list.clone()),
            (Err(TryLockError::WouldBlock), None) => tickers.download.lock().unwrap_or_else(|e| e.into_inner()),
            (Err(TryLockError::Poisoned(e)), _) => e.into_inner(),
        };
        {
            // A download that finished while this caller waited is its answer too.
            let state = tickers.state();
            if state.attempts != seen {
                if let Some((list, _)) = &state.list {
                    return Ok(list.clone());
                }
                if let Some(e) = &state.error {
                    return Err(copy_error(e));
                }
            }
        }
        let downloaded = self.download_companies();
        let mut state = tickers.state();
        state.attempts += 1;
        match (downloaded, old) {
            (Ok(list), _) => {
                let list: Arc<[Company]> = list.into();
                state.list = Some((list.clone(), now));
                state.error = None;
                state.failures = 0;
                state.retry_at = None;
                Ok(list)
            }
            (Err(_), Some(old)) => {
                state.failures = state.failures.saturating_add(1);
                state.retry_at = Some(now + retry_wait(state.failures));
                Ok(old)
            }
            (Err(e), None) => {
                state.error = Some(copy_error(&e));
                Err(e)
            }
        }
    }

    fn download_companies(&self) -> Result<Vec<Company>> {
        let file: TickerFile = self.get_json(TICKERS_URL)?;
        let col = |name: &str| file.fields.iter().position(|f| f == name);
        let (Some(ci), Some(ni), Some(ti)) = (col("cik"), col("name"), col("ticker")) else {
            return Err(Error::Invalid(format!("unexpected columns in {TICKERS_URL}: {:?}", file.fields)));
        };
        let ei = col("exchange");
        let text = |row: &[serde_json::Value], i: usize| row.get(i).and_then(|v| v.as_str()).map(str::to_owned);
        let list: Vec<Company> = file
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
        // An empty file is a fault at the SEC, and must not replace a list in use.
        if list.is_empty() {
            return Err(Error::Invalid(format!("no companies in {TICKERS_URL}")));
        }
        Ok(list)
    }

    /// Look up a company by ticker (`AAPL`, `BRK.B`) or CIK (`320193`, `0000320193`, `CIK320193`).
    /// Tickers come from [`Client::companies`], so a new or changed ticker is found once the list is refreshed.
    pub fn company(&self, query: &str) -> Result<Company> {
        let q = query.trim();
        if let Some(cik) = parse_cik(q) {
            if let Some(c) = self.companies()?.iter().find(|c| c.cik == cik) {
                return Ok(c.clone());
            }
            // Filers without a listed ticker are missing from the ticker file.
            let profile = self.submissions(cik)?.profile.into_profile(cik);
            return Ok(Company {
                cik,
                name: profile.name,
                ticker: profile.tickers.into_iter().next(),
                exchange: profile.exchanges.into_iter().next(),
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

/// Wait before the next refresh after `failures` failed ones in a row: 1, 2, 4 ... minutes, at most an hour.
fn retry_wait(failures: u32) -> Duration {
    FIRST_RETRY_WAIT.saturating_mul(1 << failures.saturating_sub(1).min(6)).min(MAX_RETRY_WAIT)
}

/// A copy of `e`, so every caller that waited for one failed download gets its error.
fn copy_error(e: &Error) -> Error {
    match e {
        Error::Status { status, url } => Error::Status { status: *status, url: url.clone() },
        Error::Network { url, message } => Error::Network { url: url.clone(), message: message.clone() },
        Error::Json { url, source } => Error::Json { url: url.clone(), source: serde::de::Error::custom(source) },
        Error::Io(io) => Error::Io(std::io::Error::new(io.kind(), io.to_string())),
        Error::CompanyNotFound(s) => Error::CompanyNotFound(s.clone()),
        Error::FilingNotFound(s) => Error::FilingNotFound(s.clone()),
        Error::Invalid(s) => Error::Invalid(s.clone()),
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

    const DAY: Duration = DEFAULT_TICKERS_MAX_AGE;
    const MINUTE: Duration = Duration::from_secs(60);

    /// A ticker file with one row per ticker, each with its own CIK.
    fn ticker_file(tickers: &[&str]) -> String {
        let rows: Vec<_> = tickers
            .iter()
            .enumerate()
            .map(|(i, t)| serde_json::json!([i + 1, format!("{t} Inc."), t, "NYSE"]))
            .collect();
        serde_json::json!({ "fields": ["cik", "name", "ticker", "exchange"], "data": rows }).to_string()
    }

    fn listed(list: &[Company]) -> Vec<&str> {
        list.iter().filter_map(|c| c.ticker.as_deref()).collect()
    }

    /// A client whose ticker file is `tickers`, served by stubs instead of the network.
    fn client(tickers: &[&str]) -> Client {
        let c = Client::builder("Example Corp admin@example.com").build().unwrap();
        c.stub(TICKERS_URL, ticker_file(tickers));
        c
    }

    #[test]
    fn list_is_downloaded_again_after_max_age() {
        let c = client(&["AAA"]);
        let t0 = Instant::now();
        assert_eq!(listed(&c.companies_at(t0).unwrap()), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);

        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));
        assert_eq!(listed(&c.companies_at(t0 + DAY - MINUTE).unwrap()), ["AAA"]);
        assert!(c.take_fetched().is_empty());

        assert_eq!(listed(&c.companies_at(t0 + DAY).unwrap()), ["AAA", "NEW"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        // The new list is good for another day.
        assert_eq!(listed(&c.companies_at(t0 + DAY + DAY - MINUTE).unwrap()), ["AAA", "NEW"]);
        assert!(c.take_fetched().is_empty());
    }

    #[test]
    fn failed_refresh_keeps_the_old_list_and_backs_off() {
        let c = client(&["AAA"]);
        let t0 = Instant::now();
        c.companies_at(t0).unwrap();
        c.take_fetched();

        // A refresh that fails returns the old list, with no error.
        c.stub(TICKERS_URL, "<html>Service Unavailable</html>");
        let t1 = t0 + DAY;
        assert_eq!(listed(&c.companies_at(t1).unwrap()), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        // No new try for a minute.
        assert_eq!(listed(&c.companies_at(t1 + MINUTE / 2).unwrap()), ["AAA"]);
        assert!(c.take_fetched().is_empty());
        // The second failure waits two minutes.
        let t2 = t1 + MINUTE;
        assert_eq!(listed(&c.companies_at(t2).unwrap()), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));
        c.companies_at(t2 + MINUTE).unwrap();
        assert!(c.take_fetched().is_empty());

        let t3 = t2 + 2 * MINUTE;
        assert_eq!(listed(&c.companies_at(t3).unwrap()), ["AAA", "NEW"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        // A success starts a new day and clears the failures. An empty file is a failure too.
        c.companies_at(t3 + DAY - MINUTE).unwrap();
        assert!(c.take_fetched().is_empty());
        c.stub(TICKERS_URL, ticker_file(&[]));
        assert_eq!(listed(&c.companies_at(t3 + DAY).unwrap()), ["AAA", "NEW"]);
        assert_eq!(listed(&c.companies_at(t3 + DAY + MINUTE).unwrap()), ["AAA", "NEW"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL, TICKERS_URL]);
    }

    #[test]
    fn retry_wait_doubles_up_to_an_hour() {
        let waits: Vec<u64> = (1..=9).map(|n| retry_wait(n).as_secs() / 60).collect();
        assert_eq!(waits, [1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(retry_wait(u32::MAX), MAX_RETRY_WAIT);
    }

    /// While one caller refreshes the list, the others use the old one without waiting.
    #[test]
    fn lookups_during_a_refresh_use_the_old_list() {
        let c = client(&["AAA"]);
        let t0 = Instant::now();
        c.companies_at(t0).unwrap();
        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));
        c.take_fetched();

        let refreshing = c.tickers.download.lock().unwrap();
        assert_eq!(listed(&c.companies_at(t0 + DAY).unwrap()), ["AAA"]);
        assert!(c.take_fetched().is_empty());
        drop(refreshing);
        assert_eq!(listed(&c.companies_at(t0 + DAY).unwrap()), ["AAA", "NEW"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
    }

    #[test]
    fn first_download_failure_is_an_error_and_not_kept() {
        let c = client(&[]);
        assert!(matches!(c.companies(), Err(Error::Invalid(_))));
        c.stub(TICKERS_URL, "not json");
        assert!(matches!(c.companies(), Err(Error::Json { .. })));
        assert!(matches!(c.company("AAA"), Err(Error::Json { .. })));
        assert_eq!(c.take_fetched(), [TICKERS_URL, TICKERS_URL, TICKERS_URL]);
        c.stub(TICKERS_URL, ticker_file(&["AAA"]));
        assert_eq!(listed(&c.companies().unwrap()), ["AAA"]);
    }

    /// Callers that need the list at the same time share one download.
    #[test]
    fn concurrent_first_lookups_download_once() {
        for round in 0..20 {
            let c = client(&["AAA", "BBB"]);
            std::thread::scope(|s| {
                let handles: Vec<_> = (0..8).map(|_| s.spawn(|| c.company("BBB"))).collect();
                for h in handles {
                    assert_eq!(h.join().unwrap().unwrap().cik, 2, "round {round}");
                }
            });
            assert_eq!(c.take_fetched(), [TICKERS_URL], "round {round}");
        }
    }

    #[test]
    fn copied_errors_read_the_same() {
        let json = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let errors = [
            Error::Status { status: 503, url: TICKERS_URL.into() },
            Error::Network { url: TICKERS_URL.into(), message: "connection reset".into() },
            Error::Json { url: TICKERS_URL.into(), source: json },
            Error::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out")),
            Error::Invalid("unexpected columns".into()),
        ];
        for e in &errors {
            assert_eq!(copy_error(e).to_string(), e.to_string());
        }
    }

    /// The builder's age applies, and a ticker added to the SEC file is found after a refresh.
    #[test]
    fn new_ticker_is_found_after_refresh() {
        let c = Client::builder("Example Corp admin@example.com").tickers_max_age(Duration::ZERO).build().unwrap();
        c.stub(TICKERS_URL, ticker_file(&["AAA"]));
        assert!(matches!(c.company("NEW"), Err(Error::CompanyNotFound(_))));
        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));
        assert_eq!(c.company("new").unwrap().cik, 2);
        assert_eq!(c.take_fetched(), [TICKERS_URL, TICKERS_URL]);
    }
}
