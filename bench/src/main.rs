//! Accuracy benchmark for Item section extraction. See bench/README.md.

mod align;
mod labels;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Parser, Subcommand};
use parsec_edgar::{Client, FormKind, document_to_text, extract_sections};
use regex::Regex;
use serde::Serialize;

use labels::{Doc, ItemLabel, LabelFile};

#[derive(Parser)]
#[command(name = "parsec-edgar-bench", about = "Score Item section extraction against hand labels")]
struct Cli {
    /// Directory of label files, one JSON file per filing.
    #[arg(long, global = true, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/labels"))]
    labels: PathBuf,
    /// Name and contact email sent to the SEC.
    #[arg(long, global = true, env = "PARSEC_EDGAR_USER_AGENT")]
    user_agent: Option<String>,
    /// Cache directory for EDGAR archive documents [default: ~/.cache/parsec-edgar]
    #[arg(long, global = true, env = "PARSEC_EDGAR_CACHE")]
    cache_dir: Option<PathBuf>,
    /// Requests per second to SEC servers.
    #[arg(long, global = true, default_value_t = 3.0)]
    rps: f64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Score the extractor on every labeled filing.
    Score {
        /// Only filings whose accession, company or form contains this text.
        #[arg(long)]
        filter: Option<String>,
        /// Parse each filing this many times and keep the fastest time.
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        /// Write per-filing results as JSON.
        #[arg(long)]
        json: Option<PathBuf>,
        /// Leave out the per-filing list of misses.
        #[arg(long)]
        no_misses: bool,
    },
    /// Score another tool's output: a directory of `<accession>.json` files mapping section ids to text.
    ScoreExternal {
        /// Directory with the tool's output.
        dir: PathBuf,
        /// Tool name for the report.
        #[arg(long, default_value = "external")]
        name: String,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        json: Option<PathBuf>,
        #[arg(long)]
        no_misses: bool,
    },
    /// Create a label file with filing metadata from the SEC (no Items yet).
    Add {
        /// Ticker or CIK.
        company: String,
        accession: String,
        /// Tags such as plain-text, cross-reference, small-cap, bank, insurer.
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Filing agent, e.g. self, DFIN, Workiva, Toppan Merrill.
        #[arg(long, default_value = "")]
        agent: String,
    },
    /// Labeling aid: every Item-like line of a filing with its context.
    Items { accession: String },
    /// Labeling aid: print text lines with line numbers.
    Lines {
        accession: String,
        #[arg(long, default_value_t = 0)]
        from: usize,
        #[arg(long)]
        to: Option<usize>,
        /// Only lines matching this regex.
        #[arg(long)]
        grep: Option<String>,
        /// Truncate lines to this many characters.
        #[arg(long, default_value_t = 160)]
        width: usize,
    },
    /// Write Item labels from line numbers chosen by hand: `1A:812`, `I2:300`, `2.02:40`, `15:900-1200` (heading-end).
    Set {
        accession: String,
        /// Item:line pairs.
        items: Vec<String>,
        /// Line where the last Item ends (signatures, exhibit index, attached report).
        #[arg(long)]
        end: Option<usize>,
        /// Items whose body only incorporates the answer by reference or says "not applicable".
        #[arg(long, value_delimiter = ',')]
        incorporated: Vec<String>,
        /// Items with no body heading at all.
        #[arg(long, value_delimiter = ',')]
        absent: Vec<String>,
        /// Items whose label is uncertain.
        #[arg(long, value_delimiter = ',')]
        uncertain: Vec<String>,
        #[arg(long)]
        note: Option<String>,
    },
    /// Write this extractor's sections in the score-external format. Scoring them
    /// with score-external checks that the text placement there is faithful.
    Predict { dir: PathBuf },
    /// Write each labeled filing's raw document to a directory, for running other tools.
    Export {
        dir: PathBuf,
        /// Also write the complete SGML submission as `<accession>.full.txt` (all
        /// documents, as edgartools reads a filing).
        #[arg(long)]
        full_submission: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

type Res<T> = Result<T, String>;

fn client(cli: &Cli) -> Res<Client> {
    let ua = cli
        .user_agent
        .clone()
        .ok_or("set --user-agent or PARSEC_EDGAR_USER_AGENT to your name and contact email")?;
    let cache = cli.cache_dir.clone().or_else(|| {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache").join("parsec-edgar"))
    });
    Client::builder(ua).requests_per_second(cli.rps).cache_dir(cache).build().map_err(|e| e.to_string())
}

fn run(cli: Cli) -> Res<()> {
    match &cli.command {
        Command::Score { filter, repeat, json, no_misses } => {
            let client = client(&cli)?;
            let files = labels::load_all(&cli.labels, filter.as_deref())?;
            let results = score_all(&client, &files, (*repeat).max(1));
            report(&results, "parsec-edgar", !no_misses, true);
            if let Some(path) = json {
                write_json(path, &results)?;
            }
        }
        Command::ScoreExternal { dir, name, filter, json, no_misses } => {
            let client = client(&cli)?;
            let files = labels::load_all(&cli.labels, filter.as_deref())?;
            let results = score_external(&client, &files, dir);
            report(&results, name, !no_misses, false);
            if let Some(path) = json {
                write_json(path, &results)?;
            }
        }
        Command::Add { company, accession, tags, agent } => {
            let client = client(&cli)?;
            let f = client.filing(company, accession).map_err(|e| e.to_string())?;
            let path = cli.labels.join(format!("{}.json", f.accession_number));
            if path.exists() {
                return Err(format!("{} exists", path.display()));
            }
            // Fetch now so later runs read the cache. Some old filings list a
            // primary document that is not served (404); the full submission is.
            // Any other failure is an error, so a passing outage cannot change
            // which document a filing is scored on.
            let fetched = f.primary_document_url.clone().map(|u| (client.get_text(&u), u));
            let (raw, url) = match fetched {
                Some((Ok(raw), u)) => (raw, u),
                Some((Err(e), _)) if !not_served(&e) => return Err(e.to_string()),
                _ => {
                    let u = f.full_submission_url();
                    (client.get_text(&u).map_err(|e| e.to_string())?, u)
                }
            };
            let label = LabelFile {
                accession: f.accession_number.clone(),
                cik: f.cik,
                company: f.company.clone(),
                form: f.form.clone(),
                filing_date: f.filing_date.clone(),
                url,
                agent: agent.clone(),
                tags: tags.clone(),
                ..LabelFile::default()
            };
            labels::save(&path, &label)?;
            println!("{} {} {} {} ({} bytes)", path.display(), label.company, label.form, label.filing_date, raw.len());
        }
        Command::Items { accession } => {
            let client = client(&cli)?;
            let (_, label) = labels::find(&cli.labels, accession)?;
            let doc = load_doc(&client, &label)?;
            print_items(&doc, &label);
        }
        Command::Lines { accession, from, to, grep, width } => {
            let client = client(&cli)?;
            let (_, label) = labels::find(&cli.labels, accession)?;
            let doc = load_doc(&client, &label)?;
            let re = grep.as_deref().map(Regex::new).transpose().map_err(|e| e.to_string())?;
            let to = to.unwrap_or(doc.lines()).min(doc.lines());
            for i in *from..to {
                let line = doc.line(i);
                if re.as_ref().is_none_or(|r| r.is_match(line)) {
                    println!("{i:>6} #{} {}", doc.occurrence(i), trunc(line, *width));
                }
            }
        }
        Command::Set { accession, items, end, incorporated, absent, uncertain, note } => {
            let client = client(&cli)?;
            let (path, mut label) = labels::find(&cli.labels, accession)?;
            let doc = load_doc(&client, &label)?;
            let kind = FormKind::from_form(&label.form).ok_or("form has no Item sections")?;
            let norm_ids = |v: &[String]| -> Res<Vec<String>> { v.iter().map(|s| labels::item_id(kind, s)).collect() };
            let incorporated = norm_ids(incorporated)?;
            let uncertain = norm_ids(uncertain)?;
            let mut out = Vec::new();
            for spec in items {
                let (id, rest) = spec.split_once(':').ok_or_else(|| format!("expected ITEM:LINE, got {spec}"))?;
                let id = labels::item_id(kind, id)?;
                let (start, item_end) = match rest.split_once('-') {
                    Some((a, b)) => (parse_line(a)?, Some(parse_line(b)?)),
                    None => (parse_line(rest)?, None),
                };
                out.push(ItemLabel {
                    heading: doc.line_ref(start)?,
                    end: item_end.map(|e| doc.line_ref(e)).transpose()?,
                    incorporated: incorporated.contains(&id),
                    uncertain: uncertain.contains(&id),
                    id,
                    note: String::new(),
                });
            }
            label.items = out;
            label.end = end.map(|e| doc.line_ref(e)).transpose()?;
            label.absent = norm_ids(absent)?;
            if let Some(n) = note {
                label.notes = n.clone();
            }
            label.labeled = true;
            // Save only labels that resolve back to lines of the filing.
            let truth = labels::resolve(&doc, &label)?;
            labels::save(&path, &label)?;
            for t in &truth.items {
                println!("{:<16} {:>6} {}", t.id, t.line, trunc(doc.line(t.line), 90));
            }
            if let Some(e) = truth.end_pos.map(|p| doc.line_of(p)) {
                println!("{:<16} {:>6} {}", "end", e, trunc(doc.line(e), 90));
            }
        }
        Command::Predict { dir } => {
            let client = client(&cli)?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let files = labels::load_all(&cli.labels, None)?;
            for (_, l) in &files {
                let doc = load_doc(&client, l)?;
                let kind = FormKind::from_form(&l.form).ok_or("form has no Item sections")?;
                let out: HashMap<String, String> =
                    extract_sections(&doc.text, kind).into_iter().map(|s| (s.id, s.text)).collect();
                write_json(&dir.join(format!("{}.json", l.accession)), &out)?;
            }
            println!("wrote {} filings to {}", files.len(), dir.display());
        }
        Command::Export { dir, full_submission } => {
            let client = client(&cli)?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let files = labels::load_all(&cli.labels, None)?;
            let mut manifest = Vec::new();
            for (_, l) in &files {
                let raw = client.get_text(&l.url).map_err(|e| e.to_string())?;
                let ext = if l.url.ends_with(".txt") { "txt" } else { "htm" };
                let name = format!("{}.{ext}", l.accession);
                std::fs::write(dir.join(&name), &raw).map_err(|e| e.to_string())?;
                let mut entry = serde_json::json!({
                    "accession": l.accession, "cik": l.cik, "company": l.company, "form": l.form,
                    "filing_date": l.filing_date, "url": l.url, "file": name,
                });
                if *full_submission {
                    let url = format!(
                        "https://www.sec.gov/Archives/edgar/data/{}/{}/{}.txt",
                        l.cik,
                        l.accession.replace('-', ""),
                        l.accession
                    );
                    let bytes = client.get_bytes(&url).map_err(|e| e.to_string())?;
                    let full = format!("{}.full.txt", l.accession);
                    std::fs::write(dir.join(&full), &bytes).map_err(|e| e.to_string())?;
                    entry["full_submission"] = full.into();
                }
                manifest.push(entry);
            }
            write_json(&dir.join("manifest.json"), &manifest)?;
            println!("wrote {} filings to {}", manifest.len(), dir.display());
        }
    }
    Ok(())
}

/// Whether a failed fetch means the document is not served at all.
fn not_served(e: &parsec_edgar::Error) -> bool {
    matches!(e, parsec_edgar::Error::Status { status: 404, .. })
}

fn parse_line(s: &str) -> Res<usize> {
    s.trim().trim_start_matches('L').parse().map_err(|_| format!("not a line number: {s}"))
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Res<()> {
    let s = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, s + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

fn load_doc(client: &Client, label: &LabelFile) -> Res<Doc> {
    let raw = client.get_text(&label.url).map_err(|e| e.to_string())?;
    Ok(Doc::new(document_to_text(&raw)))
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_owned() } else { s.chars().take(n).collect::<String>() + "…" }
}

/// Lines that matter when labeling: Item-like lines, Part lines, signatures and exhibit indexes.
fn print_items(doc: &Doc, label: &LabelFile) {
    static ITEMISH: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?i)^\W{0,3}(?:part\s+[ivx1-4]+\W{0,6})?items?\s*\d{1,2}(?:\.\d{1,2})?[a-c]?\b").unwrap()
    });
    static OTHER: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?i)^(?:\W{0,3}part\s+[ivx]+\b.{0,60}$|signatures?\W*$|exhibit\s+index|index\s+to\s+exhibits|.*cross[- ]reference)").unwrap()
    });
    println!("{} {} {} {} lines, {} bytes", label.company, label.form, label.filing_date, doc.lines(), doc.text.len());
    let marks: Vec<(usize, bool)> = (0..doc.lines())
        .filter_map(|i| {
            let l = doc.line(i);
            if ITEMISH.is_match(l) {
                Some((i, true))
            } else if OTHER.is_match(l) {
                Some((i, false))
            } else {
                None
            }
        })
        .collect();
    for (n, &(i, item)) in marks.iter().enumerate() {
        let next_item = marks[n + 1..].iter().find(|m| m.1).map_or(doc.text.len(), |m| doc.start(m.0));
        let gap = next_item - doc.start(i);
        let next = if i + 1 < doc.lines() { doc.line(i + 1) } else { "" };
        println!(
            "{}{i:>6} #{} {gap:>8} | {} | > {}",
            if item { ' ' } else { '*' },
            doc.occurrence(i),
            trunc(doc.line(i), 72),
            trunc(next, 44)
        );
    }
}

