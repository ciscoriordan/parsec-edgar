use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::company::{DEFAULT_TICKERS_MAX_AGE, Tickers};
use crate::encoding;
use crate::error::{Error, Result};

/// Largest response body we accept (the biggest EDGAR documents are a few hundred MB).
const MAX_BODY_BYTES: u64 = 1024 * 1024 * 1024;

/// Builder for [`Client`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    user_agent: String,
    requests_per_second: f64,
    cache_dir: Option<PathBuf>,
    max_retries: u32,
    timeout: Duration,
    tickers_max_age: Duration,
}

impl ClientBuilder {
    /// Maximum request rate to SEC servers. The SEC fair-access limit is 10 per second.
    pub fn requests_per_second(mut self, rps: f64) -> Self {
        self.requests_per_second = rps;
        self
    }

    /// Directory for caching immutable EDGAR archive documents. `None` disables caching.
    pub fn cache_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.cache_dir = dir;
        self
    }

    /// Retries for throttled (429), server-error (5xx) and network failures.
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Timeout for a whole request, including reading the body.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// How long the SEC ticker list behind [`Client::company`] and [`Client::companies`] is used
    /// before it is downloaded again, so a long-lived client sees new and changed tickers.
    /// Default one day. The old list stays in use until the new one has arrived;
    /// `Duration::MAX` keeps the first list for the life of the client.
    pub fn tickers_max_age(mut self, age: Duration) -> Self {
        self.tickers_max_age = age;
        self
    }

    pub fn build(self) -> Result<Client> {
        let ua = self.user_agent.trim().to_owned();
        if ua.is_empty() || !ua.contains('@') {
            return Err(Error::Invalid(
                "the SEC requires a User-Agent with a name and contact email, \
                 e.g. \"Example Corp admin@example.com\""
                    .into(),
            ));
        }
        if !(self.requests_per_second > 0.0 && self.requests_per_second <= 10.0) {
            return Err(Error::Invalid(
                "requests_per_second must be greater than 0 and at most 10 (SEC fair-access limit)".into(),
            ));
        }
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(self.timeout)).http_status_as_error(false).build().into();
        Ok(Client {
            agent,
            user_agent: ua,
            limiter: RateLimiter::new(self.requests_per_second),
            cache_dir: self.cache_dir,
            max_retries: self.max_retries,
            tickers: Tickers::new(self.tickers_max_age),
            requests: AtomicU64::new(0),
            #[cfg(test)]
            stubs: Mutex::default(),
        })
    }
}

/// Client for SEC EDGAR. Cheap to share across threads (`Client: Send + Sync`).
///
/// Every request goes through one rate limiter, so a single client used from many
/// threads stays within the configured rate.
pub struct Client {
    agent: ureq::Agent,
    user_agent: String,
    limiter: RateLimiter,
    cache_dir: Option<PathBuf>,
    max_retries: u32,
    pub(crate) tickers: Tickers,
    requests: AtomicU64,
    /// Test responses served instead of the network, and the URLs fetched.
    #[cfg(test)]
    stubs: Mutex<Option<Stubs>>,
}

#[cfg(test)]
#[derive(Default)]
struct Stubs {
    bodies: std::collections::HashMap<String, Vec<u8>>,
    fetched: Vec<String>,
}

impl Client {
    /// Start building a client. `user_agent` must identify you, as the SEC requires,
    /// for example `"Example Corp admin@example.com"`.
    pub fn builder(user_agent: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            user_agent: user_agent.into(),
            requests_per_second: 8.0,
            cache_dir: None,
            max_retries: 4,
            timeout: Duration::from_secs(120),
            tickers_max_age: DEFAULT_TICKERS_MAX_AGE,
        }
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// GET a URL and return the raw body. Archive documents (`/Archives/`) are
    /// immutable, so they are served from and written to the cache when enabled.
    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let cache_path = self.cache_path(url);
        if let Some(path) = &cache_path
            && let Ok(bytes) = std::fs::read(path)
        {
            return Ok(bytes);
        }
        let bytes = self.fetch(url)?;
        if let Some(path) = &cache_path {
            write_atomic(path, &bytes)?;
        }
        Ok(bytes)
    }

    /// GET a URL without reading or writing the cache, for archive files that
    /// change, such as the current quarter's full index.
    pub fn get_bytes_fresh(&self, url: &str) -> Result<Vec<u8>> {
        self.fetch(url)
    }

    /// HTTP requests sent so far, retries included (cache hits are not requests).
    pub fn requests_sent(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }

    /// GET a URL and decode it as text (UTF-8, falling back to Windows-1252).
    pub fn get_text(&self, url: &str) -> Result<String> {
        Ok(encoding::decode(&self.get_bytes(url)?))
    }

    /// GET a URL and parse it as JSON.
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url)?;
        serde_json::from_slice(&bytes).map_err(|source| Error::Json { url: url.to_owned(), source })
    }

    fn fetch(&self, url: &str) -> Result<Vec<u8>> {
        #[cfg(test)]
        if let Some(stubs) = self.stubs.lock().unwrap().as_mut() {
            self.requests.fetch_add(1, Ordering::Relaxed);
            stubs.fetched.push(url.to_owned());
            return stubs.bodies.get(url).cloned().ok_or_else(|| Error::Status { status: 404, url: url.to_owned() });
        }
        let mut attempt = 0;
        loop {
            self.limiter.acquire();
            self.requests.fetch_add(1, Ordering::Relaxed);
            let outcome =
                self.agent.get(url).header("User-Agent", &self.user_agent).header("Accept-Encoding", "gzip").call();
            let retryable = match outcome {
                Ok(mut resp) => {
                    let status = resp.status().as_u16();
                    if status == 200 {
                        return resp
                            .body_mut()
                            .with_config()
                            .limit(MAX_BODY_BYTES)
                            .read_to_vec()
                            .map_err(|e| Error::Network { url: url.to_owned(), message: e.to_string() });
                    }
                    if status == 429 || status >= 500 {
                        Error::Status { status, url: url.to_owned() }
                    } else {
                        return Err(Error::Status { status, url: url.to_owned() });
                    }
                }
                Err(e) => Error::Network { url: url.to_owned(), message: e.to_string() },
            };
            if attempt >= self.max_retries {
                return Err(retryable);
            }
            // 1s, 2s, 4s, 8s... The SEC asks clients to back off when throttled.
            std::thread::sleep(Duration::from_secs(1 << attempt.min(6)));
            attempt += 1;
        }
    }

    fn cache_path(&self, url: &str) -> Option<PathBuf> {
        let dir = self.cache_dir.as_ref()?;
        let rest = url.strip_prefix("https://www.sec.gov/Archives/")?;
        if rest.is_empty() || rest.contains("..") || rest.contains('?') {
            return None;
        }
        Some(dir.join("Archives").join(rest))
    }

    /// Serve `body` for `url` instead of the network. Once any stub is set,
    /// unknown URLs get a 404 and nothing goes to the network.
    #[cfg(test)]
    pub(crate) fn stub(&self, url: &str, body: impl Into<Vec<u8>>) {
        self.stubs.lock().unwrap().get_or_insert_with(Stubs::default).bodies.insert(url.to_owned(), body.into());
    }

    /// URLs fetched from the stubs so far, in order, then cleared.
    #[cfg(test)]
    pub(crate) fn take_fetched(&self) -> Vec<String> {
        self.stubs.lock().unwrap().as_mut().map(|s| std::mem::take(&mut s.fetched)).unwrap_or_default()
    }
}

