//! Fast SEC EDGAR client and filing parser.
//!
//! - [`Client`]: rate-limited, cached access to sec.gov and data.sec.gov
//! - [`Company`] lookup by ticker or CIK, and [`Filing`] lists from the submissions API
//! - [`html_to_text`]: filing HTML to clean plain text
//! - [`html_to_text_and_tables`]: the same text, plus its [`Table`]s as rows of cells with their offsets in it
//! - [`extract_sections`]: 10-K, 10-Q and 8-K Item sections
//!
//! ```no_run
//! use parsec_edgar::{Client, FilingQuery};
//!
//! let client = Client::builder("Example Corp admin@example.com").build()?;
//! let filings = client.filings("AAPL", &FilingQuery::forms(&["10-K"]).limit(1))?;
//! let sections = client.filing_sections(&filings[0])?;
//! for s in &sections {
//!     println!("{} {} ({} chars)", s.id, s.title, s.text.len());
//! }
//! # Ok::<(), parsec_edgar::Error>(())
//! ```

mod client;
mod company;
mod crawl;
mod efts;
mod encoding;
mod error;
mod filing_index;
mod filings;
mod html;
mod index;
mod profile;
mod sections;

pub use client::{Client, ClientBuilder};
pub use company::Company;
pub use crawl::{CrawlQuery, Document};
pub use efts::{
    FULL_TEXT_MAX_RESULTS, FULL_TEXT_PAGE_SIZE, FullTextFiler, FullTextHit, FullTextQuery, FullTextResults,
};
pub use error::{Error, Result};
pub use filing_index::{Filer, FilingIndex, IndexDocument, parse_filing_index};
pub use filings::{Filing, FilingQuery};
pub use html::{
    Table, document_is_html, document_to_text, document_to_text_and_tables, html_to_text, html_to_text_and_tables,
};
pub use index::{IndexEntry, Quarter, parse_master_index};
pub use profile::CompanyProfile;
pub use sections::{FormKind, Section, extract_sections};
