//! The `parsec-edgar` command-line tool. The binary in this crate and the
//! `parsec-edgar` command installed by the Python package both call [`run_from`].

mod crawl;
mod mcp;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use parsec_edgar::{
    Client, Error, Filing, FilingQuery, FormKind, Section, Table, document_to_text_and_tables, extract_sections,
};

/// Fast SEC EDGAR client: companies, filings, clean text, and Item sections and tables as JSON.
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

    /// Tables of a filing's primary document as JSON.
    ///
    /// Each table has its rows of cells, its byte offsets in the clean text (as
    /// printed by `text`), and the id of the Item section it is in.
    Tables {
        company: String,
        #[command(flatten)]
        which: Which,
        /// Only tables in these sections, e.g. 8, item_7, part_1_item_1. Repeat for several.
        #[arg(long = "item")]
        items: Vec<String>,
    },

    /// Bulk download filings by form and date (all companies, or a list) into JSON files.
    Crawl(crawl::CrawlArgs),
    /// Run an MCP server for LLM clients (stdio unless --http is given).
    Mcp(mcp::McpArgs),
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

/// Run the command line with these arguments (the first is the program name)
/// and return the process exit code: 0 on success, 1 on an error, 2 on bad usage.
pub fn run_from<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        // --help and --version also arrive here, with exit code 0.
        Err(e) => {
            let _ = e.print();
            return u8::try_from(e.exit_code()).unwrap_or(2);
        }
    };
    let result = run(cli);
    let _ = std::io::stdout().flush();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
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
        Command::Company { query } => print_json(&client.company(&query)?)?,
        Command::Filings { company, forms, since, until, limit, all_history } => {
            let query = FilingQuery { forms, since, until, limit, all_history };
            print_json(&client.filings(&company, &query)?)?;
        }
        Command::Text { company, which } => {
            let filing = pick(&client, &company, &which)?;
            write_stdout(&client.filing_text(&filing)?)?;
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
            print_json(&serde_json::json!({ "filing": filing, "sections": sections }))?;
        }
        Command::Tables { company, which, items } => {
            let filing = pick(&client, &company, &which)?;
            let (text, tables) = document_to_text_and_tables(&client.primary_document(&filing)?);
            let sections = match FormKind::from_form(&filing.form) {
                Some(kind) => extract_sections(&text, kind),
                None if items.is_empty() => Vec::new(),
                None => {
                    return Err(Error::Invalid(format!(
                        "--item needs a 10-K, 10-Q or 8-K filing, which have Item sections, not {}",
                        filing.form
                    )));
                }
            };
            let wanted: Vec<String> = items.iter().map(|i| normalize_section_id(i)).collect();
            let tables = tables_in_sections(&tables, &sections, &wanted);
            print_json(&serde_json::json!({ "filing": filing, "tables": tables }))?;
        }
        Command::Crawl(args) => crawl::run(&client, args)?,
        Command::Mcp(args) => mcp::serve(client, args)?,
    }
    Ok(())
}

fn pick(client: &Client, company: &str, which: &Which) -> Result<Filing, Error> {
    match &which.accession {
        Some(acc) => client.filing(company, acc),
        None => client.latest(company, which.form.as_deref().unwrap_or("10-K")),
    }
}

/// A table with the id of the Item section that holds its start.
#[derive(serde::Serialize)]
struct TableIn<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<&'a str>,
    #[serde(flatten)]
    table: &'a Table,
}

/// Each table with its section, keeping only tables in the `wanted` section ids when any are given.
fn tables_in_sections<'a>(tables: &'a [Table], sections: &'a [Section], wanted: &[String]) -> Vec<TableIn<'a>> {
    tables
        .iter()
        .map(|table| {
            let section = sections.iter().find(|s| s.start <= table.start && table.start < s.end);
            TableIn { section: section.map(|s| s.id.as_str()), table }
        })
        .filter(|t| wanted.is_empty() || t.section.is_some_and(|id| wanted.iter().any(|w| w == id)))
        .collect()
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

fn print_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<(), Error> {
    let mut json = serde_json::to_string_pretty(value).expect("values serialize");
    json.push('\n');
    write_stdout(&json)
}

/// Write to standard output. A reader that closed the pipe early, as in
/// `parsec-edgar filings AAPL | head`, is not an error.
fn write_stdout(text: &str) -> Result<(), Error> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => Err(e.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_get_their_sections() {
        let body = "<p>We make and sell things in many markets around the world, every year.</p>".repeat(30);
        let html = format!(
            "<table><tr><td>Cover</td><td>page</td></tr></table>\
             <p>Item 7. Management's Discussion and Analysis</p>{body}\
             <table><tr><td>Net sales</td><td>100</td></tr></table>{body}\
             <p>Item 8. Financial Statements and Supplementary Data</p>{body}\
             <table><tr><td>Total assets</td><td>500</td></tr></table>{body}"
        );
        let (text, tables) = parsec_edgar::html_to_text_and_tables(&html);
        let sections = extract_sections(&text, FormKind::TenK);
        let json = |wanted: &[&str]| {
            let wanted: Vec<String> = wanted.iter().map(|w| normalize_section_id(w)).collect();
            serde_json::to_value(tables_in_sections(&tables, &sections, &wanted)).unwrap()
        };
        let all = json(&[]);
        assert_eq!(all[0].get("section"), None);
        assert_eq!(all[1]["section"], "item_7");
        assert_eq!(all[2]["section"], "item_8");
        assert_eq!(all[2]["rows"], serde_json::json!([["Total assets", "500"]]));
        assert_eq!(all[2]["start"], tables[2].start);
        let item_8 = json(&["8"]);
        assert_eq!(item_8.as_array().unwrap().len(), 1);
        assert_eq!(item_8[0]["rows"][0][0], "Total assets");
    }
}
