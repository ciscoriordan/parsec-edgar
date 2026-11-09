//! `parsec-edgar crawl`: bulk download and parse filings into JSON.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use clap::Args;
use parsec_edgar::{
    Client, CompanyProfile, CrawlQuery, Error, Filing, FormKind, IndexEntry, Quarter, Section, extract_sections,
};
use serde::Serialize;
use serde::ser::SerializeMap;

/// How long every worker pauses after a 403 from the SEC.
const FORBIDDEN_PAUSE: Duration = Duration::from_secs(60);
/// Throttling episodes in a row (with no successful download between them)
/// that stop the crawl: the first ones pause, this one stops.
const FORBIDDEN_EPISODES: usize = 3;

#[derive(Args)]
pub struct CrawlArgs {
    /// Form type to crawl, e.g. 10-K. Repeat for several. Types match exactly: 10-K does not include 10-K/A.
    #[arg(long = "form", required = true)]
    forms: Vec<String>,

    /// Year or range of years, e.g. 2024 or 2020-2024. Repeat for several.
    #[arg(long = "year", visible_alias = "years", value_name = "YYYY[-YYYY]")]
    years: Vec<String>,

    /// Quarters (1 to 4) within the years [default: all]. Repeat or separate with commas.
    #[arg(long = "quarter", value_delimiter = ',')]
    quarters: Vec<u8>,

    /// Earliest filing date (YYYY-MM-DD, YYYY-MM or YYYY). Without --year, the crawl covers --since to --until.
    #[arg(long)]
    since: Option<String>,

    /// Latest filing date (YYYY-MM-DD, YYYY-MM or YYYY) [default: today].
    #[arg(long)]
    until: Option<String>,

    /// Crawl this company (ticker or CIK) through the submissions API instead of the full index. Repeat for several.
    #[arg(long = "company")]
    companies: Vec<String>,

    /// File with one ticker or CIK per line (`#` starts a comment).
    #[arg(long)]
    companies_file: Option<PathBuf>,

    /// Output directory.
    #[arg(long)]
    out: PathBuf,

    /// Worker threads. They share one client, so together they stay within --rps.
    #[arg(long, default_value_t = 8)]
    threads: usize,

    /// Write every filing to OUT/filings.jsonl instead of one JSON file each.
    #[arg(long)]
    jsonl: bool,

    /// Also save each primary document as served, in OUT/raw.
    #[arg(long)]
    raw: bool,

    /// Keep only these sections, e.g. 1A, 7, 2.02, part_2_item_1a. Repeat for several.
    #[arg(long = "item")]
    items: Vec<String>,

    /// Crawl at most this many filings (the first ones found), for a trial run.
    #[arg(long)]
    limit: Option<usize>,

    /// Find the filings and report how many there are, without downloading them.
    #[arg(long)]
    dry_run: bool,
}

/// One unit of work: a full-index row still to be resolved, or a filing found
/// through the submissions API, with its company's profile.
enum Job {
    Index(IndexEntry),
    Known(Filing, Arc<CompanyProfile>),
}

impl Job {
    fn key(&self) -> (u64, &str, &str, &str) {
        match self {
            Job::Index(e) => (e.cik, &e.form, &e.filing_date, &e.accession_number),
            Job::Known(f, _) => (f.cik, &f.form, &f.filing_date, &f.accession_number),
        }
    }

    /// File name stem shared by the JSON output and the raw document.
    fn stem(&self) -> String {
        let (cik, form, date, acc) = self.key();
        let form: String = form.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        format!("{cik}_{form}_{date}_{acc}")
    }

    /// The first URL the job fetches, for failures that happen before any is known.
    fn url(&self) -> String {
        match self {
            Job::Index(e) => e.index_url(),
            Job::Known(f, _) => filing_url(f),
        }
    }
}

/// Counters shared by the workers.
#[derive(Default)]
struct Stats {
    ok: AtomicUsize,
    failed: AtomicUsize,
    no_sections: AtomicUsize,
    bytes: AtomicU64,
}

type Failures = Mutex<Box<dyn Write + Send>>;

struct Output {
    out: PathBuf,
    jsonl: Option<Mutex<File>>,
    failures: Failures,
    raw: bool,
    items: Vec<String>,
}