// ---------- scoring ----------

#[derive(Serialize, Default, Clone)]
struct ItemResult {
    id: String,
    /// hit, missed (labeled, not predicted), wrong (predicted at another line), extra (not labeled).
    status: String,
    true_line: Option<usize>,
    pred_line: Option<usize>,
    true_chars: usize,
    pred_chars: usize,
    overlap_chars: usize,
}

#[derive(Serialize, Default, Clone)]
struct FilingResult {
    accession: String,
    company: String,
    form: String,
    filing_date: String,
    group: String,
    tags: Vec<String>,
    labeled: usize,
    predicted: usize,
    hits: usize,
    span_correct: usize,
    span_union: usize,
    parse_ms: f64,
    sections_ms: f64,
    text_bytes: usize,
    items: Vec<ItemResult>,
    error: Option<String>,
}

impl FilingResult {
    /// A filing that could not be scored (fetch failure, labels that do not
    /// resolve) scores 0, so an error can never raise the totals.
    fn span_accuracy(&self) -> f64 {
        if self.error.is_some() {
            0.0
        } else if self.span_union == 0 {
            1.0
        } else {
            self.span_correct as f64 / self.span_union as f64
        }
    }
}

fn group_of(label: &LabelFile) -> String {
    match FormKind::from_form(&label.form) {
        Some(FormKind::TenK) => {
            let y: u32 = label.filing_date.get(..4).and_then(|y| y.parse().ok()).unwrap_or(0);
            let era = match y {
                0..=2001 => "1993-2001",
                2002..=2008 => "2002-2008",
                2009..=2015 => "2009-2015",
                2016..=2020 => "2016-2020",
                _ => "2021-2026",
            };
            format!("10-K {era}")
        }
        Some(FormKind::TenQ) => "10-Q".into(),
        Some(FormKind::EightK) => "8-K".into(),
        None => label.form.clone(),
    }
}

