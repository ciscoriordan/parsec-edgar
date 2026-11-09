//! `parsec-edgar crawl`: bulk download and parse filings into JSON.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use clap::Args;
use parsec_edgar::{
    Client, CompanyProfile, CrawlQuery, Error, Filing, FormKind, IndexEntry, Quarter, Section, extract_sections,
};
use serde::Serialize;
use serde::ser::SerializeMap;

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

/// One unit of work: a full-index row still to be resolved, or a known filing.
enum Job {
    Index(IndexEntry),
    Known(Filing),
}

impl Job {
    fn key(&self) -> (u64, &str, &str, &str) {
        match self {
            Job::Index(e) => (e.cik, &e.form, &e.filing_date, &e.accession_number),
            Job::Known(f) => (f.cik, &f.form, &f.filing_date, &f.accession_number),
        }
    }

    /// File name stem shared by the JSON output and the raw document.
    fn stem(&self) -> String {
        let (cik, form, date, acc) = self.key();
        let form: String = form.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        format!("{cik}_{form}_{date}_{acc}")
    }
}

/// Counters shared by the workers.
#[derive(Default)]
struct Stats {
    ok: AtomicUsize,
    failed: AtomicUsize,
    no_sections: AtomicUsize,
    /// 403 responses in a row; the SEC answers 403 when a client goes over its rate limit.
    forbidden_streak: AtomicUsize,
    stop: AtomicBool,
    bytes: AtomicU64,
}

type Failures = Mutex<Box<dyn Write + Send>>;

struct Output {
    out: PathBuf,
    jsonl: Option<Mutex<BufWriter<File>>>,
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

