use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

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

/// A client's ticker list. Once it is `max_age` old, a lookup starts a new
/// download on another thread and keeps using the old list until it has arrived.
pub(crate) struct Tickers {
    max_age: Duration,
    /// How long a caller with no list waits for a download under way before it downloads the list itself.
    wait_limit: Duration,
    state: Mutex<TickerState>,
    /// Notified when a download ends.
    ended: Condvar,
    #[cfg(test)]
    clock: Mutex<TestClock>,
}

#[derive(Default)]
struct TickerState {
    /// The list in use, and when its download ended.
    list: Option<(Arc<[Company]>, Moment)>,
    /// The download under way, if any.
    download: Option<Claim>,
    /// Downloads started so far, to number them.
    started: u64,
    /// Downloads finished so far, failed ones included, so a caller that waited for one can tell it ended.
    attempts: u64,
    /// Why the last download failed, while there is no list to fall back on.
    error: Option<Error>,
    /// Failed refreshes in a row, and the earliest time for the next one.
    failures: u32,
    retry_at: Option<Instant>,
}

/// A time read from both clocks. `Instant` never goes back, but on macOS and
/// Linux it stops while the machine sleeps. The wall clock counts sleep, but it
/// can be set back.
#[derive(Clone, Copy)]
struct Moment {
    instant: Instant,
    wall: SystemTime,
}

impl Moment {
    /// Time from `self` to `now`, the longer of the two clocks' readings. When
    /// the wall clock reads earlier than at `self`, it has been set back and
    /// the age is unknown: `Duration::MAX`, so the list is downloaded again.
    fn age(&self, now: Moment) -> Duration {
        let wall = now.wall.duration_since(self.wall).unwrap_or(Duration::MAX);
        now.instant.saturating_duration_since(self.instant).max(wall)
    }
}

/// A download under way: the process that started it, its number, and when it started.
#[derive(Clone, Copy)]
struct Claim {
    pid: u32,
    id: u64,
    started: Instant,
}

impl Tickers {
    pub(crate) fn new(max_age: Duration, wait_limit: Duration) -> Self {
        Self {
            max_age,
            wait_limit,
            state: Mutex::default(),
            ended: Condvar::new(),
            #[cfg(test)]
            clock: Mutex::default(),
        }
    }

    fn state(&self) -> MutexGuard<'_, TickerState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn now(&self) -> Moment {
        let now = Moment { instant: Instant::now(), wall: SystemTime::now() };
        #[cfg(test)]
        let now = self.clock.lock().unwrap().apply(now);
        now
    }

    /// Whether the download `claim` can still end: it runs in this process and
    /// started less than `wait_limit` ago. A child process forked during a
    /// download inherits the claim, but not the thread doing the download.
    fn live(&self, claim: &Claim, now: Instant) -> bool {
        claim.pid == std::process::id() && now.saturating_duration_since(claim.started) < self.wait_limit
    }

    /// Record a new download as the one under way, in place of any earlier one.
    fn claim(&self, state: &mut TickerState, now: Instant) -> Claim {
        state.started += 1;
        let claim = Claim { pid: std::process::id(), id: state.started, started: now };
        state.download = Some(claim);
        claim
    }

    /// Record how the download `claim` ended, and wake the callers waiting for it.
    /// The list's age and the wait before the next try count from now, when the download ended.
    fn finish(&self, claim: Claim, downloaded: Result<Vec<Company>>) -> Result<Arc<[Company]>> {
        let now = self.now();
        let mut state = self.state();
        state.attempts += 1;
        if state.download.is_some_and(|c| c.id == claim.id) {
            state.download = None;
        }
        let old = state.list.as_ref().map(|(list, _)| list.clone());
        let result = match (downloaded, old) {
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
                state.retry_at = Some(now.instant + retry_wait(state.failures));
                Ok(old)
            }
            (Err(e), None) => {
                state.error = Some(copy_error(&e));
                Err(e)
            }
        };
        drop(state);
        self.ended.notify_all();
        result
    }
}