pub fn run(client: &Client, args: CrawlArgs) -> Result<(), Error> {
    let query = build_query(&args)?;
    let companies = read_companies(&args)?;
    if companies.is_empty() && !query.has_range() {
        return Err(Error::Invalid("give --year or --since for a crawl of the full index".into()));
    }
    fs::create_dir_all(&args.out)?;
    let started = Instant::now();

    // Discover. Failures wait in memory until discovery is over, so an error
    // here leaves the previous run's failures.jsonl as it was.
    let discovery_failures: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());
    let mut jobs: Vec<Job> = if companies.is_empty() {
        let mut jobs = Vec::new();
        for q in query.quarters_to_read()? {
            let rows = client.full_index(q)?;
            if rows.is_empty() {
                eprintln!("index {q}: not published yet (the SEC builds it after the quarter's first business day)");
                continue;
            }
            let total = rows.len();
            let kept = query.select(rows);
            eprintln!("index {q}: {total} filings, {} match", kept.len());
            jobs.extend(kept.into_iter().map(Job::Index));
        }
        jobs
    } else {
        if companies.iter().any(|c| parse_cik(c).is_none()) {
            // Load the ticker file once, before the workers race to load it.
            client.companies()?;
        }
        let found = Mutex::new(Vec::new());
        parallel(&companies, args.threads, |company| match client.crawl_company_with_profile(company, &query) {
            Ok((filings, profile)) => {
                eprintln!("{company}: {} filings", filings.len());
                let profile = Arc::new(profile);
                lock(&found).extend(filings.into_iter().map(|f| Job::Known(f, profile.clone())));
            }
            Err(e) => {
                eprintln!("{company}: {e}");
                lock(&discovery_failures).push(serde_json::json!({ "company": company, "error": e.to_string() }));
            }
        });
        let mut jobs = found.into_inner().unwrap_or_else(|e| e.into_inner());
        jobs.sort_by(|a, b| a.key().cmp(&b.key()));
        // A filing shared by two listed companies (co-registrants) is crawled once.
        let mut seen = HashSet::new();
        jobs.retain(|j| seen.insert(j.key().3.to_owned()));
        jobs
    };
    let discovery_failures = discovery_failures.into_inner().unwrap_or_else(|e| e.into_inner());
    let discovery_requests = client.requests_sent();
    let discovery_time = started.elapsed();
    if let Some(n) = args.limit {
        jobs.truncate(n);
    }

    // Skip filings already done in an earlier run.
    let jsonl_path = args.out.join("filings.jsonl");
    let done: HashSet<String> = if args.jsonl { completed_jsonl(&jsonl_path, !args.dry_run)? } else { HashSet::new() };
    let found = jobs.len();
    jobs.retain(|j| {
        if args.jsonl {
            !done.contains(j.key().3)
        } else {
            !record_complete(&args.out.join(format!("{}.json", j.stem())))
        }
    });
    let skipped = found - jobs.len();
    let per_filing = if companies.is_empty() { 2 } else { 1 };
    eprintln!(
        "found {found} filings in {:.1}s ({discovery_requests} requests), {skipped} already done, {} to crawl \
         (about {} requests)",
        discovery_time.as_secs_f64(),
        jobs.len(),
        jobs.len() * per_filing
    );
    if args.dry_run {
        return Ok(());
    }

    // Crawl.
    remove_stale_temp_files(&args.out)?;
    remove_stale_temp_files(&args.out.join("raw"))?;
    if args.raw {
        fs::create_dir_all(args.out.join("raw"))?;
    }
    // Rewritten on each run, since failed filings are tried again.
    let failures_path = args.out.join("failures.jsonl");
    let failures: Failures = Mutex::new(Box::new(BufWriter::new(File::create(&failures_path)?)));
    for f in &discovery_failures {
        write_line(&failures, f);
    }
    let output = Output {
        jsonl: if args.jsonl {
            Some(Mutex::new(OpenOptions::new().create(true).append(true).open(&jsonl_path)?))
        } else {
            None
        },
        out: args.out.clone(),
        failures,
        raw: args.raw,
        items: args.items.iter().map(|i| crate::normalize_section_id(i)).collect(),
    };
    let stats = Stats::default();
    let throttle = Throttle::new(FORBIDDEN_PAUSE, FORBIDDEN_EPISODES);
    let crawl_started = Instant::now();
    let total = jobs.len();
    with_progress(
        Duration::from_secs(10),
        || eprintln!("{}", progress_line(client, &stats, total, crawl_started, discovery_requests)),
        || parallel(&jobs, args.threads, |job| crawl_job(client, job, &output, &stats, &throttle)),
    );
    if let Some(j) = &output.jsonl {
        lock(j).sync_all()?;
    }
    lock(&output.failures).flush()?;

    let ok = stats.ok.load(Ordering::Relaxed);
    let failed = stats.failed.load(Ordering::Relaxed) + discovery_failures.len();
    let elapsed = crawl_started.elapsed().as_secs_f64();
    let crawl_requests = client.requests_sent() - discovery_requests;
    let done_now = ok + stats.failed.load(Ordering::Relaxed);
    eprintln!(
        "done in {:.1}s: ok {ok}, failed {failed}, skipped {skipped}; {} with no sections found (full text kept); \
         {:.1} filings/min, {:.2} requests/filing ({crawl_requests} for filings, {discovery_requests} for discovery), \
         {:.1} MB of documents",
        started.elapsed().as_secs_f64(),
        stats.no_sections.load(Ordering::Relaxed),
        if elapsed > 0.0 { ok as f64 * 60.0 / elapsed } else { 0.0 },
        if done_now > 0 { crawl_requests as f64 / done_now as f64 } else { 0.0 },
        stats.bytes.load(Ordering::Relaxed) as f64 / 1e6,
    );
    if throttle.stopped() {
        return Err(Error::Invalid(format!(
            "stopped with {} filings not started: the SEC kept answering 403, which usually means the rate \
             limit was exceeded. Wait ten minutes and run the same command again to resume.",
            total - done_now
        )));
    }
    if failed > 0 {
        return Err(Error::Invalid(format!(
            "{failed} failed, reasons in {}; run the same command again to retry them",
            failures_path.display()
        )));
    }
    Ok(())
}