/// A predicted section as byte offsets in the filing text.
struct Pred {
    id: String,
    start: usize,
    end: usize,
}

fn score_all(client: &Client, files: &[(PathBuf, LabelFile)], repeat: usize) -> Vec<FilingResult> {
    let mut out = Vec::new();
    for (_, label) in files {
        let mut r = base_result(label);
        let raw = match client.get_text(&label.url) {
            Ok(raw) => raw,
            Err(e) => {
                r.error = Some(e.to_string());
                out.push(r);
                continue;
            }
        };
        let kind = FormKind::from_form(&label.form).expect("labeled forms have sections");
        let mut best_parse = f64::INFINITY;
        let mut best_sections = f64::INFINITY;
        let mut text = String::new();
        let mut sections = Vec::new();
        for _ in 0..repeat {
            let t0 = Instant::now();
            text = document_to_text(&raw);
            let t1 = Instant::now();
            sections = extract_sections(&text, kind);
            let t2 = Instant::now();
            best_parse = best_parse.min((t1 - t0).as_secs_f64() * 1000.0);
            best_sections = best_sections.min((t2 - t1).as_secs_f64() * 1000.0);
        }
        r.parse_ms = best_parse;
        r.sections_ms = best_sections;
        let preds: Vec<Pred> = sections.iter().map(|s| Pred { id: s.id.clone(), start: s.start, end: s.end }).collect();
        let doc = Doc::new(text);
        score_one(&doc, label, &preds, &mut r);
        out.push(r);
    }
    out
}

