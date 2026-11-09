use std::path::PathBuf;
use std::process::ExitCode;

mod crawl;

use clap::{Args, Parser, Subcommand};
use parsec_edgar::{Client, Error, Filing, FilingQuery};

/// Fast SEC EDGAR client: companies, filings, clean text and Item sections as JSON.
#[derive(Parser)]
#[command(name = "parsec-edgar", version)]
struct Cli {
    /// Name and contact email sent to the SEC with every request, as the SEC requires,
    /// e.g. "Example Corp admin@example.com".
    #[arg(long, global = true, env = "PARSEC_EDGAR_USER_AGENT")]
    user_agent: Option<String>,

    /// Cache directory for EDGAR archive documents [default: ~/.cache/parsec-edgar]
    #[arg(long, global = true, env = "PARSEC_EDGAR_CACHE")]
    cache_dir: Option<PathBuf>,

    /// Do not read or write the cache.
    #[arg(long, global = true)]
    no_cache: bool,

    /// Requests per second to SEC servers (the SEC allows at most 10).
    #[arg(long, global = true, default_value_t = 8.0)]
    rps: f64,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Look up a company by ticker or CIK.
    Company { query: String },

    /// List a company's filings, newest first.
    Filings {
        /// Ticker or CIK.
        company: String,
        /// Form type to keep, e.g. 10-K. Repeat for several.
        #[arg(long = "form")]
        forms: Vec<String>,
        /// Earliest filing date (YYYY, YYYY-MM or YYYY-MM-DD).
        #[arg(long)]
        since: Option<String>,
        /// Latest filing date (YYYY, YYYY-MM or YYYY-MM-DD).
        #[arg(long)]
        until: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        /// Read the company's whole filing history, not just the recent block.
        #[arg(long)]
        all_history: bool,
    },

    /// Print the clean text of a filing's primary document.
    Text {
        company: String,
        #[command(flatten)]
        which: Which,
    },

    /// Item sections of a 10-K, 10-Q or 8-K as JSON.
    Sections {
        company: String,
        #[command(flatten)]
        which: Which,
        /// Only these sections, e.g. 1A, item_7, part_2_item_1a, 2.02. Repeat for several.
        #[arg(long = "item")]
        items: Vec<String>,
        /// Leave out section text (ids, titles, headings and offsets only).
        #[arg(long)]
        no_text: bool,
    },

    /// Bulk download filings by form and date (all companies, or a list) into JSON files.
    Crawl(crawl::CrawlArgs),
}

#[derive(Args)]
struct Which {
    /// Accession number, e.g. 0000320193-24-000123.
    #[arg(long, conflicts_with = "form")]
    accession: Option<String>,
    /// Use the latest filing of this form type [default: 10-K].
    #[arg(long)]
    form: Option<String>,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    let user_agent = cli.user_agent.or_else(|| std::env::var("EDGAR_IDENTITY").ok()).ok_or_else(|| {
        Error::Invalid(
            "set --user-agent or PARSEC_EDGAR_USER_AGENT to your name and contact email, \
             e.g. \"Example Corp admin@example.com\" (the SEC requires it)"
                .into(),
        )
    })?;
    let cache_dir = if cli.no_cache { None } else { cli.cache_dir.or_else(default_cache_dir) };
    let client = Client::builder(user_agent).requests_per_second(cli.rps).cache_dir(cache_dir).build()?;

    match cli.command {
        Command::Company { query } => print_json(&client.company(&query)?),
        Command::Filings { company, forms, since, until, limit, all_history } => {
            let query = FilingQuery { forms, since, until, limit, all_history };
            print_json(&client.filings(&company, &query)?);
        }
        Command::Text { company, which } => {
            let filing = pick(&client, &company, &which)?;
            print!("{}", client.filing_text(&filing)?);
        }
        Command::Sections { company, which, items, no_text } => {
            let filing = pick(&client, &company, &which)?;
            let wanted: Vec<String> = items.iter().map(|i| normalize_section_id(i)).collect();
            let mut sections = client.filing_sections(&filing)?;
            if !wanted.is_empty() {
                sections.retain(|s| wanted.contains(&s.id));
            }
            if no_text {
                sections.iter_mut().for_each(|s| s.text.clear());
            }
            print_json(&serde_json::json!({ "filing": filing, "sections": sections }));
        }
        Command::Crawl(args) => crawl::run(&client, args)?,
    }
    Ok(())
}

fn pick(client: &Client, company: &str, which: &Which) -> Result<Filing, Error> {
    match &which.accession {
        Some(acc) => client.filing(company, acc),
        None => client.latest(company, which.form.as_deref().unwrap_or("10-K")),
    }
}

/// `1A` -> `item_1a`; ids that already start with `item_` or `part_` pass through.
fn normalize_section_id(s: &str) -> String {
    let s = s.trim().to_ascii_lowercase();
    if s.starts_with("item_") || s.starts_with("part_") { s } else { format!("item_{s}") }
}

fn default_cache_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("parsec-edgar").join("cache"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .map(|d| d.join("parsec-edgar"))
    }
}

fn print_json<T: serde::Serialize + ?Sized>(value: &T) {
    println!("{}", serde_json::to_string_pretty(value).expect("values serialize"));
}