/// Crawl one filing and record the outcome. On a 403, the throttle decides
/// whether to pause and try once more, or to stop.
fn crawl_job(client: &Client, job: &Job, output: &Output, stats: &Stats, throttle: &Throttle) {
    if !throttle.wait() {
        return;
    }
    let attempt = || {
        let started = Instant::now();
        let before = client.requests_sent();
        let result = catch_panic(|| job.url(), || crawl_one(client, job, output, stats));
        // Approximate with several workers, but zero means a pure cache hit.
        (started, client.requests_sent() > before, result)
    };
    let (mut started, mut sent, mut result) = attempt();
    if is_forbidden(&result)
        && throttle.first_forbidden(started, Instant::now()) == OnForbidden::Retry
        && throttle.wait()
    {
        (started, sent, result) = attempt();
        if is_forbidden(&result) {
            throttle.retry_forbidden(Instant::now());
        }
    }
    match result {
        Ok(()) => {
            throttle.success(started, sent);
            stats.ok.fetch_add(1, Ordering::Relaxed);
        }
        Err((url, e)) => {
            stats.failed.fetch_add(1, Ordering::Relaxed);
            let (cik, form, date, acc) = job.key();
            write_line(
                &output.failures,
                &serde_json::json!({
                    "cik": cik, "form": form, "filing_date": date, "accession_number": acc,
                    "url": url, "error": e.to_string(),
                }),
            );
        }
    }
}

/// Run `f`, turning a panic (a parser bug on an unusual document, say) into an
/// error for this filing, so the crawl goes on and failures.jsonl records it.
fn catch_panic<T>(
    url: impl FnOnce() -> String,
    f: impl FnOnce() -> Result<T, (String, Error)>,
) -> Result<T, (String, Error)> {
    panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown cause".into());
        Err((url(), Error::Invalid(format!("panic while crawling: {message}"))))
    })
}

/// Fetch, parse and write one filing. On failure, returns the URL involved and the error.
fn crawl_one(client: &Client, job: &Job, output: &Output, stats: &Stats) -> Result<(), (String, Error)> {
    let (filing, profile) = match job {
        Job::Index(entry) => {
            let (filing, index) = client.resolve_index_entry(entry).map_err(|e| (entry.index_url(), e))?;
            let profile = index.filer(entry.cik).map(|f| f.profile.clone());
            (filing, profile)
        }
        Job::Known(filing, profile) => (filing.clone(), Some(profile.as_ref().clone())),
    };
    let doc = client.fetch_primary_document(&filing).map_err(|e| (filing_url(&filing), e))?;
    stats.bytes.fetch_add(doc.bytes.len() as u64, Ordering::Relaxed);
    let text = doc.text();
    let split = split_sections(&text, &filing.form, &output.items);
    if split.no_sections_found {
        stats.no_sections.fetch_add(1, Ordering::Relaxed);
    }

    let stem = job.stem();
    let io = |e: io::Error| (output.out.display().to_string(), Error::Io(e));
    let raw_file = if output.raw {
        let name = format!("raw/{stem}.{}", doc.extension());
        write_atomic(&output.out.join(&name), &doc.bytes).map_err(io)?;
        Some(name)
    } else {
        None
    };
    let sections = &split.sections;
    let record = Record {
        cik: filing.cik,
        company: &filing.company,
        form: &filing.form,
        filing_date: &filing.filing_date,
        report_date: filing.report_date.as_deref(),
        acceptance_datetime: filing.acceptance_datetime.as_deref(),
        accession_number: &filing.accession_number,
        items: &filing.items,
        sic: profile.as_ref().and_then(|p| p.sic.as_deref()),
        sic_description: profile.as_ref().and_then(|p| p.sic_description.as_deref()),
        state_of_incorporation: profile.as_ref().and_then(|p| p.state_of_incorporation.as_deref()),
        state_location: profile.as_ref().and_then(|p| p.state_location.as_deref()),
        fiscal_year_end: profile.as_ref().and_then(|p| p.fiscal_year_end.as_deref()),
        index_url: &filing.index_url,
        primary_document: filing.primary_document.as_deref(),
        primary_document_url: filing.primary_document_url.as_deref(),
        full_submission_url: filing.full_submission_url(),
        raw_file,
        text_chars: text.chars().count(),
        section_list: sections.iter().map(SectionEntry).collect(),
        sections: Sections(sections),
        text: split.with_text.then_some(text.as_str()),
    };
    match &output.jsonl {
        Some(jsonl) => {
            let mut line = serde_json::to_vec(&record).expect("records serialize");
            line.push(b'\n');
            append_line(&mut *lock(jsonl), &line).map_err(io)?;
        }
        None => {
            let json = serde_json::to_vec_pretty(&record).expect("records serialize");
            write_atomic(&output.out.join(format!("{stem}.json")), &json).map_err(io)?;
        }
    }
    Ok(())
}

/// The Item sections a record keeps, and whether it also carries the full text.
struct Split {
    sections: Vec<Section>,
    /// The form has no Item sections, or none was found: the text is the content.
    with_text: bool,
    /// A 10-K, 10-Q or 8-K in which no Item section was found.
    no_sections_found: bool,
}

fn split_sections(text: &str, form: &str, items: &[String]) -> Split {
    let kind = FormKind::from_form(form);
    let mut sections = kind.map(|k| extract_sections(text, k)).unwrap_or_default();
    let with_text = sections.is_empty();
    // --item narrows the sections a filing has; it does not bring back the full text.
    if !items.is_empty() {
        sections.retain(|s| items.contains(&s.id));
    }
    Split { sections, with_text, no_sections_found: kind.is_some() && with_text }
}

fn filing_url(f: &Filing) -> String {
    f.primary_document_url.clone().unwrap_or_else(|| f.full_submission_url())
}