/// Write through a temp file and rename it into place, so readers never see a
/// partial file. Each write has its own temp name: threads of one process may
/// cache the same document at once.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let tmp = PathBuf::from(tmp);
    let written = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        // Archive documents never change, so a copy another writer put in place
        // is as good as ours (Windows refuses to replace a file that is open).
        if !path.is_file() {
            return Err(e.into());
        }
    }
    Ok(())
}

/// Spaces requests evenly: each caller reserves the next free slot, then sleeps until it.
struct RateLimiter {
    interval: Duration,
    next: Mutex<Instant>,
}

impl RateLimiter {
    fn new(rps: f64) -> Self {
        Self { interval: Duration::from_secs_f64(1.0 / rps), next: Mutex::new(Instant::now()) }
    }

    fn acquire(&self) {
        let wait = {
            let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            let slot = (*next).max(now);
            *next = slot + self.interval;
            slot - now
        };
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_contact_email() {
        assert!(Client::builder("").build().is_err());
        assert!(Client::builder("Example Corp").build().is_err());
        assert!(Client::builder("Example Corp admin@example.com").build().is_ok());
    }

    #[test]
    fn rejects_rate_over_sec_limit() {
        assert!(Client::builder("a@b.c").requests_per_second(11.0).build().is_err());
        assert!(Client::builder("a@b.c").requests_per_second(0.0).build().is_err());
    }

    #[test]
    fn caches_only_archive_urls() {
        let c = Client::builder("a@b.c").cache_dir(Some("/cache".into())).build().unwrap();
        assert_eq!(
            c.cache_path("https://www.sec.gov/Archives/edgar/data/320193/x/a.htm"),
            Some(PathBuf::from("/cache/Archives/edgar/data/320193/x/a.htm"))
        );
        assert_eq!(c.cache_path("https://data.sec.gov/submissions/CIK0000320193.json"), None);
        assert_eq!(c.cache_path("https://www.sec.gov/Archives/../etc/passwd"), None);
    }

    /// A fresh directory under the system temp directory.
    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("parsec-edgar-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Threads that download the same uncached document at once all succeed,
    /// and the cached file is always complete.
    #[test]
    fn concurrent_cache_writes() {
        let dir = temp_dir("concurrent-cache");
        let c = Client::builder("a@b.c").cache_dir(Some(dir.clone())).build().unwrap();
        let body = vec![b'x'; 512 * 1024];
        for round in 0..30 {
            let url = format!("https://www.sec.gov/Archives/edgar/data/1/{round}/a.htm");
            c.stub(&url, body.clone());
            std::thread::scope(|s| {
                let handles: Vec<_> = (0..8).map(|_| s.spawn(|| c.get_bytes(&url))).collect();
                for h in handles {
                    let got = h.join().unwrap().unwrap_or_else(|e| panic!("round {round}: {e}"));
                    assert_eq!(got.len(), body.len());
                }
            });
            assert_eq!(std::fs::read(c.cache_path(&url).unwrap()).unwrap().len(), body.len());
        }
        // No temp files are left behind.
        let leftovers: Vec<_> = walk(&dir).into_iter().filter(|p| !p.ends_with("a.htm")).collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() { out.extend(walk(&p)) } else { out.push(p) }
        }
        out
    }

    #[test]
    fn limiter_spaces_requests() {
        let l = RateLimiter::new(20.0);
        let start = Instant::now();
        for _ in 0..5 {
            l.acquire();
        }
        // The first slot is immediate, the next four are 50 ms apart.
        assert!(start.elapsed() >= Duration::from_millis(190));
    }
}