fn score_external(client: &Client, files: &[(PathBuf, LabelFile)], dir: &Path) -> Vec<FilingResult> {
    let mut out = Vec::new();
    for (_, label) in files {
        let mut r = base_result(label);
        let path = dir.join(format!("{}.json", label.accession));
        let sections: HashMap<String, String> = match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(s) => s,
            Err(e) => {
                r.error = Some(format!("{}: {e}", path.display()));
                out.push(r);
                continue;
            }
        };
        let doc = match load_doc(client, label) {
            Ok(d) => d,
            Err(e) => {
                r.error = Some(e);
                out.push(r);
                continue;
            }
        };
        let aligner = align::Aligner::new(&doc.text);
        let truth: HashMap<String, labels::TrueItem> = labels::resolve(&doc, label)
            .map(|t| t.items.into_iter().map(|i| (i.id.clone(), i)).collect())
            .unwrap_or_default();
        let mut preds = Vec::new();
        let mut unplaced = Vec::new();
        for (id, text) in &sections {
            let t = truth.get(id);
            match aligner.place(text) {
                Some((start, end)) => {
                    let mut start = doc.line_start_of(start);
                    // A tool that leaves the heading out of the Item's text
                    // starts it on the line after the heading.
                    if let Some(t) = t
                        && !align::starts_with_item(text)
                        && heading_just_above(&doc, t, doc.line_of(start))
                    {
                        start = t.pos;
                    }
                    preds.push(Pred { id: id.clone(), start, end });
                }
                None if text.trim().is_empty() => {}
                // Too short to find in the filing ("None."): the labeled
                // Item's body, when the two are the same text.
                None => match t {
                    Some(t) if same_body(&doc, t, text) => {
                        preds.push(Pred { id: id.clone(), start: t.pos, end: t.end })
                    }
                    _ => unplaced.push(id.clone()),
                },
            }
        }
        preds.sort_by_key(|p| p.start);
        score_one(&doc, label, &preds, &mut r);
        for id in unplaced {
            // Output we could not find in the filing text counts as a wrong prediction.
            r.predicted += 1;
            r.items.push(ItemResult { id, status: "unplaced".into(), ..ItemResult::default() });
        }
        out.push(r);
    }
    out
}