/// One output record. Field names follow edgar-crawler where they overlap.
#[derive(Serialize)]
struct Record<'a> {
    cik: u64,
    company: &'a str,
    form: &'a str,
    filing_date: &'a str,
    report_date: Option<&'a str>,
    acceptance_datetime: Option<&'a str>,
    accession_number: &'a str,
    items: &'a [String],
    sic: Option<&'a str>,
    sic_description: Option<&'a str>,
    state_of_incorporation: Option<&'a str>,
    state_location: Option<&'a str>,
    fiscal_year_end: Option<&'a str>,
    index_url: &'a str,
    primary_document: Option<&'a str>,
    primary_document_url: Option<&'a str>,
    full_submission_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_file: Option<String>,
    text_chars: usize,
    /// Every section without its text, in filing order.
    section_list: Vec<SectionEntry<'a>>,
    /// Section text keyed by id, in filing order.
    sections: Sections<'a>,
    /// Full text, for forms without Item sections and filings where none was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
}

/// Serializes as a map from id to text, keeping filing order.
struct Sections<'a>(&'a [Section]);

impl Serialize for Sections<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for s in self.0 {
            map.serialize_entry(&s.id, &s.text)?;
        }
        map.end()
    }
}

/// A section without its text, as a list entry.
struct SectionEntry<'a>(&'a Section);

impl Serialize for SectionEntry<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        const ORDER: [&str; 7] = ["id", "part", "item", "title", "heading", "start", "end"];
        let value = serde_json::to_value(self.0).map_err(serde::ser::Error::custom)?;
        let fields = value.as_object().cloned().unwrap_or_default();
        let mut map = serializer.serialize_map(None)?;
        for key in ORDER {
            if let Some(v) = fields.get(key) {
                map.serialize_entry(key, v)?;
            }
        }
        for (key, v) in &fields {
            if key != "text" && !ORDER.contains(&key.as_str()) {
                map.serialize_entry(key, v)?;
            }
        }
        map.serialize_entry("chars", &self.0.text.chars().count())?;
        map.end()
    }
}

fn build_query(args: &CrawlArgs) -> Result<CrawlQuery, Error> {
    let mut query = CrawlQuery {
        forms: args.forms.iter().map(|f| f.trim().to_owned()).collect(),
        since: args.since.clone(),
        until: args.until.clone(),
        quarters: Vec::new(),
    };
    if query.forms.iter().any(|f| f.is_empty()) {
        return Err(Error::Invalid("--form must not be empty".into()));
    }
    let wanted: Vec<u8> = if args.quarters.is_empty() { vec![1, 2, 3, 4] } else { args.quarters.clone() };
    if let Some(q) = wanted.iter().find(|q| !(1..=4).contains(*q)) {
        return Err(Error::Invalid(format!("quarters are 1 to 4, got {q}")));
    }
    for spec in &args.years {
        let (from, to) = parse_years(spec)?;
        for year in from..=to {
            for &q in &wanted {
                query.quarters.push(Quarter::new(year, q)?);
            }
        }
    }
    if args.years.is_empty() && !args.quarters.is_empty() {
        // Quarter numbers without years narrow the --since/--until range.
        let since = args.since.as_deref().ok_or_else(|| Error::Invalid("--quarter needs --year or --since".into()))?;
        let until = match &args.until {
            Some(u) => Quarter::of_date(u)?,
            None => Quarter::current(),
        };
        query.quarters = Quarter::range(Quarter::of_date(since)?, until)
            .into_iter()
            .filter(|q| wanted.contains(&q.quarter()))
            .collect();
    }
    Ok(query)
}

/// `2024`, `2020-2024` or `2020..2024`.
fn parse_years(spec: &str) -> Result<(u16, u16), Error> {
    let bad = || Error::Invalid(format!("--year takes YYYY or YYYY-YYYY, got {spec:?}"));
    let spec = spec.trim();
    let (a, b) = spec.split_once("..").or_else(|| spec.split_once('-')).unwrap_or((spec, spec));
    let from: u16 = a.trim().parse().map_err(|_| bad())?;
    let to: u16 = b.trim().parse().map_err(|_| bad())?;
    if from > to || !(1000..=9999).contains(&from) {
        return Err(bad());
    }
    Ok((from, to))
}

fn read_companies(args: &CrawlArgs) -> Result<Vec<String>, Error> {
    let mut out: Vec<String> = args.companies.iter().map(|c| c.trim().to_owned()).filter(|c| !c.is_empty()).collect();
    if let Some(path) = &args.companies_file {
        let text = fs::read_to_string(path)?;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            out.extend(line.split([',', ' ', '\t']).map(str::trim).filter(|c| !c.is_empty()).map(str::to_owned));
        }
    }
    let mut seen = HashSet::new();
    out.retain(|c| seen.insert(c.to_ascii_uppercase()));
    Ok(out)
}