impl Client {
    /// All companies with a listed ticker, from the SEC's `company_tickers_exchange.json`.
    /// A company with several share classes appears once per ticker.
    ///
    /// The list is downloaded at first use. Once it is older than the client's
    /// [`tickers_max_age`](crate::ClientBuilder::tickers_max_age) (one day by default,
    /// counting time the machine is asleep), the next call starts a download on another
    /// thread and returns the old list, as do all calls until the new list has arrived.
    /// After a failed download the next try waits one minute from the failure, doubling
    /// with each failure in a row up to an hour. An error is returned only while there
    /// is no list yet.
    ///
    /// The list is shared: iterate it with `.iter()`, and pass `&list` where a
    /// `&[Company]` is wanted. A list returned earlier stays valid after a new one
    /// replaces it. Up to version 0.1.1 this method returned `&[Company]`.
    pub fn companies(&self) -> Result<Arc<[Company]>> {
        let tickers = &self.tickers;
        let mut state = tickers.state();
        let seen = state.attempts;
        let claim = loop {
            let now = tickers.now();
            if let Some((list, loaded)) = &state.list {
                let list = list.clone();
                let stale = loaded.age(now) >= tickers.max_age;
                let backing_off = state.retry_at.is_some_and(|at| now.instant < at);
                let downloading = state.download.is_some_and(|c| tickers.live(&c, now.instant));
                if stale && !backing_off && !downloading {
                    let claim = tickers.claim(&mut state, now.instant);
                    drop(state);
                    self.refresh_in_background(claim);
                }
                return Ok(list);
            }
            // A first download that failed while this caller waited is its answer too.
            if state.attempts != seen
                && let Some(e) = &state.error
            {
                return Err(copy_error(e));
            }
            match state.download {
                Some(c) if tickers.live(&c, now.instant) => {
                    let left = tickers.wait_limit.saturating_sub(now.instant.saturating_duration_since(c.started));
                    state = tickers.ended.wait_timeout(state, left).unwrap_or_else(|e| e.into_inner()).0;
                }
                // No download under way, or one that will not end: download the list here.
                _ => break tickers.claim(&mut state, now.instant),
            }
        };
        drop(state);
        tickers.finish(claim, self.download_companies())
    }