/// Whether the labeled heading is just above `line`, with at most one short
/// line (a title) between them.
fn heading_just_above(doc: &Doc, t: &labels::TrueItem, line: usize) -> bool {
    line > t.line && line - t.line <= 2 && (t.line + 1..line).all(|l| doc.line(l).chars().count() <= 100)
}

/// Whether a tool's text is the labeled Item without its heading line, give
/// or take a short tail such as a following PART line.
fn same_body(doc: &Doc, t: &labels::TrueItem, text: &str) -> bool {
    let body_start = doc.text[t.pos..t.end].find('\n').map_or(t.end, |i| t.pos + i);
    let body = align::normalize(&doc.text[body_start..t.end]);
    let text = align::normalize(text);
    !text.is_empty() && body.starts_with(&text) && body.len() - text.len() <= 40
}

/// A filing's result before scoring. Its labeled Items count as missed
/// until scoring finds them, so a filing that errors still counts.
fn base_result(label: &LabelFile) -> FilingResult {
    FilingResult {
        accession: label.accession.clone(),
        company: label.company.clone(),
        form: label.form.clone(),
        filing_date: label.filing_date.clone(),
        group: group_of(label),
        tags: label.tags.clone(),
        labeled: label.items.len(),
        ..FilingResult::default()
    }
}