/// `320193`, `0000320193` or `CIK320193`.
fn parse_cik(s: &str) -> Option<u64> {
    let s = s.trim();
    let digits = s.strip_prefix("CIK").or_else(|| s.strip_prefix("cik")).unwrap_or(s);
    (!digits.is_empty() && digits.len() <= 10 && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// Accession numbers of the complete records in a JSON lines file. Records are
/// read in order up to the first line that is not one whole JSON value (a
/// partial last line left by a killed run, or a failed write that a later
/// record ran into). With `cut`, the file is cut there, so the filings from
/// that line on are crawled again and new records start on a clean line.
fn completed_jsonl(path: &Path, cut: bool) -> Result<HashSet<String>, Error> {
    let mut done = HashSet::new();
    let Ok(file) = File::open(path) else { return Ok(done) };
    let len = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut valid_end = 0u64;
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 || line.last() != Some(&b'\n') || serde_json::from_slice::<serde::de::IgnoredAny>(&line).is_err() {
            break;
        }
        valid_end += n as u64;
        if let Some(acc) = accession_in(&line) {
            done.insert(acc);
        }
    }
    if cut && valid_end < len {
        eprintln!(
            "{}: cut {} bytes after the last complete record; those filings are crawled again",
            path.display(),
            len - valid_end
        );
        OpenOptions::new().write(true).open(path)?.set_len(valid_end)?;
    }
    Ok(done)
}

/// The accession number of a record line, without parsing the whole (possibly large) line.
fn accession_in(line: &[u8]) -> Option<String> {
    const KEY: &[u8] = b"\"accession_number\":\"";
    let head = &line[..line.len().min(4096)];
    let at = head.windows(KEY.len()).position(|w| w == KEY)? + KEY.len();
    let acc = std::str::from_utf8(line.get(at..at + 20)?).ok()?;
    acc.bytes().all(|b| b.is_ascii_digit() || b == b'-').then(|| acc.to_owned())
}

/// A file that records are appended to, and that can be cut back.
trait AppendFile: Write {
    fn size(&self) -> io::Result<u64>;
    fn cut(&mut self, len: u64) -> io::Result<()>;
}

impl AppendFile for File {
    fn size(&self) -> io::Result<u64> {
        Ok(self.metadata()?.len())
    }

    fn cut(&mut self, len: u64) -> io::Result<()> {
        self.set_len(len)
    }
}

/// Append one record line, newline included. If the write fails partway (on
/// a full disk, say), the file is cut back to where it was, so a partial
/// record cannot run into the next one.
fn append_line(file: &mut impl AppendFile, line: &[u8]) -> io::Result<()> {
    let start = file.size()?;
    let result = file.write_all(line).and_then(|_| file.flush());
    if result.is_err() {
        let _ = file.cut(start);
    }
    result
}

/// Whether a record file is complete. Records are pretty JSON, which starts
/// with `{` and ends with a `}` on its own line; a file left empty or cut
/// short by a crash is crawled again.
fn record_complete(path: &Path) -> bool {
    let check = || -> io::Result<bool> {
        let mut f = File::open(path)?;
        if f.metadata()?.len() < 4 {
            return Ok(false);
        }
        let (mut head, mut tail) = ([0u8; 1], [0u8; 2]);
        f.read_exact(&mut head)?;
        f.seek(SeekFrom::End(-2))?;
        f.read_exact(&mut tail)?;
        Ok(&head == b"{" && &tail == b"\n}")
    };
    check().unwrap_or(false)
}

/// Delete the temporary files (`{stem}.tmp{pid}`) that killed runs left in a directory.
fn remove_stale_temp_files(dir: &Path) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let path = entry?.path();
        let pid = path.extension().and_then(|e| e.to_str()).and_then(|e| e.strip_prefix("tmp"));
        if pid.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) && path.is_file() {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Run `f` on every item with `threads` workers.
fn parallel<T: Sync>(items: &[T], threads: usize, f: impl Fn(&T) + Sync) {
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.clamp(1, 64).min(items.len().max(1)) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    f(item);
                }
            });
        }
    });
}

/// Run `work` while another thread calls `report` every `every`. The reporter
/// stops when the work ends, also when it ends in a panic, which then reaches
/// the caller instead of leaving the crawl hanging.
fn with_progress<R>(every: Duration, report: impl Fn() + Sync, work: impl FnOnce() -> R) -> R {
    struct Finish<'a>(&'a AtomicBool);
    impl Drop for Finish<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }
    let finished = AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            let mut last = Instant::now();
            while !finished.load(Ordering::Relaxed) {
                std::thread::sleep(every.min(Duration::from_millis(200)));
                if last.elapsed() >= every && !finished.load(Ordering::Relaxed) {
                    last = Instant::now();
                    report();
                }
            }
        });
        let _finish = Finish(&finished);
        work()
    })
}

/// What a worker does after a 403 on a first attempt.
#[derive(Debug, PartialEq)]
enum OnForbidden {
    /// Wait for the pause, then try the filing once more.
    Retry,
    /// The crawl is stopping: record the filing as failed.
    Stop,
}

/// How the workers react to 403 responses, which the SEC sends to a client
/// over its rate limit. The workers all run into a block at about the same
/// time, so the 403s are grouped into episodes: the first 403 that no pause
/// covers starts one and pauses every worker. 403s that arrive during the
/// pause, or come from requests sent before it began, belong to the same
/// episode. A retry's 403 starts none (a missing file can answer 403 too);
/// the next first attempts show whether the block goes on. When `max_episodes`
/// come in a row with no successful download between them, the crawl stops.
struct Throttle {
    pause: Duration,
    max_episodes: usize,
    state: Mutex<ThrottleState>,
}

#[derive(Default)]
struct ThrottleState {
    pause_started: Option<Instant>,
    pause_until: Option<Instant>,
    /// Episodes since the last successful download.
    episodes: usize,
    last_forbidden: Option<Instant>,
    stopped: bool,
}

impl Throttle {
    fn new(pause: Duration, max_episodes: usize) -> Self {
        Self { pause, max_episodes, state: Mutex::default() }
    }

    /// Sleep through any pause. False once the crawl is stopping.
    fn wait(&self) -> bool {
        loop {
            let (until, stopped) = {
                let s = lock(&self.state);
                (s.pause_until, s.stopped)
            };
            let now = Instant::now();
            match until {
                _ if stopped => return false,
                Some(t) if t > now => std::thread::sleep(t - now),
                _ => return true,
            }
        }
    }