    // Discover.
    // Rewritten on each run, since failed filings are tried again. A dry run keeps it.
    let failures_path = args.out.join("failures.jsonl");
    let failures: Failures = Mutex::new(if args.dry_run {
        Box::new(std::io::sink())
    } else {
        Box::new(BufWriter::new(File::create(&failures_path)?))
    });
    let discovery_failed = AtomicUsize::new(0);
    let profiles: ProfileCache = Mutex::new(HashMap::new());
    let mut jobs: Vec<Job> = if companies.is_empty() {
        let mut jobs = Vec::new();
        for q in query.quarters_to_read()? {
            let rows = client.full_index(q)?;
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
        parallel(&companies, args.threads, |company| match crawl_company(client, company, &query) {
            Ok(filings) => {
                eprintln!("{company}: {} filings", filings.len());
                found.lock().unwrap().extend(filings.into_iter().map(Job::Known));
            }
            Err(e) => {
                eprintln!("{company}: {e}");
                discovery_failed.fetch_add(1, Ordering::Relaxed);
                write_line(&failures, &serde_json::json!({ "company": company, "error": e.to_string() }));
            }
        });
        let mut jobs = found.into_inner().unwrap();
        jobs.sort_by(|a, b| a.key().cmp(&b.key()));
        // A filing shared by two listed companies (co-registrants) is crawled once.
        let mut seen = HashSet::new();
        jobs.retain(|j| seen.insert(j.key().3.to_owned()));
        jobs
    };
    let discovery_requests = client.requests_sent();
    let discovery_time = started.elapsed();
    if let Some(n) = args.limit {
        jobs.truncate(n);
    }

    // Skip filings already done in an earlier run.
    let jsonl_path = args.out.join("filings.jsonl");
    let done: HashSet<String> = if args.jsonl { completed_jsonl(&jsonl_path)? } else { HashSet::new() };
    let found = jobs.len();
    jobs.retain(|j| {
        if args.jsonl {
            !done.contains(j.key().3)
        } else {
            !args.out.join(format!("{}.json", j.stem())).exists()
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
    if args.raw {
        fs::create_dir_all(args.out.join("raw"))?;
    }
    let output = Output {
        jsonl: if args.jsonl {
            Some(Mutex::new(BufWriter::new(OpenOptions::new().create(true).append(true).open(&jsonl_path)?)))
        } else {
            None
        },
        out: args.out.clone(),
        failures,
        raw: args.raw,
        items: args.items.iter().map(|i| crate::normalize_section_id(i)).collect(),
    };
    let stats = Stats::default();
    let pause_until: Mutex<Option<Instant>> = Mutex::new(None);
    let crawl_started = Instant::now();
    let total = jobs.len();
    let finished = AtomicBool::new(false);

    std::thread::scope(|s| {
        s.spawn(|| {
            let mut last = Instant::now();
            while !finished.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(200));
                if last.elapsed() >= Duration::from_secs(10) {
                    last = Instant::now();
                    eprintln!("{}", progress_line(client, &stats, total, crawl_started, discovery_requests));
                }
            }
        });
        parallel(&jobs, args.threads, |job| {
            if stats.stop.load(Ordering::Relaxed) {
                return;
            }
            wait_for_pause(&pause_until);
            let mut result = crawl_one(client, job, &output, &profiles, &stats);
            if is_forbidden(&result) {
                // Likely over the SEC rate limit: pause every worker, then try once more.
                let streak = stats.forbidden_streak.fetch_add(1, Ordering::Relaxed) + 1;
                if streak >= 5 {
                    stats.stop.store(true, Ordering::Relaxed);
                } else {
                    let mut p = pause_until.lock().unwrap();
                    let until = Instant::now() + Duration::from_secs(60);
                    *p = Some(p.map_or(until, |x| x.max(until)));
                    drop(p);
                    wait_for_pause(&pause_until);
                    result = crawl_one(client, job, &output, &profiles, &stats);
                }
            }
            match result {
                Ok(()) => {
                    stats.forbidden_streak.store(0, Ordering::Relaxed);
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
        });
        finished.store(true, Ordering::Relaxed);
    });
    if let Some(j) = &output.jsonl {
        j.lock().unwrap().flush()?;
    }
    output.failures.lock().unwrap().flush()?;

    let ok = stats.ok.load(Ordering::Relaxed);
    let failed = stats.failed.load(Ordering::Relaxed) + discovery_failed.load(Ordering::Relaxed);
    let elapsed = crawl_started.elapsed().as_secs_f64();
    let crawl_requests = client.requests_sent() - discovery_requests;
    let done_now = ok + stats.failed.load(Ordering::Relaxed);
    eprintln!(
        "done in {:.1}s: ok {ok}, failed {failed}, skipped {skipped}; {} with no sections; \
         {:.1} filings/min, {:.2} requests/filing ({crawl_requests} for filings, {discovery_requests} for discovery), \
         {:.1} MB of documents",
        started.elapsed().as_secs_f64(),
        stats.no_sections.load(Ordering::Relaxed),
        if elapsed > 0.0 { ok as f64 * 60.0 / elapsed } else { 0.0 },
        if done_now > 0 { crawl_requests as f64 / done_now as f64 } else { 0.0 },
        stats.bytes.load(Ordering::Relaxed) as f64 / 1e6,
    );
    if stats.stop.load(Ordering::Relaxed) {
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

type ProfileCache = Mutex<HashMap<u64, Arc<OnceLock<Option<CompanyProfile>>>>>;

fn crawl_company(client: &Client, company: &str, query: &CrawlQuery) -> Result<Vec<Filing>, Error> {
    let cik = match parse_cik(company) {
        Some(cik) => cik,
        None => client.company(company)?.cik,
    };
    client.crawl_company(&cik.to_string(), query)
}

/// Fetch, parse and write one filing. On failure, returns the URL involved and the error.
fn crawl_one(
    client: &Client,
    job: &Job,
    output: &Output,
    profiles: &ProfileCache,
    stats: &Stats,
) -> Result<(), (String, Error)> {
    let (filing, profile) = match job {
        Job::Index(entry) => {
            let (filing, index) = client.resolve_index_entry(entry).map_err(|e| (entry.index_url(), e))?;
            let profile = index.filer(entry.cik).map(|f| f.profile.clone());
            (filing, profile)
        }
        Job::Known(filing) => {
            // One submissions request per company, shared by all its filings.
            let cell = profiles.lock().unwrap().entry(filing.cik).or_default().clone();
            let profile = cell.get_or_init(|| client.company_profile(filing.cik).ok()).clone();
            (filing.clone(), profile)
        }
    };
    let doc = client.fetch_primary_document(&filing).map_err(|e| (filing_url(&filing), e))?;
    stats.bytes.fetch_add(doc.bytes.len() as u64, Ordering::Relaxed);
    let text = doc.text();
    let kind = FormKind::from_form(&filing.form);
    let mut sections = kind.map(|k| extract_sections(&text, k)).unwrap_or_default();
    if kind.is_some() && sections.is_empty() {
        stats.no_sections.fetch_add(1, Ordering::Relaxed);
    }
    if !output.items.is_empty() {
        sections.retain(|s| output.items.contains(&s.id));
    }

    let stem = job.stem();
    let io = |e: std::io::Error| (output.out.display().to_string(), Error::Io(e));
    let raw_file = if output.raw {
        let name = format!("raw/{stem}.{}", doc.extension());
        write_atomic(&output.out.join(&name), &doc.bytes).map_err(io)?;
        Some(name)
    } else {
        None
    };
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
        sections: Sections(&sections),
        text: if kind.is_none() { Some(text.as_str()) } else { None },
    };
    match &output.jsonl {
        Some(jsonl) => {
            let line = serde_json::to_string(&record).expect("records serialize");
            let mut w = jsonl.lock().unwrap();
            writeln!(w, "{line}").and_then(|_| w.flush()).map_err(io)?;
        }
        None => {
            let json = serde_json::to_vec_pretty(&record).expect("records serialize");
            write_atomic(&output.out.join(format!("{stem}.json")), &json).map_err(io)?;
        }
    }
    Ok(())
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
    /// Full text, for forms without Item sections.
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

/// Accession numbers of complete records in a JSON lines file. A partial last
/// line, left by an interrupted run, is cut off so appends start on a new line.
fn completed_jsonl(path: &Path) -> Result<HashSet<String>, Error> {
    let mut done = HashSet::new();
    let Ok(file) = File::open(path) else { return Ok(done) };
    let len = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut complete_end = 0u64;
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 || line.last() != Some(&b'\n') {
            break;
        }
        complete_end += n as u64;
        if let Some(acc) = accession_in(&line) {
            done.insert(acc);
        }
    }
    if complete_end < len {
        OpenOptions::new().write(true).open(path)?.set_len(complete_end)?;
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

fn wait_for_pause(pause_until: &Mutex<Option<Instant>>) {
    let until = *pause_until.lock().unwrap();
    if let Some(t) = until {
        let now = Instant::now();
        if t > now {
            std::thread::sleep(t - now);
        }
    }
}

fn is_forbidden(r: &Result<(), (String, Error)>) -> bool {
    matches!(r, Err((_, Error::Status { status: 403, .. })))
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
    let mut w = w.lock().unwrap();
    let _ = writeln!(w, "{value}").and_then(|_| w.flush());
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
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
        let dir = std::env::temp_dir().join(format!("parsec-crawl-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("filings.jsonl");
        let full = r#"{"cik":1,"company":"A","form":"8-K","filing_date":"2024-01-02","report_date":null,"acceptance_datetime":null,"accession_number":"0000000001-24-000001","sections":{}}"#;
        fs::write(&path, format!("{full}\n{{\"cik\":2,\"accession_number\":\"0000000002-24-0")).unwrap();
        let done = completed_jsonl(&path).unwrap();
        assert_eq!(done.len(), 1);
        assert!(done.contains("0000000001-24-000001"));
        assert_eq!(fs::read_to_string(&path).unwrap(), format!("{full}\n"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn companies_file() {
        let dir = std::env::temp_dir().join(format!("parsec-crawl-companies-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
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