fn score_one(doc: &Doc, label: &LabelFile, preds: &[Pred], r: &mut FilingResult) {
    r.text_bytes = doc.text.len();
    let truth = match labels::resolve(doc, label) {
        Ok(t) => t,
        Err(e) => {
            // Every labeled Item counts as missed and every prediction as wrong.
            r.error = Some(format!("labels do not resolve: {e}"));
            r.predicted = preds.len();
            return;
        }
    };
    r.labeled = truth.items.len();
    r.predicted = preds.len();
    let true_spans: HashMap<&str, (usize, usize, usize)> =
        truth.items.iter().map(|t| (t.id.as_str(), (t.line, t.pos, t.end))).collect();
    let mut intervals: Vec<(usize, usize)> = Vec::new();
    let mut seen = Vec::new();
    for p in preds {
        let pred_line = doc.line_of(p.start);
        let pc = (doc.char_of(p.start), doc.char_of(p.end.max(p.start)));
        intervals.push(pc);
        let mut ir = ItemResult {
            id: p.id.clone(),
            pred_line: Some(pred_line),
            pred_chars: pc.1 - pc.0,
            ..ItemResult::default()
        };
        match true_spans.get(p.id.as_str()) {
            Some(&(line, s, e)) => {
                seen.push(p.id.clone());
                let tc = (doc.char_of(s), doc.char_of(e));
                ir.true_line = Some(line);
                ir.true_chars = tc.1 - tc.0;
                ir.overlap_chars = tc.1.min(pc.1).saturating_sub(tc.0.max(pc.0));
                r.span_correct += ir.overlap_chars;
                if line == pred_line {
                    r.hits += 1;
                    ir.status = "hit".into();
                } else {
                    ir.status = "wrong".into();
                }
            }
            None => ir.status = "extra".into(),
        }
        r.items.push(ir);
    }
    for t in &truth.items {
        let tc = (doc.char_of(t.pos), doc.char_of(t.end));
        intervals.push(tc);
        if !seen.contains(&t.id) {
            r.items.push(ItemResult {
                id: t.id.clone(),
                status: "missed".into(),
                true_line: Some(t.line),
                true_chars: tc.1 - tc.0,
                ..ItemResult::default()
            });
        }
    }
    r.span_union = union_len(&mut intervals);
}

fn union_len(v: &mut [(usize, usize)]) -> usize {
    v.sort_unstable();
    let mut total = 0;
    let mut cur: Option<(usize, usize)> = None;
    for &(s, e) in v.iter() {
        match cur {
            Some((cs, ce)) if s <= ce => cur = Some((cs, ce.max(e))),
            Some((cs, ce)) => {
                total += ce - cs;
                cur = Some((s, e));
            }
            None => cur = Some((s, e)),
        }
    }
    total + cur.map_or(0, |(s, e)| e - s)
}

#[derive(Default)]
struct Agg {
    filings: usize,
    labeled: usize,
    predicted: usize,
    hits: usize,
    span_sum: f64,
    parse_ms: Vec<f64>,
    sections_ms: Vec<f64>,
}

impl Agg {
    fn add(&mut self, r: &FilingResult) {
        self.filings += 1;
        self.labeled += r.labeled;
        self.predicted += r.predicted;
        self.hits += r.hits;
        self.span_sum += r.span_accuracy();
        self.parse_ms.push(r.parse_ms);
        self.sections_ms.push(r.sections_ms);
    }

    fn row(&self, name: &str, timing: bool) -> String {
        let p = ratio(self.hits, self.predicted);
        let rc = ratio(self.hits, self.labeled);
        let f1 = if p + rc > 0.0 { 2.0 * p * rc / (p + rc) } else { 0.0 };
        let span = if self.filings > 0 { self.span_sum / self.filings as f64 } else { 0.0 };
        let mut s = format!(
            "| {name:<20} | {:>7} | {:>7} | {:>9} | {:>4} | {:>9.3} | {:>6.3} | {:>5.3} | {:>8.3} |",
            self.filings, self.labeled, self.predicted, self.hits, p, rc, f1, span
        );
        if timing {
            s += &format!(
                " {:>9.1} | {:>9.2} | {:>8.0} |",
                median(&self.parse_ms),
                median(&self.sections_ms),
                self.parse_ms.iter().sum::<f64>() + self.sections_ms.iter().sum::<f64>()
            );
        }
        s
    }
}

fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 }
}

fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

