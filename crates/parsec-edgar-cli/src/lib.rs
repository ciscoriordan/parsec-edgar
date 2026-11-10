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

    /// Item sections of a 10-K, 10-Q, 8-K, 10-KSB or 10-QSB as JSON.
    Sections {
        company: String,
        #[command(flatten)]
        which: Which,
        /// Only these sections, e.g. 1A, item_7, "Item 7", part_2_item_1a, 2.02, or in a 10-Q
        /// an Item number that only one part has. Repeat for several.
        #[arg(long = "item")]
        items: Vec<String>,
        /// Leave out section text (ids, titles, headings and offsets only).
        #[arg(long)]
        no_text: bool,
    },

    /// Tables of a filing's primary document as JSON.
    ///
    /// Each table has its rows of cells, each cell's column in the table's grid
    /// and how many columns it covers, its byte offsets in the clean text (as
    /// printed by `text`), and the id of the Item section it is in.
    Tables {
        company: String,
        #[command(flatten)]
        which: Which,
        /// Only tables in these sections, e.g. 8, item_7, "Item 7", part_1_item_1, or in a 10-Q
        /// an Item number that only one part has. Repeat for several.
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
            let mut sections = client.filing_sections(&filing)?;
            if !items.is_empty() {
                if sections.is_empty() {
                    return Err(Error::Invalid(
                        "no Item sections were found in this filing; the text command prints its text".into(),
                    ));
                }
                let wanted = wanted_ids(&sections, &items)?;
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
            let sections = table_sections(&text, &filing.form, &items)?;
            let wanted = if items.is_empty() {
                Vec::new()
            } else if sections.is_empty() {
                return Err(Error::Invalid(format!(
                    "no Item sections were found in this filing; leave out --item to print all {} tables",
                    tables.len()
                )));
            } else {
                wanted_ids(&sections, &items)?
            };
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

/// The Item sections of a filing's text for the tables command: none for a
/// form without Items, which `--item` cannot be used with.
fn table_sections(text: &str, form: &str, items: &[String]) -> Result<Vec<Section>, Error> {
    match FormKind::from_form(form) {
        Some(kind) => Ok(extract_sections(text, kind)),
        None if items.is_empty() => Ok(Vec::new()),
        None => Err(Error::Invalid(format!(
            "--item needs a {} filing, which have Item sections, not {form}",
            FormKind::FORMS
        ))),
    }
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

/// The ids of the sections that `items` name, or an error when one names none.
fn wanted_ids(sections: &[Section], items: &[String]) -> Result<Vec<String>, Error> {
    items.iter().map(|item| find_section(sections, item).map(|s| s.id.clone()).map_err(Error::Invalid)).collect()
}

/// The section that `wanted` names: an id such as `item_1a`, or another form
/// of it such as `1A` or `Item 1A`. In a 10-Q, an Item number alone names the
/// Item when only one part has it. The error lists the ids there are.
pub(crate) fn find_section<'a>(sections: &'a [Section], wanted: &str) -> Result<&'a Section, String> {
    let id = normalize_section_id(wanted);
    if let Some(s) = sections.iter().find(|s| s.id == id) {
        return Ok(s);
    }
    // "1A" in a 10-Q: accept it when only one part has that Item.
    if id.starts_with("item_") {
        let suffix = format!("_{id}");
        let found: Vec<&Section> = sections.iter().filter(|s| s.id.ends_with(&suffix)).collect();
        match found.as_slice() {
            [one] => return Ok(one),
            [] => {}
            many => {
                let ids: Vec<&str> = many.iter().map(|s| s.id.as_str()).collect();
                return Err(format!("section {wanted:?} is ambiguous; use one of {}", ids.join(", ")));
            }
        }
    }
    let ids: Vec<&str> = sections.iter().map(|s| s.id.as_str()).collect();
    if ids.is_empty() {
        return Err("no Item sections were found in this filing".into());
    }
    Err(format!("no section {wanted:?} in this filing; available: {}", ids.join(", ")))
}

/// `Item 1A.` -> `item_1a`, `Part II, Item 1A` -> `part_2_item_1a`, `2.02` -> `item_2.02`.
pub(crate) fn normalize_section_id(s: &str) -> String {
    let s = s.trim().trim_end_matches('.').to_ascii_lowercase().replace(',', " ");
    let mut id = s.split_whitespace().collect::<Vec<_>>().join("_");
    // One underscore after each keyword, so "item1a" and "item_1a" agree.
    for word in ["item", "part"] {
        id = id.replace(&format!("{word}_"), word).replace(word, &format!("{word}_"));
    }
    for (roman, n) in [("part_ii_", "part_2_"), ("part_i_", "part_1_")] {
        if let Some(rest) = id.strip_prefix(roman) {
            id = format!("{n}{rest}");
        }
    }
    if id.starts_with("item_") || id.starts_with("part_") { id } else { format!("item_{id}") }
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
            let items: Vec<String> = wanted.iter().map(|w| w.to_string()).collect();
            let wanted = wanted_ids(&sections, &items).unwrap();
            serde_json::to_value(tables_in_sections(&tables, &sections, &wanted)).unwrap()
        };
        let all = json(&[]);
        assert_eq!(all[0].get("section"), None);
        assert_eq!(all[1]["section"], "item_7");
        assert_eq!(all[2]["section"], "item_8");
        assert_eq!(all[2]["rows"], serde_json::json!([["Total assets", "500"]]));
        assert_eq!(all[2]["start"], tables[2].start);
        for item in ["8", "Item 8", "item_8", "ITEM 8."] {
            let item_8 = json(&[item]);
            assert_eq!(item_8.as_array().unwrap().len(), 1, "{item}");
            assert_eq!(item_8[0]["rows"][0][0], "Total assets");
        }
        // An id that names no section is an error that lists the ones there are.
        let err = wanted_ids(&sections, &["9".into()]).unwrap_err().to_string();
        assert!(err.contains("no section \"9\"") && err.contains("available: item_7, item_8"), "{err}");
    }

    fn section(id: &str) -> Section {
        let s = serde_json::json!({
            "id": id, "item": "", "title": "Risk Factors", "heading": "", "start": 0, "end": 0, "text": ""
        });
        serde_json::from_value(s).unwrap()
    }

    #[test]
    fn section_ids() {
        assert_eq!(normalize_section_id("1A"), "item_1a");
        assert_eq!(normalize_section_id("Item 7."), "item_7");
        assert_eq!(normalize_section_id("item_7a"), "item_7a");
        assert_eq!(normalize_section_id("ITEM 2.02"), "item_2.02");
        assert_eq!(normalize_section_id("2.02"), "item_2.02");
        assert_eq!(normalize_section_id("Part II, Item 1A"), "part_2_item_1a");
        assert_eq!(normalize_section_id("part_1_item_2"), "part_1_item_2");
        assert_eq!(normalize_section_id("part 1 item 2"), "part_1_item_2");
        assert_eq!(normalize_section_id("item1a"), "item_1a");
    }

    #[test]
    fn finds_sections() {
        let q =
            [section("part_1_item_1"), section("part_1_item_2"), section("part_2_item_1"), section("part_2_item_1a")];
        assert_eq!(find_section(&q, "part_1_item_2").unwrap().id, "part_1_item_2");
        assert_eq!(find_section(&q, "1A").unwrap().id, "part_2_item_1a");
        assert_eq!(find_section(&q, "Item 2").unwrap().id, "part_1_item_2");
        let err = find_section(&q, "1").unwrap_err();
        assert!(err.contains("ambiguous") && err.contains("part_2_item_1"), "{err}");
        let err = find_section(&q, "item_9").unwrap_err();
        assert!(err.contains("available: part_1_item_1"), "{err}");
        assert_eq!(find_section(&[], "1").unwrap_err(), "no Item sections were found in this filing");
    }

    /// `--item` takes the ids the MCP tools take, so a 10-Q Item number that
    /// only one part has finds its section. Most 10-Qs have an Item 2 in both
    /// parts, so "2" alone is an error that names both, as is an id that
    /// matches none.
    #[test]
    fn ten_q_items_by_number() {
        let body = "<p>We make and sell things in many markets around the world, every quarter.</p>".repeat(30);
        let html = format!(
            "<p>PART I. FINANCIAL INFORMATION</p><p>Item 1. Financial Statements</p>{body}\
             <table><tr><td>Total assets</td><td>500</td></tr></table>{body}\
             <p>Item 2. Management's Discussion and Analysis</p>{body}\
             <table><tr><td>Net sales</td><td>100</td></tr></table>{body}\
             <p>PART II. OTHER INFORMATION</p><p>Item 1A. Risk Factors</p>{body}\
             <p>Item 2. Unregistered Sales of Equity Securities and Use of Proceeds</p>{body}\
             <p>Item 6. Exhibits</p>{body}"
        );
        let (text, tables) = parsec_edgar::html_to_text_and_tables(&html);
        let sections = extract_sections(&text, FormKind::TenQ);
        let ids: Vec<&str> = sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["part_1_item_1", "part_1_item_2", "part_2_item_1a", "part_2_item_2", "part_2_item_6"]);
        for item in ["Part I, Item 2", "part_1_item_2", "Part 1 Item 2"] {
            let wanted = wanted_ids(&sections, &[item.into()]).unwrap();
            assert_eq!(wanted, ["part_1_item_2"], "{item}");
            let found = tables_in_sections(&tables, &sections, &wanted);
            assert_eq!(found.len(), 1, "{item}");
            assert_eq!(found[0].table.rows[0][0], "Net sales");
        }
        for item in ["1A", "Item 1A", "item_1a"] {
            assert_eq!(wanted_ids(&sections, &[item.into()]).unwrap(), ["part_2_item_1a"], "{item}");
        }
        for item in ["2", "Item 2"] {
            let err = wanted_ids(&sections, &[item.into()]).unwrap_err().to_string();
            assert!(err.contains("ambiguous; use one of part_1_item_2, part_2_item_2"), "{err}");
        }
        let err = wanted_ids(&sections, &["1A".into(), "Item 3".into()]).unwrap_err().to_string();
        assert!(err.contains("no section \"Item 3\"") && err.contains("available: part_1_item_1"), "{err}");
    }

    /// The help and the errors name every form with Item sections.
    #[test]
    fn names_the_forms_with_items() {
        use clap::CommandFactory;
        let cli = Cli::command();
        let about = cli.find_subcommand("sections").and_then(|c| c.get_about()).unwrap().to_string();
        assert!(about.contains(FormKind::FORMS), "{about}");
        let text = format!("Item 7. Financial Statements\n{}", "We make things. ".repeat(100));
        let err = table_sections(&text, "S-1", &["7".into()]).unwrap_err().to_string();
        assert!(err.contains(FormKind::FORMS) && err.ends_with("not S-1"), "{err}");
        assert!(table_sections(&text, "S-1", &[]).unwrap().is_empty());
        let sections = table_sections(&text, "10KSB", &["7".into()]).unwrap();
        assert_eq!((sections[0].id.as_str(), sections[0].title.as_str()), ("item_7", "Financial Statements"));
    }
}