    /// A first attempt that started at `started` got a 403 at `now`.
    fn first_forbidden(&self, started: Instant, now: Instant) -> OnForbidden {
        let mut s = lock(&self.state);
        s.last_forbidden = Some(now);
        if s.stopped {
            return OnForbidden::Stop;
        }
        let pausing = s.pause_until.is_some_and(|t| t > now);
        let sent_before_pause = s.pause_started.is_some_and(|t| t >= started);
        if pausing || sent_before_pause {
            return OnForbidden::Retry;
        }
        s.episodes += 1;
        if s.episodes >= self.max_episodes {
            s.stopped = true;
            return OnForbidden::Stop;
        }
        s.pause_started = Some(now);
        s.pause_until = Some(now + self.pause);
        OnForbidden::Retry
    }

    /// A retry after a pause got a 403 at `now`.
    fn retry_forbidden(&self, now: Instant) {
        lock(&self.state).last_forbidden = Some(now);
    }

    /// A filing that started at `started` was crawled. It ends the run of
    /// episodes only if it reached the SEC (not just the cache) and no 403
    /// came in while it ran.
    fn success(&self, started: Instant, reached_network: bool) {
        let mut s = lock(&self.state);
        if reached_network && s.last_forbidden.is_none_or(|t| t < started) {
            s.episodes = 0;
        }
    }

    fn stopped(&self) -> bool {
        lock(&self.state).stopped
    }
}

fn is_forbidden(r: &Result<(), (String, Error)>) -> bool {
    matches!(r, Err((_, Error::Status { status: 403, .. })))
}

/// Lock a mutex, also after a panic in another worker poisoned it.
fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn progress_line(client: &Client, stats: &Stats, total: usize, started: Instant, base_requests: u64) -> String {
    let ok = stats.ok.load(Ordering::Relaxed);
    let failed = stats.failed.load(Ordering::Relaxed);
    let done = ok + failed;
    let mins = started.elapsed().as_secs_f64() / 60.0;
    let rate = if mins > 0.0 { done as f64 / mins } else { 0.0 };
    let eta = if rate > 0.0 { (total - done.min(total)) as f64 / rate } else { 0.0 };
    let requests = client.requests_sent() - base_requests;
    format!(
        "{done}/{total} (ok {ok}, failed {failed}), {rate:.1} filings/min, {:.2} requests/filing, about {eta:.0} min left",
        if done > 0 { requests as f64 / done as f64 } else { 0.0 }
    )
}

fn write_line(w: &Failures, value: &serde_json::Value) {
    let mut w = lock(w);
    let _ = writeln!(w, "{value}").and_then(|_| w.flush());
}