fn report(results: &[FilingResult], name: &str, misses: bool, timing: bool) {
    let mut groups: Vec<String> = results.iter().map(|r| r.group.clone()).collect();
    groups.sort();
    groups.dedup();
    let mut tags: Vec<String> = results.iter().flat_map(|r| r.tags.clone()).collect();
    tags.sort();
    tags.dedup();

    println!("{name}: Item start precision, recall and F1, span accuracy\n");
    let mut header =
        "| Group                | Filings | Labeled | Predicted | Hits | Precision | Recall |    F1 | Span acc |".to_owned();
    let mut rule = "|----------------------|--------:|--------:|----------:|-----:|----------:|-------:|------:|---------:|".to_owned();
    if timing {
        header += " Parse ms¹ | Split ms¹ | Total ms |";
        rule += "----------:|----------:|---------:|";
    }
    println!("{header}\n{rule}");
    // Filings with errors stay in every row: their labeled Items count as
    // missed and their span as 0.
    for g in &groups {
        let mut a = Agg::default();
        results.iter().filter(|r| &r.group == g).for_each(|r| a.add(r));
        println!("{}", a.row(g, timing));
    }
    let mut ten_k = Agg::default();
    results.iter().filter(|r| r.group.starts_with("10-K")).for_each(|r| ten_k.add(r));
    println!("{}", ten_k.row("10-K all eras", timing));
    let mut all = Agg::default();
    results.iter().for_each(|r| all.add(r));
    println!("{}", all.row("All", timing));
    if !tags.is_empty() {
        println!("\nBy tag (a filing can have several):\n\n{header}\n{rule}");
        for t in &tags {
            let mut a = Agg::default();
            results.iter().filter(|r| r.tags.contains(t)).for_each(|r| a.add(r));
            println!("{}", a.row(t, timing));
        }
    }
    if timing {
        println!("\n¹ median per filing; Parse is document_to_text, Split is extract_sections.");
    }
    println!("Span acc is the mean over filings of (characters given the right Item) / (characters inside any true or predicted Item).");

    let errors: Vec<&FilingResult> = results.iter().filter(|r| r.error.is_some()).collect();
    if !errors.is_empty() {
        println!("\nErrors (scored as all Items missed, span 0):");
        for r in errors {
            println!("  {} {} {}: {}", r.accession, r.company, r.form, r.error.as_deref().unwrap_or(""));
        }
    }
    if misses {
        println!("\nPer-filing misses:");
        for r in results.iter().filter(|r| r.error.is_none()) {
            let bad: Vec<&ItemResult> = r
                .items
                .iter()
                .filter(|i| {
                    i.status != "hit"
                        || (i.true_chars.max(i.pred_chars) > 2000
                            && (i.overlap_chars as f64) < 0.9 * i.true_chars.max(i.pred_chars) as f64)
                })
                .collect();
            if bad.is_empty() {
                continue;
            }
            println!(
                "\n  {} {} {} {}: {}/{} hits, {} predicted, span {:.3}",
                r.company,
                r.form,
                r.filing_date,
                r.accession,
                r.hits,
                r.labeled,
                r.predicted,
                r.span_accuracy()
            );
            for i in bad {
                let line = |l: Option<usize>| l.map_or("-".to_owned(), |l| l.to_string());
                println!(
                    "    {:<16} {:<8} true line {:>6}, predicted {:>6}, chars {} true / {} predicted / {} shared",
                    i.id,
                    i.status,
                    line(i.true_line),
                    line(i.pred_line),
                    i.true_chars,
                    i.pred_chars,
                    i.overlap_chars
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errored_filings_count_as_missed() {
        let label = LabelFile {
            items: vec![ItemLabel::default(), ItemLabel::default()],
            filing_date: "2024-01-01".into(),
            form: "10-Q".into(),
            ..LabelFile::default()
        };
        let good = FilingResult { labeled: 2, predicted: 2, hits: 2, span_correct: 10, span_union: 10, ..base_result(&label) };
        let bad = FilingResult { error: Some("labels do not resolve".into()), ..base_result(&label) };
        assert_eq!(bad.labeled, 2);
        assert_eq!(bad.span_accuracy(), 0.0);
        let mut a = Agg::default();
        a.add(&good);
        a.add(&bad);
        assert_eq!((a.filings, a.labeled, a.predicted, a.hits), (2, 4, 2, 2));
        assert_eq!(a.span_sum, 1.0);
    }

    #[test]
    fn only_a_missing_document_falls_back_to_the_full_submission() {
        let status = |status| parsec_edgar::Error::Status { status, url: String::new() };
        assert!(not_served(&status(404)));
        assert!(!not_served(&status(403)));
        assert!(!not_served(&status(429)));
        assert!(!not_served(&status(503)));
        assert!(!not_served(&parsec_edgar::Error::Network { url: String::new(), message: "timeout".into() }));
    }
}