    /// Download the list again on another thread, for [`Client::companies`].
    fn refresh_in_background(&self, claim: Claim) {
        let client = self.share();
        let spawned = std::thread::Builder::new().name("parsec-edgar-tickers".into()).spawn(move || {
            let downloaded = client.download_companies();
            let _ = client.tickers.finish(claim, downloaded);
        });
        // Without a thread the refresh fails, and the next try waits as after any failure.
        if let Err(e) = spawned {
            let _ = self.tickers.finish(claim, Err(e.into()));
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
            // Take the first ticker of the submissions API, with its own exchange.
            let profile = self.submissions(cik)?.profile.into_profile(cik);
            return Ok(Company {
                cik,
                name: profile.name,
                ticker: profile.tickers.into_iter().next(),
                exchange: profile.exchanges.into_iter().next().flatten(),
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

/// Offsets tests add to the clocks. Time awake moves both clocks, time asleep only the wall clock.
#[cfg(test)]
#[derive(Default)]
struct TestClock {
    awake: Duration,
    asleep: Duration,
    set_back: Duration,
}

#[cfg(test)]
impl TestClock {
    fn apply(&self, now: Moment) -> Moment {
        Moment { instant: now.instant + self.awake, wall: now.wall + self.awake + self.asleep - self.set_back }
    }
}

#[cfg(test)]
impl Tickers {
    /// Move both clocks forward, as when time passes with the machine awake.
    fn advance(&self, by: Duration) {
        self.clock.lock().unwrap().awake += by;
    }

    /// Move the wall clock forward only, as when time passes with the machine asleep.
    fn sleep(&self, by: Duration) {
        self.clock.lock().unwrap().asleep += by;
    }

    fn set_back(&self, by: Duration) {
        self.clock.lock().unwrap().set_back += by;
    }

    /// Wait until no download is under way.
    fn wait_idle(&self) {
        let mut state = self.state();
        while state.download.is_some() {
            state = self.ended.wait(state).unwrap();
        }
    }
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
    const HOUR: Duration = Duration::from_secs(60 * 60);
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

    /// The list, after waiting for the refresh that the call may have started.
    fn lookup(c: &Client) -> Arc<[Company]> {
        let list = c.companies().unwrap();
        c.tickers.wait_idle();
        list
    }

    /// `companies()` run on another thread, or None if it has not returned within `limit`.
    fn companies_within(c: &Client, limit: Duration) -> Option<Result<Arc<[Company]>>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let other = c.share();
        std::thread::spawn(move || tx.send(other.companies()));
        rx.recv_timeout(limit).ok()
    }

    #[test]
    fn list_is_downloaded_again_after_max_age() {
        let c = client(&["AAA"]);
        assert_eq!(listed(&c.companies().unwrap()), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);

        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));
        c.tickers.advance(DAY - MINUTE);
        assert_eq!(listed(&lookup(&c)), ["AAA"]);
        assert!(c.take_fetched().is_empty());

        // The call that finds the list a day old gets the old one and starts a download.
        c.tickers.advance(MINUTE);
        assert_eq!(listed(&lookup(&c)), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        assert_eq!(listed(&lookup(&c)), ["AAA", "NEW"]);
        // The new list is good for another day.
        c.tickers.advance(DAY - MINUTE);
        assert_eq!(listed(&lookup(&c)), ["AAA", "NEW"]);
        assert!(c.take_fetched().is_empty());
    }

    #[test]
    fn failed_refresh_keeps_the_old_list_and_backs_off() {
        let c = client(&["AAA"]);
        lookup(&c);
        c.take_fetched();

        // A refresh that fails leaves the old list in use, with no error.
        c.stub(TICKERS_URL, "<html>Service Unavailable</html>");
        c.tickers.advance(DAY);
        assert_eq!(listed(&lookup(&c)), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        // No new try for a minute.
        c.tickers.advance(MINUTE / 2);
        assert_eq!(listed(&lookup(&c)), ["AAA"]);
        assert!(c.take_fetched().is_empty());
        // The second failure waits two minutes.
        c.tickers.advance(MINUTE / 2);
        assert_eq!(listed(&lookup(&c)), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));
        c.tickers.advance(MINUTE);
        lookup(&c);
        assert!(c.take_fetched().is_empty());

        c.tickers.advance(MINUTE);
        assert_eq!(listed(&lookup(&c)), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        assert_eq!(listed(&lookup(&c)), ["AAA", "NEW"]);
        // A success starts a new day and clears the failures. An empty file is a failure too.
        c.tickers.advance(DAY - MINUTE);
        lookup(&c);
        assert!(c.take_fetched().is_empty());
        c.stub(TICKERS_URL, ticker_file(&[]));
        c.tickers.advance(MINUTE);
        assert_eq!(listed(&lookup(&c)), ["AAA", "NEW"]);
        c.tickers.advance(MINUTE);
        assert_eq!(listed(&lookup(&c)), ["AAA", "NEW"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL, TICKERS_URL]);
    }

    /// Lookups do not wait for a refresh, even when the server takes ten minutes
    /// to fail, and the next try waits a minute from that failure.
    #[test]
    fn slow_refresh_holds_up_no_lookup_and_backs_off_from_its_end() {
        let c = client(&["AAA"]);
        lookup(&c);
        c.take_fetched();
        c.stub(TICKERS_URL, "<html>Service Unavailable</html>");
        c.tickers.advance(DAY);

        let held = c.hold_fetches();
        let first = companies_within(&c, Duration::from_secs(10)).expect("a lookup with a list waited for the refresh");
        assert_eq!(listed(&first.unwrap()), ["AAA"]);
        // Later lookups get the old list too, and start no other download.
        assert_eq!(listed(&c.companies().unwrap()), ["AAA"]);
        c.tickers.advance(10 * MINUTE);
        drop(held);
        c.tickers.wait_idle();
        assert_eq!(c.take_fetched(), [TICKERS_URL]);

        c.tickers.advance(MINUTE / 2);
        lookup(&c);
        assert!(c.take_fetched().is_empty(), "a new try within a minute of the failure");
        c.tickers.advance(MINUTE / 2);
        lookup(&c);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
    }

    /// The list's age counts time the machine spends asleep, which `Instant` leaves out.
    #[test]
    fn list_age_counts_time_asleep() {
        let c = client(&["AAA"]);
        lookup(&c);
        c.take_fetched();
        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));

        // Two hours awake, then the rest of the day asleep.
        c.tickers.advance(2 * HOUR);
        lookup(&c);
        assert!(c.take_fetched().is_empty());
        c.tickers.sleep(DAY - 2 * HOUR);
        assert_eq!(listed(&lookup(&c)), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        assert_eq!(listed(&lookup(&c)), ["AAA", "NEW"]);

        // When the wall clock is set back, the age is unknown, so the list is downloaded once more.
        c.tickers.set_back(HOUR);
        lookup(&c);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
        lookup(&c);
        assert!(c.take_fetched().is_empty());
    }

    /// A child process forked during the first download inherits the parent's
    /// claim on it, but not the thread doing it, so it downloads the list itself.
    #[test]
    fn first_lookup_does_not_wait_for_a_download_in_another_process() {
        let c = client(&["AAA"]);
        {
            let mut state = c.tickers.state();
            let claim = c.tickers.claim(&mut state, c.tickers.now().instant);
            state.download = Some(Claim { pid: claim.pid.wrapping_add(1), ..claim });
        }
        let got = companies_within(&c, Duration::from_secs(10)).expect("waited for a download in another process");
        assert_eq!(listed(&got.unwrap()), ["AAA"]);
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
    }

    /// A caller with no list waits for a download under way in this process at most the wait limit.
    #[test]
    fn first_lookup_waits_at_most_the_wait_limit() {
        let mut c = client(&["AAA"]);
        Arc::get_mut(&mut c.tickers).unwrap().wait_limit = Duration::from_millis(300);
        // A download that never ends.
        {
            let mut state = c.tickers.state();
            c.tickers.claim(&mut state, c.tickers.now().instant);
        }
        let started = Instant::now();
        let got = companies_within(&c, Duration::from_secs(10)).expect("waited past the wait limit");
        assert_eq!(listed(&got.unwrap()), ["AAA"]);
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert_eq!(c.take_fetched(), [TICKERS_URL]);
    }

    #[test]
    fn retry_wait_doubles_up_to_an_hour() {
        let waits: Vec<u64> = (1..=9).map(|n| retry_wait(n).as_secs() / 60).collect();
        assert_eq!(waits, [1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(retry_wait(u32::MAX), MAX_RETRY_WAIT);
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

    /// The builder's age applies, and a ticker added to the SEC file is found once the refresh has arrived.
    #[test]
    fn new_ticker_is_found_after_refresh() {
        let c = Client::builder("Example Corp admin@example.com").tickers_max_age(Duration::ZERO).build().unwrap();
        c.stub(TICKERS_URL, ticker_file(&["AAA"]));
        assert!(matches!(c.company("NEW"), Err(Error::CompanyNotFound(_))));
        c.stub(TICKERS_URL, ticker_file(&["AAA", "NEW"]));
        // The lookup that starts the refresh still has the old list.
        assert!(matches!(c.company("NEW"), Err(Error::CompanyNotFound(_))));
        c.tickers.wait_idle();
        assert_eq!(c.company("new").unwrap().cik, 2);
        c.tickers.wait_idle();
        assert_eq!(c.take_fetched(), [TICKERS_URL, TICKERS_URL, TICKERS_URL]);
    }
}