/// Write a file so that it is complete or absent, also after a power failure:
/// write a temporary file, flush it to disk, then rename it into place.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let result = File::create(&tmp)
        .and_then(|mut f| {
            f.write_all(bytes)?;
            f.sync_all()
        })
        .and_then(|_| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Test {
        #[command(flatten)]
        args: CrawlArgs,
    }

    fn args(cli: &[&str]) -> CrawlArgs {
        let mut argv = vec!["test", "--out", "x"];
        argv.extend_from_slice(cli);
        Test::parse_from(argv).args
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("parsec-crawl-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record_line(acc: &str) -> String {
        format!(
            r#"{{"cik":1,"company":"A","form":"8-K","filing_date":"2024-01-02","report_date":null,"acceptance_datetime":null,"accession_number":"{acc}","sections":{{}}}}"#
        )
    }

    #[test]
    fn years_and_quarters() {
        assert_eq!(parse_years("2024").unwrap(), (2024, 2024));
        assert_eq!(parse_years("2020-2024").unwrap(), (2020, 2024));
        assert_eq!(parse_years("2020..2024").unwrap(), (2020, 2024));
        assert!(parse_years("2024-2020").is_err());
        assert!(parse_years("24").is_err());

        let q = build_query(&args(&["--form", "10-K", "--year", "2023-2024", "--quarter", "1,4"])).unwrap();
        let names: Vec<String> = q.quarters.iter().map(|q| q.to_string()).collect();
        assert_eq!(names, ["2023Q1", "2023Q4", "2024Q1", "2024Q4"]);

        let q = build_query(&args(&["--form", "8-K", "--since", "2024-11-05", "--until", "2024-11-06"])).unwrap();
        assert!(q.quarters.is_empty());
        assert_eq!(q.quarters_to_read().unwrap(), [Quarter::new(2024, 4).unwrap()]);

        let q = build_query(&args(&["--form", "8-K", "--since", "2023", "--until", "2024-12", "--quarter", "2"]))
            .unwrap();
        assert_eq!(q.quarters, [Quarter::new(2023, 2).unwrap(), Quarter::new(2024, 2).unwrap()]);

        assert!(build_query(&args(&["--form", "8-K", "--quarter", "2"])).is_err());
        assert!(build_query(&args(&["--form", "8-K", "--year", "2024", "--quarter", "5"])).is_err());
    }

    #[test]
    fn output_names() {
        let entry = IndexEntry {
            cik: 320193,
            company: "Apple Inc.".into(),
            form: "10-K/A".into(),
            filing_date: "2024-11-01".into(),
            accession_number: "0000320193-24-000123".into(),
        };
        assert_eq!(Job::Index(entry).stem(), "320193_10-K-A_2024-11-01_0000320193-24-000123");
    }

    #[test]
    fn ciks() {
        assert_eq!(parse_cik("CIK0000320193"), Some(320193));
        assert_eq!(parse_cik("320193"), Some(320193));
        assert_eq!(parse_cik("AAPL"), None);
    }

    #[test]
    fn resumes_from_jsonl_and_cuts_partial_line() {
        let dir = temp_dir("jsonl");
        let path = dir.join("filings.jsonl");
        let full = record_line("0000000001-24-000001");
        fs::write(&path, format!("{full}\n{{\"cik\":2,\"accession_number\":\"0000000002-24-0")).unwrap();
        // A dry run reads but does not cut.
        assert_eq!(completed_jsonl(&path, false).unwrap().len(), 1);
        assert_ne!(fs::read_to_string(&path).unwrap(), format!("{full}\n"));
        let done = completed_jsonl(&path, true).unwrap();
        assert_eq!(done.len(), 1);
        assert!(done.contains("0000000001-24-000001"));
        assert_eq!(fs::read_to_string(&path).unwrap(), format!("{full}\n"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resume_stops_at_a_merged_line() {
        // A failed write left half of record 2, and record 3 was appended to it.
        let dir = temp_dir("merged");
        let path = dir.join("filings.jsonl");
        let (r1, r2, r3, r4) = (
            record_line("0000000001-24-000001"),
            record_line("0000000002-24-000002"),
            record_line("0000000003-24-000003"),
            record_line("0000000004-24-000004"),
        );
        let half = &r2[..r2.len() / 2];
        fs::write(&path, format!("{r1}\n{half}{r3}\n{r4}\n")).unwrap();
        let done = completed_jsonl(&path, true).unwrap();
        // Record 2 is not done, and record 3, merged into its line, is crawled again.
        assert_eq!(done, HashSet::from(["0000000001-24-000001".to_owned()]));
        assert_eq!(fs::read_to_string(&path).unwrap(), format!("{r1}\n"));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A disk with `room` bytes that takes writes in small pieces.
    struct Disk {
        data: Vec<u8>,
        room: usize,
    }

    impl Write for Disk {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let free = self.room.saturating_sub(self.data.len());
            if free == 0 {
                return Err(io::Error::other("no space left on device"));
            }
            let n = buf.len().min(free).min(16);
            self.data.extend_from_slice(&buf[..n]);
            Ok(n)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl AppendFile for Disk {
        fn size(&self) -> io::Result<u64> {
            Ok(self.data.len() as u64)
        }

        fn cut(&mut self, len: u64) -> io::Result<()> {
            self.data.truncate(len as usize);
            Ok(())
        }
    }

    #[test]
    fn failed_append_leaves_no_partial_record() {
        let (r1, r2, r3) = (
            format!("{}\n", record_line("0000000001-24-000001")),
            format!("{}\n", record_line("0000000002-24-000002")),
            format!("{}\n", record_line("0000000003-24-000003")),
        );
        let mut disk = Disk { data: Vec::new(), room: r1.len() + 40 };
        append_line(&mut disk, r1.as_bytes()).unwrap();
        assert!(append_line(&mut disk, r2.as_bytes()).is_err());
        assert_eq!(disk.data, r1.as_bytes());
        disk.room = 10_000;
        append_line(&mut disk, r3.as_bytes()).unwrap();
        assert_eq!(String::from_utf8(disk.data).unwrap(), format!("{r1}{r3}"));
    }

    #[test]
    fn appends_to_a_real_file() {
        let dir = temp_dir("append");
        let path = dir.join("filings.jsonl");
        let mut f = OpenOptions::new().create(true).append(true).open(&path).unwrap();
        for acc in ["0000000001-24-000001", "0000000002-24-000002"] {
            append_line(&mut f, format!("{}\n", record_line(acc)).as_bytes()).unwrap();
        }
        assert_eq!(completed_jsonl(&path, true).unwrap().len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn json_records_must_be_complete_to_count_as_done() {
        let dir = temp_dir("records");
        let path = dir.join("r.json");
        write_atomic(&path, b"{\n  \"cik\": 1,\n  \"sections\": {}\n}").unwrap();
        assert!(record_complete(&path));
        // Left empty or cut short by a crash: crawled again.
        fs::write(&path, b"").unwrap();
        assert!(!record_complete(&path));
        fs::write(&path, b"{\n  \"cik\": 1,\n  \"section_list\": [\n    {\n      \"id\": \"item_1\"\n    }").unwrap();
        assert!(!record_complete(&path));
        assert!(!record_complete(&dir.join("missing.json")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn removes_stale_temp_files_only() {
        let dir = temp_dir("stale");
        for name in ["a.tmp123", "a.json", "b.tmpx", "c.htm"] {
            fs::write(dir.join(name), b"x").unwrap();
        }
        remove_stale_temp_files(&dir).unwrap();
        remove_stale_temp_files(&dir.join("raw")).unwrap();
        let mut left: Vec<String> =
            fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["a.json", "b.tmpx", "c.htm"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn discovery_error_keeps_previous_failures() {
        let dir = temp_dir("failures");
        let failures = dir.join("failures.jsonl");
        fs::write(&failures, "{\"accession_number\":\"0000000001-24-000001\"}\n").unwrap();
        let client = Client::builder("parsec-edgar tests dev@pars.ec").build().unwrap();
        // Fails during discovery, before any request.
        let mut a = args(&["--form", "8-K", "--since", "2024/01/01"]);
        a.out = dir.clone();
        assert!(run(&client, a).is_err());
        assert_eq!(fs::read_to_string(&failures).unwrap(), "{\"accession_number\":\"0000000001-24-000001\"}\n");
        assert_eq!(client.requests_sent(), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn filings_without_sections_keep_their_text() {
        let items_none: Vec<String> = Vec::new();
        // A 10-K/A that only adds the auditor's name has no Item headings.
        let amendment = "AMENDMENT NO. 1\n\nThis amendment adds the name of our auditor, Example LLP, \
                         to the cover page. No other change is made.\n";
        let split = split_sections(amendment, "10-K/A", &items_none);
        assert!(split.sections.is_empty() && split.with_text && split.no_sections_found);

        // Forms without Item sections always carry the text.
        let split = split_sections("Prospectus text", "S-1", &items_none);
        assert!(split.with_text && !split.no_sections_found);

        let eight_k = "Item 2.02 Results of Operations and Financial Condition\n\nRevenue rose.\n\n\
                       Item 9.01 Financial Statements and Exhibits\n\nExhibit 99.1 Press release.\n";
        let split = split_sections(eight_k, "8-K", &items_none);
        assert!(!split.sections.is_empty() && !split.with_text);
        // --item narrows the sections and does not bring back the text.
        let split = split_sections(eight_k, "8-K", &["item_5.02".to_owned()]);
        assert!(split.sections.is_empty() && !split.with_text && !split.no_sections_found);
    }

    #[test]
    fn panics_become_failures() {
        let r: Result<(), _> = catch_panic(|| "https://www.sec.gov/x.htm".into(), || panic!("table without rows"));
        let (url, e) = r.unwrap_err();
        assert_eq!(url, "https://www.sec.gov/x.htm");
        assert!(e.to_string().contains("table without rows"), "{e}");
        assert_eq!(catch_panic(String::new, || Ok::<_, (String, Error)>(7)).unwrap(), 7);
    }

    #[test]
    fn panic_in_a_worker_does_not_hang_the_crawl() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = panic::catch_unwind(|| {
                with_progress(
                    Duration::from_secs(60),
                    || {},
                    || {
                        parallel(&[1, 2, 3], 2, |i| {
                            if *i == 2 {
                                panic!("parser bug");
                            }
                        })
                    },
                )
            });
            let _ = tx.send(r.is_err());
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)), Ok(true));
    }

    #[test]
    fn concurrent_403s_are_one_episode() {
        let throttle = Throttle::new(Duration::from_secs(60), 3);
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        // Eight workers hit a block at about the same time. Five 403s used to stop the crawl.
        for i in 0..8 {
            assert_eq!(throttle.first_forbidden(at(0), at(100 + i * 70)), OnForbidden::Retry, "worker {i}");
        }
        assert!(!throttle.stopped());
        // A slow request sent before the pause began is part of the same episode,
        // even when its 403 arrives after the pause.
        assert_eq!(throttle.first_forbidden(at(50), at(61_000)), OnForbidden::Retry);
        // After the pause the retries still get 403; that alone starts no episode.
        throttle.retry_forbidden(at(60_200));
        // The next first attempts get 403 too: a second episode, another pause.
        assert_eq!(throttle.first_forbidden(at(60_300), at(60_400)), OnForbidden::Retry);
        assert_eq!(throttle.first_forbidden(at(60_300), at(60_500)), OnForbidden::Retry);
        assert!(!throttle.stopped());
        // The block goes on after that pause too: the crawl stops.
        assert_eq!(throttle.first_forbidden(at(120_600), at(120_700)), OnForbidden::Stop);
        assert!(throttle.stopped());
        assert!(!throttle.wait());
        assert_eq!(throttle.first_forbidden(at(120_600), at(120_800)), OnForbidden::Stop);
    }

    #[test]
    fn only_a_network_success_after_the_403s_ends_the_episodes() {
        let throttle = Throttle::new(Duration::from_secs(60), 2);
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        assert_eq!(throttle.first_forbidden(at(0), at(100)), OnForbidden::Retry);
        // Served from the cache, or started before the last 403: no proof the block is over.
        throttle.success(at(60_000), false);
        throttle.success(at(50), true);
        assert_eq!(throttle.first_forbidden(at(60_100), at(60_200)), OnForbidden::Stop);

        let throttle = Throttle::new(Duration::from_secs(60), 2);
        assert_eq!(throttle.first_forbidden(at(0), at(100)), OnForbidden::Retry);
        // A download from the SEC after the pause: the next 403 is a new first episode.
        throttle.success(at(60_000), true);
        assert_eq!(throttle.first_forbidden(at(70_000), at(70_100)), OnForbidden::Retry);
        assert!(!throttle.stopped());
    }

    #[test]
    fn companies_file() {
        let dir = temp_dir("companies");
        let path = dir.join("c.txt");
        fs::write(&path, "AAPL\n# comment\nmsft, 320193\n\nNVDA # trailing\naapl\n").unwrap();
        let mut a = args(&["--form", "10-K", "--company", "BRK.B"]);
        a.companies_file = Some(path);
        assert_eq!(read_companies(&a).unwrap(), ["BRK.B", "AAPL", "msft", "320193", "NVDA"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sections_keep_filing_order() {
        let s = |id: &str| -> Section {
            serde_json::from_value(serde_json::json!({
                "id": id, "part": null, "item": "", "title": "", "heading": "",
                "start": 0, "end": 0, "text": format!("text of {id}"),
            }))
            .unwrap()
        };
        let list = [s("item_1"), s("item_1a"), s("item_10")];
        let json = serde_json::to_string(&Sections(&list)).unwrap();
        assert_eq!(json, r#"{"item_1":"text of item_1","item_1a":"text of item_1a","item_10":"text of item_10"}"#);
        let entry = serde_json::to_string(&SectionEntry(&list[1])).unwrap();
        assert_eq!(
            entry,
            r#"{"id":"item_1a","part":null,"item":"","title":"","heading":"","start":0,"end":0,"chars":15}"#
        );
    }
}
