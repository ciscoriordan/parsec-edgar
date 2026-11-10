//! Item sections of 10-K, 10-Q and 8-K filings, and of the small business
//! forms 10-KSB and 10-QSB, which number their Items differently. A filing
//! whose Item headings fit the other numbering better (a 10-KSB that prints
//! "Item 8. Financial Statements", a 10-K that prints "Item 7. Financial
//! Statements" and "Item 8A(T)") is split with the other form's Items.
//!
//! Every line that looks like an Item heading becomes a candidate. Tables of
//! contents and running page headers also produce candidates, so the extractor
//! picks one candidate per Item such that the chosen headings appear in the
//! form's Item order and the total score is highest. A candidate scores
//! `ln(1 + gap)`, where `gap` is the distance to the next heading of a
//! different Item: body headings start long sections, table-of-contents
//! entries start a few characters of text.
//!
//! Before that, contents entries whose Item and title reappear in the body
//! are dropped, and so is a bare page header ("Item 7") just above its
//! titled heading. In a 10-Q or 10-QSB a `PART I` or `PART II` line just
//! above a heading fixes its part, and without one a title that only one part
//! uses for that Item number does. Such a line that starts with a Part I
//! title and runs on as a sentence that cites the Item ("Item 1. Financial
//! Statements, of this report") is a wrapped cross-reference, not a heading.
//! In every form, so is an Item line that carries on a sentence from the line
//! just above it, which ends with a comma or with a word such as "in", "see"
//! or "to" ("as set forth in Part II,").
//! A heading that names several Items ("Items 1 and 2.", "Items 1 through
//! 5.", or with a dash, "Item 1-5.", but not "Item 1-2 of the Form") starts
//! the first of them. One that puts a comma after the Item number, as
//! running text also does ("Item 8, which ..."), counts only when the line
//! ends where the Item's standard title ends ("Item 8, Financial Statements
//! and Supplementary Data"), or where the numbers of more Items in form
//! order, the last after "and", or the standard titles of all of them end
//! ("ITEMS 9, 9A AND 9B.").
//! The last Item ends at the signatures or where a separately paginated
//! attached document (an annual report) begins.
//!
//! A titled sub-item with the standard title of a lettered Item the form
//! defines is that Item ("Item 8(A)T. Controls and Procedures" in a 10-KSB,
//! "ITEM 9(A). CONTROLS AND PROCEDURES" in a 10-K), also with the title on
//! the next line ("ITEM 9(A)." then "CONTROLS AND PROCEDURES"). A title may
//! start with a form's name ("ITEM 16. 10-K SUMMARY"). Lettered headings of
//! Items the form does not define stay out of the chain. When the chain has
//! no Item of their number, the first one printed where that Item belongs
//! starts it ("Item 5A" and "Item 5B" print Item 5); in a 10-Q a `PART` line
//! or the title picks the part. Otherwise, in a 10-K or 10-KSB, a titled one
//! ends the Item before it. The executive officers section of a 10-K or
//! 10-KSB, numbered "Item 4A" or "Item 4(a)", never starts an Item.
//!
//! Some annual report text has no Item heading of its own and belongs to no
//! Item: financial statements printed with no Item 8 heading or after Item 9
//! of a 10-K, and statements, schedules, an exhibit index, a consent or a
//! glossary printed after the last Item. A block heading (an auditors' report,
//! an index to the financial statements, a financial statement schedule, an
//! exhibit index or an accountants' consent) ends the Items that never hold
//! such text: changes in accountants (10-K Item 9, 10-KSB Item 8) and the
//! Items after the exhibits (10-K Item 16, 10-KSB Items 14 and 15), which a
//! glossary heading ends too. It ends 10-K Item 7A too when no Item 8 heading
//! was found. It does not when the section also holds the next Item ("Items 9
//! and 9A.", or "ITEM 9 A" after "ITEM 9."), or when the Item after the
//! exhibits is titled as the exhibits Item (Item 16 of some 2003 10-Ks). In
//! other Items these headings are part of the Item: the statements (10-K Item
//! 8, 10-KSB Item 7), the report on internal control (10-K Item 9A, 10-KSB
//! Item 8A), and the statements and exhibit index of the exhibits Item (10-K
//! Item 15, 10-KSB Item 13, or an Item after the statements Item that is
//! titled as the exhibits Item). The exhibits Item ends only where it prints
//! statements whose pages are numbered apart from the report's (F-1, F-2,
//! ...) after all of its own text: an index with such page numbers, then at
//! least two of the statements it lists, and none of the Item's own text
//! after them (an exhibit index, an exhibit list's caption or table header,
//! or a caption such as "(b) Reports on Form 8-K").

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Form families with Item sections. Each has its own list of Items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FormKind {
    /// Form 10-K, the annual report.
    TenK,
    /// Form 10-Q, the quarterly report, with Items in Part I and Part II.
    TenQ,
    /// Form 8-K, the current report, with Items numbered `2.02` and the like.
    EightK,
    /// Form 10-KSB, the annual report of a small business issuer (Regulation
    /// S-B, until 2009). Its Items are numbered differently from Form 10-K's:
    /// Item 6 is MD&A or plan of operation, Item 7 the financial statements,
    /// Items 8A and 8B controls and procedures and other information, Item 13
    /// the exhibits.
    TenKsb,
    /// Form 10-QSB, the quarterly report of a small business issuer. Part I
    /// has Items 1 to 3, Item 3 being controls and procedures.
    TenQsb,
}

impl FormKind {
    /// The forms with Item sections, as messages name them ("needs a 10-K,
    /// 10-Q, 8-K, 10-KSB or 10-QSB filing"). Their amendments and variants
    /// count too, as [`FormKind::from_form`] reads them.
    pub const FORMS: &'static str = "10-K, 10-Q, 8-K, 10-KSB or 10-QSB";

    /// Classify a form type. Amendments and old variants (10-K405, 10-KT, 10-KSB405, 8-K12B) count,
    /// also as EDGAR's indexes spell some of them, without the hyphen (`10KSB`, `10KSB40`, `10QSB`,
    /// `10KT405`).
    pub fn from_form(form: &str) -> Option<Self> {
        let f: String = form.trim().chars().filter(|&c| c != '-').map(|c| c.to_ascii_uppercase()).collect();
        let base = f.strip_suffix("/A").unwrap_or(&f);
        if base.starts_with("10KSB") {
            Some(Self::TenKsb)
        } else if base.starts_with("10QSB") {
            Some(Self::TenQsb)
        } else if base.starts_with("10K") {
            Some(Self::TenK)
        } else if base.starts_with("10Q") {
            Some(Self::TenQ)
        } else if base.starts_with("8K") {
            Some(Self::EightK)
        } else {
            None
        }
    }

    /// An annual report: 10-K or 10-KSB.
    fn annual(self) -> bool {
        matches!(self, Self::TenK | Self::TenKsb)
    }

    /// A quarterly report, whose Items are in two parts: 10-Q or 10-QSB.
    fn quarterly(self) -> bool {
        matches!(self, Self::TenQ | Self::TenQsb)
    }

    /// The form whose Item numbers a filing of this kind may print instead.
    /// Small business issuers moved from Forms 10-KSB and 10-QSB to Forms
    /// 10-K and 10-Q in 2008 and 2009, and around the change some filed one
    /// form with the other's Items.
    fn other_numbering(self) -> Option<Self> {
        match self {
            Self::TenK => Some(Self::TenKsb),
            Self::TenKsb => Some(Self::TenK),
            Self::TenQ => Some(Self::TenQsb),
            Self::TenQsb => Some(Self::TenQ),
            Self::EightK => None,
        }
    }
}

/// One Item section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Stable id: `item_1a` (10-K, 10-KSB), `part_2_item_1a` (10-Q, 10-QSB), `item_2.02` (8-K).
    pub id: String,
    /// `I` or `II` for 10-Q and 10-QSB sections.
    pub part: Option<String>,
    /// Item number as printed on the form, e.g. `1A`, `2.02`.
    pub item: String,
    /// Standard title of the Item.
    pub title: String,
    /// The heading line as it appears in the filing.
    pub heading: String,
    /// Byte offsets of the section in the filing text.
    pub start: usize,
    pub end: usize,
    /// Section text, starting with its heading line.
    pub text: String,
}

struct Key {
    part: Option<&'static str>,
    item: &'static str,
    title: &'static str,
}

const fn k(item: &'static str, title: &'static str) -> Key {
    Key { part: None, item, title }
}

const fn q(part: &'static str, item: &'static str, title: &'static str) -> Key {
    Key { part: Some(part), item, title }
}

const TEN_K: &[Key] = &[
    k("1", "Business"),
    k("1A", "Risk Factors"),
    k("1B", "Unresolved Staff Comments"),
    k("1C", "Cybersecurity"),
    k("2", "Properties"),
    k("3", "Legal Proceedings"),
    k("4", "Mine Safety Disclosures"),
    k(
        "5",
        "Market for Registrant's Common Equity, Related Stockholder Matters and Issuer Purchases of Equity Securities",
    ),
    k("6", "[Reserved]"),
    k("7", "Management's Discussion and Analysis of Financial Condition and Results of Operations"),
    k("7A", "Quantitative and Qualitative Disclosures About Market Risk"),
    k("8", "Financial Statements and Supplementary Data"),
    k("9", "Changes in and Disagreements with Accountants on Accounting and Financial Disclosure"),
    k("9A", "Controls and Procedures"),
    k("9B", "Other Information"),
    k("9C", "Disclosure Regarding Foreign Jurisdictions that Prevent Inspections"),
    k("10", "Directors, Executive Officers and Corporate Governance"),
    k("11", "Executive Compensation"),
    k("12", "Security Ownership of Certain Beneficial Owners and Management and Related Stockholder Matters"),
    k("13", "Certain Relationships and Related Transactions, and Director Independence"),
    k("14", "Principal Accountant Fees and Services"),
    k("15", "Exhibits and Financial Statement Schedules"),
    k("16", "Form 10-K Summary"),
];

const TEN_Q: &[Key] = &[
    q("I", "1", "Financial Statements"),
    q("I", "2", "Management's Discussion and Analysis of Financial Condition and Results of Operations"),
    q("I", "3", "Quantitative and Qualitative Disclosures About Market Risk"),
    q("I", "4", "Controls and Procedures"),
    q("II", "1", "Legal Proceedings"),
    q("II", "1A", "Risk Factors"),
    q("II", "2", "Unregistered Sales of Equity Securities and Use of Proceeds"),
    q("II", "3", "Defaults Upon Senior Securities"),
    q("II", "4", "Mine Safety Disclosures"),
    q("II", "5", "Other Information"),
    q("II", "6", "Exhibits"),
];

const EIGHT_K: &[Key] = &[
    k("1.01", "Entry into a Material Definitive Agreement"),
    k("1.02", "Termination of a Material Definitive Agreement"),
    k("1.03", "Bankruptcy or Receivership"),
    k("1.04", "Mine Safety - Reporting of Shutdowns and Patterns of Violations"),
    k("1.05", "Material Cybersecurity Incidents"),
    k("2.01", "Completion of Acquisition or Disposition of Assets"),
    k("2.02", "Results of Operations and Financial Condition"),
    k(
        "2.03",
        "Creation of a Direct Financial Obligation or an Obligation under an Off-Balance Sheet Arrangement of a Registrant",
    ),
    k(
        "2.04",
        "Triggering Events That Accelerate or Increase a Direct Financial Obligation or an Obligation under an Off-Balance Sheet Arrangement",
    ),
    k("2.05", "Costs Associated with Exit or Disposal Activities"),
    k("2.06", "Material Impairments"),
    k("3.01", "Notice of Delisting or Failure to Satisfy a Continued Listing Rule or Standard; Transfer of Listing"),
    k("3.02", "Unregistered Sales of Equity Securities"),
    k("3.03", "Material Modification to Rights of Security Holders"),
    k("4.01", "Changes in Registrant's Certifying Accountant"),
    k(
        "4.02",
        "Non-Reliance on Previously Issued Financial Statements or a Related Audit Report or Completed Interim Review",
    ),
    k("5.01", "Changes in Control of Registrant"),
    k(
        "5.02",
        "Departure of Directors or Certain Officers; Election of Directors; Appointment of Certain Officers; Compensatory Arrangements of Certain Officers",
    ),
    k("5.03", "Amendments to Articles of Incorporation or Bylaws; Change in Fiscal Year"),
    k("5.04", "Temporary Suspension of Trading Under Registrant's Employee Benefit Plans"),
    k("5.05", "Amendments to the Registrant's Code of Ethics, or Waiver of a Provision of the Code of Ethics"),
    k("5.06", "Change in Shell Company Status"),
    k("5.07", "Submission of Matters to a Vote of Security Holders"),
    k("5.08", "Shareholder Director Nominations"),
    k("6.01", "ABS Informational and Computational Material"),
    k("6.02", "Change of Servicer or Trustee"),
    k("6.03", "Change in Credit Enhancement or Other External Support"),
    k("6.04", "Failure to Make a Required Distribution"),
    k("6.05", "Securities Act Updating Disclosure"),
    k("6.06", "Static Pool"),
    k("7.01", "Regulation FD Disclosure"),
    k("8.01", "Other Events"),
    k("9.01", "Financial Statements and Exhibits"),
];

/// Form 10-KSB's Items, with the titles of its last version (2008), and two
/// Items that 10-KSBs print outside that list: Item 1A, Risk Factors, as in
/// Form 10-K, and Item 15, Principal Accountant Fees and Services, of the
/// 10-KSBs for 2003, whose Item 14 was Controls and Procedures.
const TEN_KSB: &[Key] = &[
    k("1", "Description of Business"),
    k("1A", "Risk Factors"),
    k("2", "Description of Property"),
    k("3", "Legal Proceedings"),
    k("4", "Submission of Matters to a Vote of Security Holders"),
    k(
        "5",
        "Market for Common Equity and Related Stockholder Matters and Small Business Issuer Purchases of Equity Securities",
    ),
    k("6", "Management's Discussion and Analysis or Plan of Operation"),
    k("7", "Financial Statements"),
    k("8", "Changes in and Disagreements with Accountants on Accounting and Financial Disclosure"),
    k("8A", "Controls and Procedures"),
    k("8B", "Other Information"),
    k(
        "9",
        "Directors, Executive Officers, Promoters, Control Persons and Corporate Governance; Compliance with Section 16(a) of the Exchange Act",
    ),
    k("10", "Executive Compensation"),
    k("11", "Security Ownership of Certain Beneficial Owners and Management and Related Stockholder Matters"),
    k("12", "Certain Relationships and Related Transactions, and Director Independence"),
    k("13", "Exhibits"),
    k("14", "Principal Accountant Fees and Services"),
    k("15", "Principal Accountant Fees and Services"),
];

/// Form 10-QSB's Items, with the titles of its last version (2008), and Part
/// II Item 1A, Risk Factors, which 10-QSBs print as 10-Qs do.
const TEN_QSB: &[Key] = &[
    q("I", "1", "Financial Statements"),
    q("I", "2", "Management's Discussion and Analysis or Plan of Operation"),
    q("I", "3", "Controls and Procedures"),
    q("II", "1", "Legal Proceedings"),
    q("II", "1A", "Risk Factors"),
    q("II", "2", "Unregistered Sales of Equity Securities and Use of Proceeds"),
    q("II", "3", "Defaults Upon Senior Securities"),
    q("II", "4", "Submission of Matters to a Vote of Security Holders"),
    q("II", "5", "Other Information"),
    q("II", "6", "Exhibits"),
];

fn keys_for(kind: FormKind) -> &'static [Key] {
    match kind {
        FormKind::TenK => TEN_K,
        FormKind::TenQ => TEN_Q,
        FormKind::EightK => EIGHT_K,
        FormKind::TenKsb => TEN_KSB,
        FormKind::TenQsb => TEN_QSB,
    }
}

/// `Item 1A. Risk Factors`, `PART II - ITEM 1A RISK FACTORS`, `PART I -- ITEM 2`, `Item 4(T).`, `Items 1 and 2.`,
/// `Item 9 A.`
static PERIODIC_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]*(?:(?i:part)[ \t]+[IV]{1,3}[ \t]*(?:[,.:]|[\-–—]+)?[ \t]*)?(?i:items?)[ \t]*(\d{1,2}[A-Ca-c]?)(?:[ \t]*\([Tt]\))?[ \t]*[.:\-–—]?[ \t]*([^\n]*)$",
    )
    .unwrap()
});

/// `Item 2.02 Results of Operations and Financial Condition`
static CURRENT_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]*(?i:item)[ \t]*(\d{1,2}\.\d{2})[ \t]*[.:\-–—]?[ \t]*([^\n]*)$").unwrap());

/// `SIGNATURES`, `Signature`, `Signature(s)`, `S I G N A T U R E S`
static SIGNATURES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:S ?I ?G ?N ?A ?T ?U ?R ?E(?: ?S)?|Signatures?|Signature\(s\))[ \t]*[.:]?[ \t]*$")
        .unwrap()
});

/// Starts of signatures heading lines. A literal search finds the few lines
/// worth testing, which is much faster than running the line regex over a
/// long filing.
fn signature_lines(text: &str) -> Vec<usize> {
    static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SIGNATURE|Signature|S I G N").unwrap());
    let mut found: Vec<usize> = WORD
        .find_iter(text)
        .filter_map(|m| {
            let start = text[..m.start()].rfind('\n').map_or(0, |n| n + 1);
            let end = text[m.start()..].find('\n').map_or(text.len(), |n| m.start() + n);
            SIGNATURES.is_match(&text[start..end]).then_some(start)
        })
        .collect();
    found.dedup();
    found
}

/// The signature block's opening sentence, for filings that print no
/// signatures heading.
static SIGNATURE_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]*Pursuant to the requirements of (?:Section 13|the Securities Exchange Act)[^\n]*duly caused",
    )
    .unwrap()
});

/// Bare sub-item references such as `Item 14(d):` or `Item 15(a)(1)`, which
/// sit inside an Item. `Item 6 (a). Exhibits` has a title and stays.
static SUB_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\((?:[a-z]|\d{1,2}|[ivx]{1,4})\))+[\s.:\-–—]*$").unwrap());

/// A title that names executive officers: Form 10-K lets a filer list them in
/// Part I under a caption of its own ("Executive Officers of the Registrant"),
/// which is not one of the form's Items.
static OFFICERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bexecutive\s+officers\b").unwrap());

/// `Index to Consolidated Financial Statements`, one of the block headings.
const STATEMENTS_INDEX: &str = r"index[ \t]+to[ \t]+(?:the[ \t]+)?(?:consolidated[ \t]+)?financial[ \t]+statements\b";

/// `EXHIBIT INDEX`, `Index to Exhibits`, one of the block headings.
const EXHIBITS_INDEX: &str = r"exhibits?[ \t]+index\b|index[ \t]+(?:to|of)[ \t]+exhibits\b";

/// Headings that open text printed with no Item heading of its own: the
/// auditors' report before the financial statements (`Report of Independent
/// Registered Public Accounting Firm`, `Independent Auditors' Report`, or with
/// the firm's name first, `Report of Example LLP, Independent Auditors`), an
/// index to the financial statements, a financial statement schedule
/// (`SCHEDULE I – CONDENSED PARENT COMPANY FINANCIAL STATEMENTS`), an exhibit
/// index, or an accountants' consent.
static BLOCK_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?im)^[ \t]*(?:report[ \t]+of[ \t]+(?:[^\n,]{{1,60}},[ \t]*)?independent\b|independent[ \t]+(?:registered[ \t]+public[ \t]+accounting[ \t]+firm|auditors?['’]?s?['’]?|accountants['’]?)[ \t]+(?:report|consent)\b|{STATEMENTS_INDEX}|schedule[ \t]+[ivx]{{1,4}}\b|{EXHIBITS_INDEX}|consent[ \t]+of[ \t]+independent\b)",
    ))
    .unwrap()
});

/// An index to the financial statements at the start of a line.
static STATEMENTS_INDEX_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)^[ \t]*{STATEMENTS_INDEX}")).unwrap());

/// An exhibit index at the start of a line.
static EXHIBITS_INDEX_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?im)^[ \t]*(?:{EXHIBITS_INDEX})")).unwrap());

/// A caption of one of the exhibits Item's own parts, on a trimmed line:
/// `(a)(3) Exhibits:`, `3. Exhibits`, `(a)(2) Financial Statement Schedules`,
/// `(b) Reports on Form 8-K`, numbered like that, or the lines `Exhibits`
/// and `LIST OF EXHIBITS`, or an exhibit table's header (`Exhibit No.`,
/// `Exhibit Number`).
static EXHIBITS_ITEM_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:(?:\([a-z]\)|\(\d{1,2}\)|\d{1,2}\.)[ \t]*)+(?:(?:list[ \t]+of[ \t]+)?exhibits\b|financial[ \t]+statement[ \t]+schedules?\b|reports?[ \t]+on[ \t]+form[ \t]+8-?k\b)|^(?:list[ \t]+of[ \t]+)?exhibits[ \t]*:?$|^exhibit[ \t]+(?:no\b|number\b)",
    )
    .unwrap()
});

/// A glossary's heading line: `GLOSSARY`, `Glossary of Terms and Acronyms`.
static GLOSSARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*glossary(?:[ \t]+(?:of|and)[ \t]+[^\n.:;]{1,60})?[ \t]*:?[ \t]*$").unwrap()
});

/// An entry of an index to financial statements whose pages are numbered
/// apart from the report's: `Independent Auditors' Report<TAB>F-2`,
/// `Balance Sheets ........ F-3`, `Notes<TAB>F-7 to F-20`, or the page alone
/// on its line below a wrapped title.
static F_PAGE_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|\t| {2,}|\.{2,} ?)F ?[-–] ?\d{1,3}(?:(?: ?[,–-] ?| (?:to|through) )(?:F ?[-–] ?)?\d{1,3})?[ \t]*$",
    )
    .unwrap()
});

/// A line that ends with a page number, as an index or contents entry does:
/// `Balance Sheets<TAB>F-3`, `Report of Independent Auditors ...... S-1`,
/// `Notes to Financial Statements<TAB>22-33`.
static PAGE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\t| {2,}|\.{2,} ?)(?:[A-Z]{1,2} ?[-–] ?)?\d{1,3}(?:[ \t]*[,–-][ \t]*\d{1,3})?[ \t]*$").unwrap()
});

/// `PART I`, `Part II - Other Information`, also as the start of a heading line.
static PART_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*(?i:part)[ \t]+(II|I)(?-u:\b)").unwrap());

/// One entry of a page-numbered table of contents: `26<TAB>Five-year summary` or `Introduction<TAB>46`.
static TOC_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(\d{1,3})(?:\t| {2,})\S.*|\S.*?(?:\t| {2,}|\.{3,} ?)(\d{1,3}))$").unwrap());

struct Candidate {
    pos: usize,
    label: String,
    heading: String,
    /// No title follows the Item number on the heading line.
    bare: bool,
    /// Lowercase letters of the title (from the next line for a bare
    /// heading), for matching a table-of-contents entry with its heading in
    /// the body.
    title: String,
    /// 10-Q part fixed by a `PART I` or `PART II` line just above the heading
    /// or at its start.
    part: Option<&'static str>,
    /// 10-Q part whose standard title for this Item number matches the
    /// heading's title, when only one part's does ("Item 1. Legal Proceedings").
    title_part: Option<&'static str>,
    /// The body heading that a dropped table-of-contents entry names.
    listed: bool,
    /// The executive officers caption: the title names executive officers, or
    /// a 10-K numbers the heading 4A.
    officers: bool,
    /// The heading names a second Item: `Items 9 and 9A.`, `ITEM 7A & 8.`,
    /// `ITEMS 9, 9A AND 9B.`, `Item 1-5.`
    joined: bool,
    /// Distance to the next heading of a different Item.
    gap: usize,
    score: f64,
}

/// A title that starts with a form's name, then a capitalized word: `10-K
/// SUMMARY` (Item 16 of a 10-K), `10-Q Summary`.
static FORM_NAME_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:10-[KQ](?:SB)?|8-K)[ \t]+\p{Lu}").unwrap());

/// A real heading is followed by a capitalized title (also one that starts
/// with a form's name, "ITEM 16. 10-K SUMMARY"), nothing, or a joined Item
/// ("Items 1 and 2."). Cross-references ("Item 7 of this report"), numbers
/// ("Item 5 10,000 shares") and sub-items ("Item 14(d)") are not.
fn heading_tail_ok(tail: &str) -> bool {
    let t = tail.trim_start();
    match t.chars().next() {
        None => true,
        Some(c) => {
            (c.is_uppercase()
                || matches!(c, '"' | '“' | '\'' | '‘' | '(' | '[')
                || t.starts_with("and ")
                || t.starts_with("& ")
                || t.starts_with("through ")
                || FORM_NAME_TITLE.is_match(t))
                && !SUB_ITEM.is_match(t)
        }
    }
}

/// The rest of a heading that names a second Item after the first: `and 2.`
/// in `Items 1 and 2.`, `& 8.` in `ITEM 7A & 8.`
static JOINED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?i:and|&|through)[ \t]+(?i:items?[ \t]*)?\d").unwrap());

/// After the first Item number of a heading, a comma, more Item numbers and
/// the last one after "and" or "&": `, 9A AND 9B.` in `ITEMS 9, 9A AND
/// 9B.`, `., 1A. and 2.` in `ITEMS 1., 1A. and 2. BUSINESS, ...`.
static ITEM_LIST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[ \t]*\.?((?:[ \t]*,[ \t]*\d{1,2}[A-Ca-c]?\.?)+)[ \t]*,?[ \t]*(?:(?i:and)|&)[ \t]*(\d{1,2}[A-Ca-c]?)[ \t]*[.:\-–—]?",
    )
    .unwrap()
});

/// An Item number in an `ITEM_LIST` match.
static ITEM_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{1,2}[A-Ca-c]?").unwrap());

/// After the first Item number of a heading, a dash and the last Item of a
/// range: `-5.` in `Item 1-5.`, ` – 13.` in `ITEMS 10 – 13.`
static ITEM_RANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ \t]*\.?[ \t]*[-–—][ \t]*(\d{1,2}[A-Ca-c]?)(?:[ \t]*[.:])?").unwrap());

/// After the Item number, a comma: `Item 8, Financial Statements`.
static ITEM_COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*\.?[ \t]*,").unwrap());

/// The words of a title as compared whole with a standard title: the stems
/// (`word_stem`) of its words and numbers, without joining words, so
/// "FINANCIAL STATEMENTS & SUPPLEMENTARY DATA." has the words of "Financial
/// Statements and Supplementary Data", and a page number is a word.
fn title_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’')
        .map(|w| w.trim_matches(['\'', '’']))
        .filter(|w| !w.is_empty())
        .map(word_stem)
        .filter(|w| !JOINING_WORDS.contains(&w.as_str()))
        .collect()
}

/// The keys of the Items `labels` names, when they are in form order: one
/// list for each part that defines them all (a 10-Q has Items 1 to 4 in both).
fn in_form_order<'k>(labels: &[String], keys: &'k [Key]) -> Vec<Vec<&'k Key>> {
    let mut parts: Vec<Option<&str>> = keys.iter().map(|k| k.part).collect();
    parts.dedup();
    parts
        .into_iter()
        .filter_map(|part| {
            let at: Vec<usize> = labels
                .iter()
                .map(|l| keys.iter().position(|k| k.item == l && k.part == part))
                .collect::<Option<_>>()?;
            at.windows(2).all(|w| w[0] < w[1]).then(|| at.iter().map(|&i| &keys[i]).collect())
        })
        .collect()
}

/// A heading that puts a comma after the Item number: `Item 8, Financial
/// Statements and Supplementary Data`, or `ITEMS 9, 9A AND 9B.` for several
/// Items. Running text cites Items the same way ("Item 8, which ...", "Item
/// 7, Management's Discussion and Analysis, of our Annual Report"), so the
/// line must end where the standard title of the Item ends, or for several
/// Items, which must be in form order, where their numbers or the standard
/// titles of all of them end. Or a heading that names a range of Items with a
/// dash (`Item 1-5. Not applicable.`, `ITEMS 10 - 13.`), as `Items 1 through
/// 5.` does: the range must go up in form order, and what follows it must
/// look like a title, so a number (`Item 7 - 2005`) or a cross-reference
/// (`Items 1-4 of Part I`) is not one. `after` is the line after the first
/// Item number. Returns the title, and whether the heading names more Items.
fn listed_items<'a>(label: &str, after: &'a str, keys: &[Key]) -> Option<(&'a str, bool)> {
    if let Some(list) = ITEM_LIST.captures(after) {
        let mut labels = vec![label.to_owned()];
        labels.extend(ITEM_NUMBER.find_iter(&list[1]).map(|n| n.as_str().to_ascii_uppercase()));
        labels.push(list[2].to_ascii_uppercase());
        let rest = after[list.get(0).expect("whole match").end()..].trim_start();
        let words = title_words(rest);
        let titled = |ks: &Vec<&Key>| words == ks.iter().flat_map(|k| title_words(k.title)).collect::<Vec<_>>();
        let fits =
            heading_tail_ok(rest) && in_form_order(&labels, keys).iter().any(|ks| words.is_empty() || titled(ks));
        return fits.then_some((rest, true));
    }
    if let Some(range) = ITEM_RANGE.captures(after) {
        let rest = after[range.get(0).expect("whole match").end()..].trim_start();
        let labels = [label.to_owned(), range[1].to_ascii_uppercase()];
        let fits = heading_tail_ok(rest) && !in_form_order(&labels, keys).is_empty();
        return fits.then_some((rest, true));
    }
    let rest = after[ITEM_COMMA.find(after)?.end()..].trim_start();
    let words = title_words(rest);
    let titled = heading_tail_ok(rest) && keys.iter().any(|k| k.item == label && title_words(k.title) == words);
    titled.then_some((rest, false))
}

/// `Item 3. - Quantitative ...`, `ITEM 1 -- FINANCIAL STATEMENTS`: dashes
/// right after the separator that the heading pattern takes are a second
/// separator, and the title starts after them. Done here rather than in the
/// heading regex, like `split_letter`.
fn after_dashes(tail: &str) -> &str {
    tail.trim_start_matches(['-', '–', '—'])
}

fn title_key(tail: &str) -> String {
    tail.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).take(30).collect()
}

/// `Item 9 A. Controls` and `Item 1.A. Risk Factors`: a letter set apart from
/// the number by spaces only, or by a separator with no space after it,
/// belongs to it (in `ITEM 1. A. GENERAL` it starts the title) when a
/// separator follows the letter, or the `(T)` of the temporary Items of 2007
/// to 2009 (`Item 9 A (T). Controls`). Done here rather than in the heading
/// regex, which stays fast that way.
fn split_letter<'a>(label: &str, between: &str, tail: &'a str) -> (String, &'a str) {
    let joins = label.bytes().all(|d| d.is_ascii_digit())
        && (between.trim().is_empty() || !between.ends_with(char::is_whitespace));
    let (Some(letter @ b'A'..=b'C'), true) = (tail.as_bytes().first().copied(), joins) else {
        return (label.to_owned(), tail);
    };
    let after = &tail[1..];
    let temporary = after.trim_start_matches([' ', '\t']);
    let title = if let Some(rest) = after.strip_prefix(['.', ':']) {
        rest.trim_start()
    } else if let Some(rest) = temporary.strip_prefix("(T)").or_else(|| temporary.strip_prefix("(t)")) {
        rest.trim_start_matches([' ', '\t', '.', ':', '-', '–', '—'])
    } else {
        return (label.to_owned(), tail);
    };
    (format!("{label}{}", letter as char), title)
}

/// `ITEM 4(a). EXECUTIVE OFFICERS OF THE REGISTRANT` in a 10-K or 10-KSB: the
/// executive officers caption numbered as a sub-item reads as the lettered
/// Item `4A`, which the form does not define. It keeps its Item's number,
/// like other titled sub-items (`Item 14(a). Financial Statements`), when the
/// form defines the lettered Item (`ITEM 1(a)` is not Item 1A, Risk Factors),
/// and under an Item whose title names executive officers: 10-K Item 10, and
/// 10-KSB Item 9 (`ITEM 9(a). DIRECTORS AND EXECUTIVE OFFICERS`).
fn officers_sub_item<'a>(label: &str, tail: &'a str, keys: &[Key]) -> (String, &'a str) {
    let b = tail.as_bytes();
    if label.bytes().all(|d| d.is_ascii_digit())
        && b.len() > 3
        && b[0] == b'('
        && b[1].is_ascii_alphabetic()
        && b[2] == b')'
        && OFFICERS.is_match(&tail[3..])
        && !keys.iter().any(|k| k.item == label && OFFICERS.is_match(k.title))
    {
        let lettered = format!("{label}{}", b[1].to_ascii_uppercase() as char);
        if !keys.iter().any(|k| k.item == lettered) {
            return (lettered, tail[3..].trim_start_matches([' ', '\t', '.', ':', '-', '–', '—']));
        }
    }
    (label.to_owned(), tail)
}

/// `Item 8(A). Controls and Procedures` in a 10-KSB, `ITEM 9(A). CONTROLS AND
/// PROCEDURES` in a 10-K: a titled sub-item whose title is the standard title
/// of the lettered Item the form defines (`8A`, `9A`) is that Item. A `T`
/// after the letter (`8(A)T`, `9(A)(T)`, from the temporary Items of 2007 to
/// 2009) does not change that, and neither does a title printed on the next
/// line (`ITEM 9(A).` then `CONTROLS AND PROCEDURES`), in which case the
/// returned title is empty and the caller reads the title from that line.
/// Other titled sub-items keep their Item's number: `ITEM 1(a). GENERAL
/// DEVELOPMENT OF BUSINESS` is not Item 1A, Risk Factors. Returns the
/// lettered Item's number and title, or `None` when the heading is no such
/// Item; `end` is the end of the heading line in `text`.
fn lettered_sub_item<'a>(
    label: &str,
    tail: &'a str,
    text: &str,
    end: usize,
    keys: &[Key],
) -> Option<(String, &'a str)> {
    let b = tail.as_bytes();
    let sub_item = label.bytes().all(|d| d.is_ascii_digit())
        && b.len() >= 3
        && b[0] == b'('
        && b[1].is_ascii_alphabetic()
        && b[2] == b')';
    if !sub_item {
        return None;
    }
    let lettered = format!("{label}{}", b[1].to_ascii_uppercase() as char);
    let rest = &tail[3..];
    let rest = ["(T)", "(t)", "T"].iter().find_map(|t| rest.strip_prefix(t)).unwrap_or(rest);
    let title = rest.trim_start_matches([' ', '\t', '.', ':', '-', '–', '—']);
    let printed = if title.trim().is_empty() { next_line_title(text, end) } else { title };
    let standard = keys.iter().any(|k| k.item == lettered && same_title(&title_key(printed), &title_key(k.title)));
    standard.then_some((lettered, title))
}

fn candidates(text: &str, kind: FormKind) -> Vec<Candidate> {
    let keys = keys_for(kind);
    let re = if kind == FormKind::EightK { &*CURRENT_HEADING } else { &*PERIODIC_HEADING };
    let mut cands: Vec<Candidate> = re
        .captures_iter(text)
        .filter_map(|c| {
            let m = c.get(0).expect("group 0 always matches");
            let (number, tail) = (c.get(1).expect("Item number"), c.get(2).expect("tail"));
            let label = number.as_str().to_ascii_uppercase();
            // "Item 8, Financial Statements ...", "ITEMS 9, 9A AND 9B."
            let listed = listed_items(&label, &text[number.end()..m.end()], keys);
            // A bare sub-item ("Item 9(a).") is a heading only as a lettered
            // Item whose title is on the next line.
            let tail_ok = listed.is_some() || heading_tail_ok(after_dashes(tail.as_str()));
            if !tail_ok && (kind == FormKind::EightK || !tail.as_str().starts_with('(')) {
                return None;
            }
            if continues_sentence(text, m.start()) {
                return None;
            }
            let heading: String = m.as_str().trim().chars().take(300).collect();
            let (label, tail, joined) = if let Some((title, more)) = listed {
                (label, title, more)
            } else {
                let (label, tail) = if kind == FormKind::EightK {
                    (label, tail.as_str())
                } else {
                    let (label, tail) = split_letter(&label, &text[number.end()..tail.start()], tail.as_str());
                    let lettered = lettered_sub_item(&label, tail, text, m.end(), keys);
                    if !tail_ok && lettered.is_none() {
                        return None;
                    }
                    let (label, tail) = lettered.unwrap_or((label, tail));
                    if kind.annual() { officers_sub_item(&label, tail, keys) } else { (label, tail) }
                };
                (label, tail, JOINED.is_match(after_dashes(tail).trim_start()))
            };
            let bare = !tail.chars().any(char::is_alphabetic);
            let title_text = if bare { next_line_title(text, m.end()) } else { tail };
            let title = title_key(title_text);
            // "Item 4A" is the usual number of an annual report's executive
            // officers caption, also when its title says only "Officers" or is
            // on the lines below.
            let officers = OFFICERS.is_match(title_text) || (kind.annual() && label == "4A");
            let (part, title_part) = if kind.quarterly() {
                (part_above(text, m.start(), &label), part_by_title(&label, title_text, keys))
            } else {
                (None, None)
            };
            // A hard-wrapped cross-reference that starts with a Part I title is
            // not a heading, and must not cut the Item it sits in short.
            if title_part == Some("I") && !bare && runs_on_as_sentence(&label, tail, keys) {
                return None;
            }
            Some(Candidate {
                pos: m.start(),
                label,
                heading,
                bare,
                title,
                part,
                title_part,
                listed: false,
                officers,
                joined,
                gap: 0,
                score: 0.0,
            })
        })
        .collect();
    set_gaps(&mut cands, text.len());
    let mut cands = drop_running_headers(drop_contents_entries(cands, text, keys), keys);
    // Score by the gap to the next heading of a different Item, so repeated
    // running headers of one Item do not split its score.
    set_gaps(&mut cands, text.len());
    for c in &mut cands {
        c.score = (c.gap as f64).ln_1p();
    }
    cands
}

fn set_gaps(cands: &mut [Candidate], text_len: usize) {
    for i in 0..cands.len() {
        let next = cands[i + 1..].iter().find(|c| c.label != cands[i].label).map_or(text_len, |c| c.pos);
        cands[i].gap = next - cands[i].pos;
    }
}

fn is_item_line(line: &str) -> bool {
    line.trim_start().get(..4).is_some_and(|w| w.eq_ignore_ascii_case("item"))
}

/// Whether two title keys name the same heading: one starts the other, and
/// both have at least 5 letters.
fn same_title(a: &str, b: &str) -> bool {
    a.chars().count() >= 5 && b.chars().count() >= 5 && (a.starts_with(b) || b.starts_with(a))
}

/// The line after a bare heading ("Item 1." then "Financial Statements"),
/// unless it is another Item.
fn next_line_title(text: &str, end: usize) -> &str {
    let rest = text[end..].trim_start_matches(['\n', '\r']);
    let line = rest.split('\n').next().unwrap_or("").trim();
    if is_item_line(line) { "" } else { line }
}

/// Words that qualify the financial statements in a 10-Q Part I Item 1 title:
/// "Condensed Consolidated Financial Statements (Unaudited)".
const QUALIFIERS: &[&str] = &["unaudited", "interim", "condensed", "consolidated"];

/// Joining words. A 10-Q Part I heading can hold them besides the words of its
/// standard title and the qualifiers, and a sentence does not end with one.
const JOINING_WORDS: &[&str] =
    &["a", "about", "an", "and", "as", "at", "by", "for", "from", "in", "of", "on", "or", "the", "to", "with"];

/// Other words that a sentence does not end with but a line of running text
/// that cites an Item on the next line does: "see", "as discussed under".
const OPEN_WORDS: &[&str] = &["see", "See", "under", "within", "this"];

/// A line that holds only a PART label: `PART II,`, `Part I -`.
static PART_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ \t]*(?i:part)[ \t]+[IVX]{1,4}[ \t]*[,.:\-–—]*[ \t]*$").unwrap());

/// Whether the line just above the line that starts at `line_start` stops
/// inside a sentence, so that this line carries it on: it ends with a comma, a
/// joining word or one of `OPEN_WORDS`. A hard-wrapped cross-reference starts
/// its line that way ("The information set forth in Part II," then "Item 7.
/// Management's Discussion and Analysis, of the Company's Annual Report ...").
/// A heading comes after the end of a paragraph, a page header, a blank line
/// or a PART label, also one printed with a comma ("PART II,"). A line that
/// holds only a joining word links two headings ("Item 1. Business." then
/// "and" then "Item 2. Properties."), so the heading after it stands.
fn continues_sentence(text: &str, line_start: usize) -> bool {
    let Some(above) = text[..line_start].strip_suffix('\n') else {
        return false;
    };
    let above = above.strip_suffix('\r').unwrap_or(above);
    let line = &above[above.rfind('\n').map_or(0, |i| i + 1)..];
    if PART_LABEL.is_match(line) || JOINING_WORDS.contains(&line.trim()) {
        return false;
    }
    let last = line.trim_end().rsplit([' ', '\t']).next().unwrap_or("");
    let word = last.trim_start_matches(['(', '"', '“']);
    last.ends_with(',') || JOINING_WORDS.contains(&word) || OPEN_WORDS.contains(&word)
}

/// Lowercase words that a heading does not put after its title, but a
/// sentence that cites the Item does: "Item 1. Financial Statements of this
/// report is hereby updated". Separated by spaces.
const REFERENCE_WORDS: &str =
    "this report hereby herein see refer is are was were under included contained above below";

/// Capitalized words with which a sentence names a place in the report ("in
/// Part I of this report").
const PLACE_WORDS: &[&str] = &["part", "item", "items", "note", "notes", "i", "ii"];

/// The title without its leading qualifiers: "Unaudited Condensed
/// Consolidated Financial Statements" reads "Financial Statements".
fn without_qualifiers(title: &str) -> &str {
    let mut t = title;
    loop {
        let rest = &t[t.find(char::is_alphabetic).unwrap_or(t.len())..];
        let end = rest.find(|c: char| !c.is_alphabetic()).unwrap_or(rest.len());
        if end == 0 || !QUALIFIERS.iter().any(|q| q.eq_ignore_ascii_case(&rest[..end])) {
            return t;
        }
        t = &rest[end..];
    }
}

/// The 10-Q or 10-QSB part whose standard title for Item `label` matches the
/// title text, when exactly one part's does. Leading qualifiers do not count.
fn part_by_title(label: &str, title: &str, keys: &[Key]) -> Option<&'static str> {
    let title = title_key(without_qualifiers(title));
    let mut parts = keys.iter().filter(|k| k.item == label && same_title(&title, &title_key(k.title))).map(|k| k.part);
    match (parts.next(), parts.next()) {
        (Some(part), None) => part,
        _ => None,
    }
}

/// The words of a title, with apostrophes inside them ("Management's").
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphabetic() && c != '\'' && c != '’')
        .map(|w| w.trim_matches(['\'', '’']))
        .filter(|w| !w.is_empty())
}

/// A word as compared with the words of a standard title: lowercase, without
/// a possessive or a plural `s` ("Disclosures" and "disclosure" are the same
/// word, and so are "Company's" and "company").
fn word_stem(word: &str) -> String {
    let w = word.to_lowercase();
    let w = w.strip_suffix("'s").or_else(|| w.strip_suffix("’s")).unwrap_or(&w);
    w.strip_suffix('s').unwrap_or(w).to_owned()
}

/// Whether a title that starts with the Part I title of Item `label` runs on
/// as a sentence that cites the Item, as a hard-wrapped cross-reference
/// does. Up to the first period, colon, semicolon, bracket, dash or tab, the
/// words of that title, the qualifiers, joining words and
/// `PLACE_WORDS` lead to a lowercase reference word ("Item 1. Financial
/// Statements in Part I of this report"), or to a comma followed by "Note" or
/// by a lowercase word that is not a word of the title, a qualifier or
/// "continued" ("Item 1. Financial Statements, of this report", "Item 2.
/// Management's Discussion and Analysis, under ..."). Any other word ends the
/// test: a heading may go on with words of its own ("Item 1. Financial
/// statements for the thirteen weeks ended May 2, 2020", "Item 3. Quantitative
/// and Qualitative Disclosures About the Company's Market Risk", "Item 3. ...
/// Market Risk Not applicable.", "Item 3. ... Market Risk The Company is not
/// exposed ...").
fn runs_on_as_sentence(label: &str, title: &str, keys: &[Key]) -> bool {
    let title_words: Vec<String> = keys
        .iter()
        .filter(|k| k.part == Some("I") && k.item == label)
        .flat_map(|k| words(k.title))
        .map(word_stem)
        .collect();
    let in_title = |w: &str| title_words.contains(&word_stem(w));
    let qualifier = |w: &str| QUALIFIERS.iter().any(|q| q.eq_ignore_ascii_case(w));
    let end = title.find(['.', ':', ';', '(', '[', '-', '–', '—', '\t']).unwrap_or(title.len());
    for (i, clause) in title[..end].split(',').enumerate() {
        let mut ws = words(clause).peekable();
        if i > 0
            && let Some(&w) = ws.peek()
            && (w.eq_ignore_ascii_case("note")
                || w.eq_ignore_ascii_case("notes")
                || (w.starts_with(char::is_lowercase) && !in_title(w) && !qualifier(w) && w != "continued"))
        {
            return true;
        }
        for w in ws {
            if REFERENCE_WORDS.split(' ').any(|r| r == w) {
                return true;
            }
            let place = w.starts_with(char::is_uppercase) && PLACE_WORDS.iter().any(|p| p.eq_ignore_ascii_case(w));
            if !(in_title(w) || qualifier(w) || place || JOINING_WORDS.iter().any(|j| j.eq_ignore_ascii_case(w))) {
                return false;
            }
        }
    }
    false
}

/// The Item number of a bare heading line such as "Item 7".
fn bare_item_label(line: &str) -> Option<String> {
    let c = PERIODIC_HEADING.captures(line)?;
    (!c[2].chars().any(char::is_alphabetic)).then(|| c[1].to_ascii_uppercase())
}

/// Part named at the start of the heading line or on one of the two lines
/// above it (a running header such as the company name can sit between).
/// The search stops at the line of another Item, whose PART line it would
/// be; a bare running header of this Item ("PART I", "Item 2", then
/// "ITEM 2. MANAGEMENT'S ...") does not stop it.
fn part_above(text: &str, start: usize, label: &str) -> Option<&'static str> {
    let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    let above = text[..start]
        .trim_end_matches(['\n', '\r'])
        .rsplit('\n')
        .take(2)
        .take_while(|l| !PERIODIC_HEADING.is_match(l) || bare_item_label(l).as_deref() == Some(label));
    std::iter::once(&text[start..line_end])
        .chain(above)
        .find_map(|l| PART_MARK.captures(l))
        .map(|c| if &c[1] == "II" { "II" } else { "I" })
}

/// Drop table-of-contents entries whose heading reappears in the body, and
/// lists of Item headings printed again after the body.
///
/// Two candidates are the same heading when they share Item and title and
/// other Items come between them. The earlier copy is a contents entry when
/// it starts the shorter stretch of text; the later copy is a listing when a
/// signatures heading comes between them, at or after the first body heading
/// (contents often list "Signatures" too). In a 10-Q this matters most: a
/// contents entry for Part I Item 2 could otherwise be taken as Part I and the
/// body heading as Part II. Entries just before a dropped contents entry
/// belong to the same contents block and go too (an Item listed in the
/// contents whose body has no heading). Body headings stay: an entry goes only
/// when the candidate before it is a contents entry too, or when it comes
/// before the first body heading that a contents entry names and no Item
/// earlier in the form's order with text of its own comes before it. It never
/// goes when it is the later copy of a heading printed twice. Short entries
/// just after a dropped listing entry go with it. The body headings of dropped
/// contents entries are marked `listed`.
fn drop_contents_entries(cands: Vec<Candidate>, text: &str, keys: &[Key]) -> Vec<Candidate> {
    const SAME_BLOCK: usize = 2000;
    let n = cands.len();
    // A contents entry, the body heading it was matched with, and pairs whose
    // later copy is no longer than the earlier one.
    let mut contents = vec![false; n];
    let mut body = vec![false; n];
    // The later copy of any matched pair, which the spread below must not take.
    let mut paired = vec![false; n];
    let mut later_copies: Vec<(usize, usize)> = Vec::new();
    for a in 0..n {
        if cands[a].title.chars().count() < 5 {
            continue;
        }
        let mut other_between = false;
        for b in a + 1..n {
            if cands[b].label != cands[a].label {
                other_between = true;
            } else if other_between && same_title(&cands[a].title, &cands[b].title) {
                if cands[b].gap > cands[a].gap {
                    contents[a] = true;
                    body[b] = true;
                } else {
                    later_copies.push((a, b));
                }
                paired[b] = true;
                break;
            }
        }
    }
    // A body heading is never a contents entry or reached by the spread below
    // (it is the later copy of a pair), so this is where the body starts.
    let body_start = (0..n).find(|&i| body[i] && !contents[i]).map(|i| cands[i].pos);
    // Spread to the other entries of a contents block: mark the runs of close
    // candidates that lead up to a contents entry.
    let mut leads = contents.clone();
    for i in (0..n.saturating_sub(1)).rev() {
        if leads[i + 1] && !paired[i] && cands[i + 1].pos - cands[i].pos <= SAME_BLOCK {
            leads[i] = true;
        }
    }
    // Then drop those that follow a contents entry, and those before the body
    // unless an Item earlier in the form's order, with text of its own, comes
    // before them. A short body Item just before a second contents list or an
    // index of Items stays. An Item line above the contents that names a later
    // Item (a 10-Q/A's explanatory note) does not keep the contents rows after
    // it.
    let form_order = |c: &Candidate| {
        let part = c.part.or(c.title_part);
        let mut fits =
            (0..keys.len()).filter(|&k| keys[k].item == c.label && part.is_none_or(|p| keys[k].part == Some(p)));
        let first = fits.next();
        (first, fits.next_back().or(first))
    };
    // The earliest form position of an Item with text of its own so far, and
    // whether one so far is not an Item of the form.
    let mut earliest: Option<usize> = None;
    let mut undefined = false;
    for i in 0..n {
        let (first, last) = form_order(&cands[i]);
        if leads[i] {
            let follows_item = undefined || earliest.is_some_and(|e| last.is_none_or(|l| e < l));
            if (i > 0 && contents[i - 1]) || (!follows_item && body_start.is_some_and(|b| cands[i].pos < b)) {
                contents[i] = true;
            }
        } else if cands[i].gap > SAME_BLOCK {
            match first {
                Some(f) => earliest = Some(earliest.map_or(f, |e| e.min(f))),
                None => undefined = true,
            }
        }
    }
    let mut after = vec![false; n];
    if !later_copies.is_empty() {
        let body_start = body_start.unwrap_or(0);
        let sigs = signature_lines(text);
        for (a, b) in later_copies {
            let from = body_start.max(cands[a].pos + 1);
            after[b] = sigs.iter().any(|&p| p >= from && p < cands[b].pos);
        }
    }
    // Only short entries: a body heading can follow a second contents table.
    for i in 1..n {
        if after[i - 1] && cands[i].pos - cands[i - 1].pos <= SAME_BLOCK && cands[i].gap <= SAME_BLOCK {
            after[i] = true;
        }
    }
    cands
        .into_iter()
        .enumerate()
        .filter(|&(i, _)| !contents[i] && !after[i])
        .map(|(i, c)| Candidate { listed: body[i], ..c })
        .collect()
}

/// Drop a bare running page header ("Item 7") that comes just before the
/// titled heading of the same Item ("ITEM 7. MANAGEMENT'S DISCUSSION ...").
/// A bare heading whose next line is that title ("ITEM 1." then "BUSINESS"),
/// or the Item's standard title, is a real heading and stays.
fn drop_running_headers(cands: Vec<Candidate>, keys: &[Key]) -> Vec<Candidate> {
    const NEAR: usize = 5000;
    let keep: Vec<bool> = (0..cands.len())
        .map(|i| {
            let c = &cands[i];
            let Some(next) = cands.get(i + 1) else { return true };
            let header = c.bare && !next.bare && next.label == c.label && next.pos - c.pos <= NEAR;
            !header
                || same_title(&c.title, &next.title)
                || keys.iter().any(|k| k.item == c.label && same_title(&c.title, &title_key(k.title)))
        })
        .collect();
    cands.into_iter().zip(keep).filter(|(_, k)| *k).map(|(c, _)| c).collect()
}

/// Start of a separately paginated document attached after the last Item,
/// such as a full annual report: a table of contents of at least 8
/// page-numbered entries spanning 30 or more pages, followed by much more
/// text than the Item itself. The heading line just before the contents
/// ("Financial:") goes with the attached document.
fn attached_document(text: &str, from: usize, to: usize) -> Option<usize> {
    const MIN_ENTRIES: usize = 8;
    const MIN_PAGES: u32 = 30;
    const MAX_GAP_LINES: usize = 2;
    let mut pos = from;
    let mut prev_line: Option<(usize, &str)> = None;
    // (block start, first page, last page, entries)
    let mut block: Option<(usize, u32, u32, usize)> = None;
    let mut misses = 0;
    for line in text[from..to].split_inclusive('\n') {
        let start = pos;
        pos += line.len();
        let trimmed = line.trim_end();
        let page = TOC_ENTRY
            .captures(trimmed)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .and_then(|m| m.as_str().parse::<u32>().ok());
        match page {
            Some(p) => {
                misses = 0;
                block = match block {
                    Some((s, first, last, n)) if p >= last => Some((s, first, p, n + 1)),
                    _ => {
                        let s = match prev_line {
                            Some((ps, pl)) if !pl.is_empty() && pl.chars().count() <= 40 => ps,
                            _ => start,
                        };
                        Some((s, p, p, 1))
                    }
                };
                if let Some((s, first, last, n)) = block {
                    let after = to - s;
                    if n >= MIN_ENTRIES && last - first >= MIN_PAGES && after >= 50_000 && after >= 4 * (s - from) {
                        return Some(s);
                    }
                }
            }
            None => {
                misses += 1;
                if misses > MAX_GAP_LINES {
                    block = None;
                }
            }
        }
        prev_line = Some((start, trimmed));
    }
    None
}

/// Where the signatures start after `from`: a signatures heading, or the
/// signature block's opening sentence when there is no heading.
fn signatures_after(text: &str, from: usize) -> Option<usize> {
    signature_lines(&text[from..])
        .first()
        .map(|&p| from + p)
        .or_else(|| SIGNATURE_BLOCK.find_at(text, from).map(|m| m.start()))
}

/// Start of the first line in a section, after its heading line, that `re`
/// matches (a block or glossary heading). The line must start with a capital
/// letter, so a wrapped sentence ("report of independent accountants on the
/// 1997 statements ...") does not count.
fn heading_start(re: &Regex, text: &str, start: usize, end: usize) -> Option<usize> {
    let from = text[start..end].find('\n').map_or(end, |i| start + i + 1);
    re.find_iter(&text[from..end])
        .map(|m| from + m.start())
        .find(|&p| text[p..].trim_start_matches([' ', '\t']).starts_with(char::is_uppercase))
}

/// Start of financial statements that the exhibits Item prints after its own
/// text with pages numbered apart from the report's (F-1, F-2, ...). Such
/// pages are a separately paginated part of the report and belong to no Item.
/// They are found by an index of at least three entries with F-page numbers,
/// followed later in the Item by heading lines with the titles of at least
/// two of its entries. The Item ends at the index when its heading ("Index to
/// Financial Statements") opens it and the first statement follows its last
/// entry, as on a page F-1, and otherwise at the first statement, so an index
/// printed as the Item's list of statements stays in the Item. The Item's own
/// text after the statements keeps them in the Item, which then prints them
/// in its middle: an exhibit index, an exhibit list's caption ("(a)(3)
/// Exhibits:", "3. Exhibits", "LIST OF EXHIBITS") or table header ("Exhibit
/// No."), or the caption of another of its parts ("(b) Reports on Form 8-K",
/// "2. Financial Statement Schedules").
fn separate_statements(text: &str, start: usize, end: usize) -> Option<usize> {
    const MIN_ENTRIES: usize = 3;
    // Other lines allowed between two entries, or between an index and its
    // heading.
    const MAX_GAP_LINES: usize = 2;
    const MAX_HEADING_CHARS: usize = 100;
    let from = text[start..end].find('\n').map_or(end, |i| start + i + 1);
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut pos = from;
    for line in text[from..end].split_inclusive('\n') {
        lines.push((pos, line.trim()));
        pos += line.len();
    }
    // Title keys of the first index's entries, and its first and last lines.
    let mut titles: Vec<String> = Vec::new();
    let (mut first, mut last) = (0, 0);
    for (i, &(_, line)) in lines.iter().enumerate() {
        let Some(m) = F_PAGE_ENTRY.find(line) else { continue };
        if titles.is_empty() || i - last > MAX_GAP_LINES + 1 {
            if titles.len() >= MIN_ENTRIES {
                break;
            }
            titles.clear();
            first = i;
        }
        // A page alone on its line belongs to the title above it.
        let title = title_key(&line[..m.start()]);
        let title = if title.is_empty() && i > 0 { title_key(lines[i - 1].1) } else { title };
        titles.push(title);
        last = i;
    }
    if titles.len() < MIN_ENTRIES {
        return None;
    }
    let is_heading = |line: &str| {
        line.len() <= MAX_HEADING_CHARS && line.starts_with(char::is_uppercase) && !PAGE_REFERENCE.is_match(line)
    };
    // Heading lines after the index, then the first one for each entry's title.
    let headings: Vec<(usize, String)> =
        (last + 1..lines.len()).filter(|&j| is_heading(lines[j].1)).map(|j| (j, title_key(lines[j].1))).collect();
    let mut printed: Vec<usize> =
        titles.iter().filter_map(|t| headings.iter().find(|(_, h)| same_title(h, t)).map(|&(j, _)| j)).collect();
    printed.sort_unstable();
    printed.dedup();
    let &statements = printed.first()?;
    // The Item's own text after the statements keeps them in the Item, which
    // then prints them in its middle: its exhibit index, its exhibit list's
    // caption or header, or the caption of another of its parts.
    let own_text_after = lines[statements + 1..].iter().any(|&(_, line)| {
        EXHIBITS_ITEM_PART.is_match(line)
            || (line.starts_with(char::is_uppercase) && EXHIBITS_INDEX_HEADING.is_match(line))
    });
    if printed.len() < 2 || own_text_after {
        return None;
    }
    let opens =
        (first.saturating_sub(MAX_GAP_LINES + 1)..first).find(|&i| STATEMENTS_INDEX_HEADING.is_match(lines[i].1));
    match opens {
        Some(i) if statements == last + 1 => Some(lines[i].0),
        _ => Some(lines[statements].0),
    }
}

/// Some 10-Qs print Items out of form order: banks often put the financial
/// statements (Part I Item 1) after MD&A. The ordered chain cannot hold such a
/// heading, so it is added afterwards when its part is known, its Item is
/// still unused, it starts a real stretch of text rather than a contents
/// entry, and it comes before the signatures. It must also come after the
/// chain's first heading or after the first body heading that a contents
/// entry names: when the chain keeps Item 1 printed after Item 2, Item 2
/// comes before the chain. A `PART` line gives the part, and so does a title
/// that only Part I uses for that Item number. A Part II title does not: Part
/// II Items 1A (Risk Factors) and 4 (Mine Safety Disclosures) share number and
/// title with 10-K Items that 10-Qs cite.
fn add_out_of_order(text: &str, cands: &[Candidate], keys: &[Key], picks: &mut Vec<(usize, usize)>) {
    const MIN_GAP: usize = 1000;
    let first = cands[picks[0].0].pos;
    let from = cands.iter().find(|c| c.listed).map_or(first, |c| c.pos.min(first));
    let last = picks.iter().map(|&(c, _)| cands[c].pos).max().unwrap_or(first);
    let limit = signatures_after(text, last).unwrap_or(text.len());
    let mut extra: Vec<(usize, usize)> = Vec::new();
    for (ci, c) in cands.iter().enumerate() {
        let Some(part) = c.part.or(c.title_part.filter(|&p| p == "I")) else { continue };
        if c.gap < MIN_GAP || c.pos < from || c.pos >= limit || picks.iter().any(|&(p, _)| p == ci) {
            continue;
        }
        let Some(ki) = keys.iter().position(|k| k.item == c.label && k.part == Some(part)) else { continue };
        if picks.iter().chain(&extra).any(|&(_, k)| k == ki) {
            continue;
        }
        extra.push((ci, ki));
    }
    picks.extend(extra);
    picks.sort_by_key(|&(c, _)| cands[c].pos);
}

/// The Item number without its letter: `5` for `5A`.
fn item_number(label: &str) -> &str {
    label.trim_end_matches(|c: char| c.is_ascii_alphabetic())
}

/// Some filings print an Item only as lettered headings the form does not
/// define: "Item 5A. Market for Registrant's Common Equity" and "Item 5B.
/// Equity Compensation Plan Information" for Item 5. The chain cannot hold
/// them, so the first one that sits where the Item of its number belongs,
/// between the chain's Items before and after it in form order, starts that
/// Item when the chain has none. In a 10-Q a `PART` line above the heading
/// picks the part, and so does the heading's title when it is one part's
/// standard title for that number; otherwise the earlier part in the slot
/// wins. Later lettered headings of that number stay inside the Item, unless
/// in a 10-Q their `PART` line, their title or else the Item before them puts
/// them in the other part (Part II's "Item 2A. Unregistered Sales ..." after
/// Part I's "Item 2A. Management's Discussion ..."). The executive officers
/// caption ("Item 4A. Executive Officers") is not an Item and never does.
/// Returns the Items filled this way.
fn add_lettered(cands: &[Candidate], keys: &[Key], picks: &mut Vec<(usize, usize)>) -> Vec<&'static str> {
    // Items filled here, with their part.
    let mut filled: Vec<(&str, Option<&str>)> = Vec::new();
    for (ci, c) in cands.iter().enumerate() {
        let number = item_number(&c.label);
        if number.len() == c.label.len() || c.officers || keys.iter().any(|k| k.item == c.label) {
            continue;
        }
        let at = picks.partition_point(|&(p, _)| cands[p].pos < c.pos);
        if filled.iter().any(|&(n, _)| n == number) {
            let mut titled =
                keys.iter().filter(|k| k.item == number && same_title(&c.title, &title_key(k.title))).map(|k| k.part);
            let title_part = match (titled.next(), titled.next()) {
                (Some(part), None) => part,
                _ => None,
            };
            let part = c.part.or(title_part).or_else(|| at.checked_sub(1).and_then(|i| keys[picks[i].1].part));
            if filled.iter().any(|&(n, p)| n == number && (part.is_none() || p == part)) {
                continue;
            }
        }
        let (Some(&(_, before)), Some(&(_, after))) = (at.checked_sub(1).and_then(|i| picks.get(i)), picks.get(at))
        else {
            continue;
        };
        let slot: Vec<usize> = (before + 1..after)
            .filter(|&ki| keys[ki].item == number && c.part.is_none_or(|p| keys[ki].part == Some(p)))
            .collect();
        let by_title = slot.iter().copied().find(|&ki| same_title(&c.title, &title_key(keys[ki].title)));
        let Some(ki) = by_title.or(slot.first().copied()) else { continue };
        if picks.iter().any(|&(_, k)| k == ki) {
            continue;
        }
        picks.insert(at, (ci, ki));
        filled.push((keys[ki].item, keys[ki].part));
    }
    filled.into_iter().map(|(item, _)| item).collect()
}

/// Order chain states by total score. On a tie the earlier Item wins, so an
/// unmarked 10-Q "Item 2" between Part I Item 1 and Part II Item 6 stays in Part I.
fn better(a_total: f64, a_key: usize, b_total: f64, b_key: usize) -> std::cmp::Ordering {
    a_total.total_cmp(&b_total).then(b_key.cmp(&a_key))
}

/// How many of the Items in `keys` the candidates print with a number and
/// title that fit `keys` and not `other`: the title is the standard title
/// that `keys` gives to that number and to no other, and that `other` gives
/// to another number and not to this one. "Item 7. Financial Statements"
/// fits Form 10-KSB and "Item 8. Financial Statements" Form 10-K. "Item 3.
/// Legal Proceedings" fits both, and these tell nothing either: "Item 4.
/// Submission of Matters to a Vote of Security Holders", which 10-Ks printed
/// before 2010, and "Item 15. Principal Accountant Fees and Services", which
/// 10-Ks for 2003 printed too. Each Item counts once, however often it is
/// printed.
fn numbering_fit(cands: &[Candidate], keys: &[Key], other: &[Key]) -> usize {
    keys.iter()
        .filter(|k| {
            let title = title_key(k.title);
            // The numbers a list gives this title to.
            let numbers = |list: &[Key]| -> Vec<&'static str> {
                list.iter().filter(|o| same_title(&title, &title_key(o.title))).map(|o| o.item).collect()
            };
            let theirs = numbers(other);
            numbers(keys).iter().all(|&n| n == k.item)
                && theirs.iter().any(|&n| n != k.item)
                && !theirs.contains(&k.item)
                && cands.iter().any(|c| c.label == k.item && same_title(&c.title, &title))
        })
        .count()
}

/// Split filing text (from [`crate::html_to_text`]) into Item sections, in document order.
pub fn extract_sections(text: &str, kind: FormKind) -> Vec<Section> {
    let cands = candidates(text, kind);
    // A filing that prints the Items of the form its kind replaced or was
    // replaced by (a 10-KSB numbered as a 10-K, a 10-Q numbered as a 10-QSB)
    // is split with that form's Items when its headings fit them better.
    let (kind, cands) = match kind.other_numbering() {
        Some(other)
            if numbering_fit(&cands, keys_for(other), keys_for(kind))
                > numbering_fit(&cands, keys_for(kind), keys_for(other)) =>
        {
            (other, candidates(text, other))
        }
        _ => (kind, cands),
    };
    let keys = keys_for(kind);

    // Best chain where both candidate position and Item order increase.
    struct State {
        cand: usize,
        key: usize,
        total: f64,
        prev: Option<usize>,
    }
    let mut states: Vec<State> = Vec::new();
    for (ci, cand) in cands.iter().enumerate() {
        // A PART line fixes the part; otherwise a title that names one part does.
        let part = cand.part.or(cand.title_part);
        let fits = |key: &Key| key.item == cand.label && (part.is_none() || key.part == part);
        for (ki, _) in keys.iter().enumerate().filter(|(_, key)| fits(key)) {
            let best = states
                .iter()
                .enumerate()
                .filter(|(_, s)| s.cand < ci && s.key < ki)
                .max_by(|(_, a), (_, b)| better(a.total, a.key, b.total, b.key));
            states.push(State {
                cand: ci,
                key: ki,
                total: cand.score + best.map_or(0.0, |(_, s)| s.total),
                prev: best.map(|(i, _)| i),
            });
        }
    }
    let Some(mut cur) =
        states.iter().enumerate().max_by(|(_, a), (_, b)| better(a.total, a.key, b.total, b.key)).map(|(i, _)| i)
    else {
        return Vec::new();
    };
    let mut chain = vec![cur];
    while let Some(p) = states[cur].prev {
        chain.push(p);
        cur = p;
    }
    chain.reverse();
    // (candidate, key) pairs in document order.
    let mut picks: Vec<(usize, usize)> = chain.iter().map(|&s| (states[s].cand, states[s].key)).collect();
    if kind.quarterly() {
        add_out_of_order(text, &cands, keys, &mut picks);
    }
    let filled = add_lettered(&cands, keys, &mut picks);

    let starts: Vec<usize> = picks.iter().map(|&(c, _)| cands[c].pos).collect();
    let last_start = *starts.last().expect("chain is non-empty");
    let doc_end = signatures_after(text, last_start).unwrap_or(text.len());
    let doc_end = attached_document(text, last_start, doc_end).unwrap_or(doc_end);
    // In an annual report, titled headings of Items the form does not define
    // ("Item 4A. Executive Officers") end the section before them, except the
    // lettered headings of an Item filled from them ("Item 5B" inside Item 5).
    let stops: Vec<usize> = if kind.annual() {
        cands
            .iter()
            .filter(|c| {
                !c.bare
                    && !keys.iter().any(|k| k.item == c.label)
                    && (c.officers || !filled.contains(&item_number(&c.label)))
            })
            .map(|c| c.pos)
            .collect()
    } else {
        Vec::new()
    };
    // Text with no Item heading of its own belongs to no Item. A block heading
    // ends the Items that never hold financial statements or an exhibit
    // index: changes in accountants (10-K Item 9, 10-KSB Item 8) and the Items
    // after the exhibits (10-K Item 16, 10-KSB Items 14 and 15), which a
    // glossary heading ends too. It also ends 10-K Item 7A when no Item 8
    // heading was found: the statements after it are then Item 8's text
    // without its heading. A 10-KSB's statements (Item 7) follow MD&A (Item
    // 6), which keeps them like 10-K Item 7 does. The exhibits Item keeps its
    // block headings but not statements with pages of their own numbering
    // printed after all of its own text.
    let no_item_8 = !picks.iter().any(|&(_, k)| keys[k].item == "8");
    // The Item that holds the financial statements: 10-K Item 8, 10-KSB Item 7.
    let statements_key = keys.iter().position(|k| k.title.starts_with("Financial Statements"));
    // The exhibits Item: 10-K Item 15, 10-KSB Item 13, or an Item after the
    // statements Item printed with a title that names the exhibits or the
    // financial statements (Item 14 of 10-Ks before 2003, Item 16 of some for
    // 2003). An Item before it is never the exhibits Item, also when printed
    // as "Financial Statements" (Item 7 of a 10-K numbered as a 10-KSB).
    let exhibits_item = |ki: usize, cand: &Candidate| {
        kind.annual()
            && (keys[ki].title.contains("Exhibits")
                || (statements_key.is_some_and(|s| ki > s)
                    && (cand.title.contains("exhibit") || cand.title.contains("financialstatement"))))
    };
    let after_exhibits = |ki: usize, cand: &Candidate| {
        matches!((kind, keys[ki].item), (FormKind::TenK, "16") | (FormKind::TenKsb, "14" | "15"))
            && !exhibits_item(ki, cand)
    };
    let ends_at_block = |ki: usize, cand: &Candidate| match (kind, keys[ki].item) {
        (FormKind::TenK, "9") | (FormKind::TenKsb, "8") => true,
        (FormKind::TenK, "7A") => no_item_8,
        _ => after_exhibits(ki, cand),
    };
    // A section that holds the next Item too keeps its block headings: its
    // heading names a second Item ("Items 9 and 9A."), or a heading with the
    // same number and another title follows ("ITEM 9 A – CONTROLS AND
    // PROCEDURES" reads as Item 9).
    let holds_more = |ci: usize, end: usize| {
        let c = &cands[ci];
        c.joined
            || cands.iter().any(|o| {
                o.pos > c.pos && o.pos < end && !o.bare && o.label == c.label && !same_title(&o.title, &c.title)
            })
    };

    picks
        .iter()
        .enumerate()
        .map(|(i, &(ci, ki))| {
            let key = &keys[ki];
            let cand = &cands[ci];
            let start = cand.pos;
            let next = starts.get(i + 1).copied().unwrap_or(doc_end);
            let end = stops.iter().copied().find(|&p| p > start && p < next).unwrap_or(next).max(start);
            let end = if ends_at_block(ki, cand) && !holds_more(ci, end) {
                let block = heading_start(&BLOCK_HEADING, text, start, end);
                let glossary = after_exhibits(ki, cand).then(|| heading_start(&GLOSSARY, text, start, end)).flatten();
                block.into_iter().chain(glossary).min().unwrap_or(end)
            } else if exhibits_item(ki, cand) {
                separate_statements(text, start, end).unwrap_or(end)
            } else {
                end
            };
            let item_id = key.item.to_ascii_lowercase();
            let id = match key.part {
                Some(part) => format!("part_{}_item_{item_id}", if part == "I" { 1 } else { 2 }),
                None => format!("item_{item_id}"),
            };
            Section {
                id,
                part: key.part.map(str::to_owned),
                item: key.item.to_owned(),
                title: key.title.to_owned(),
                heading: cand.heading.clone(),
                start,
                end,
                text: text[start..end].trim().to_owned(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(sections: &[Section]) -> Vec<&str> {
        sections.iter().map(|s| s.id.as_str()).collect()
    }

    const BODY: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor. ";

    #[test]
    fn form_kinds() {
        assert_eq!(FormKind::from_form("10-K"), Some(FormKind::TenK));
        assert_eq!(FormKind::from_form("10-K405"), Some(FormKind::TenK));
        assert_eq!(FormKind::from_form("10-q/a"), Some(FormKind::TenQ));
        assert_eq!(FormKind::from_form("8-K12B"), Some(FormKind::EightK));
        assert_eq!(FormKind::from_form("S-1"), None);
        // EDGAR's indexes spell the small business forms without the hyphen.
        for form in ["10KSB", "10KSB/A", "10KSB40", "10-KSB", "10-KSB405", "10-ksb/a"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenKsb), "{form}");
        }
        for form in ["10QSB", "10QSB/A", "10-QSB", "10-qsb/A"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenQsb), "{form}");
        }
        for form in ["10KT405", "10-KT", "10-K405/A", "10-KS"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenK), "{form}");
        }
        for form in ["10-QT", "10-Q/A", "10-QS"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenQ), "{form}");
        }
        for form in ["18-K", "NT 10-K", "NTN 10K", "NTN 10Q", "10-12B", "8-A12B"] {
            assert_eq!(FormKind::from_form(form), None, "{form}");
        }
    }

    /// FormKind::FORMS, which messages in the library, the CLI, the MCP server
    /// and the Python package print, names each kind once.
    #[test]
    fn forms_names_every_kind() {
        let names: Vec<&str> =
            FormKind::FORMS.split([',', ' ']).filter(|w| !w.is_empty() && *w != "or" && *w != "and").collect();
        let kinds: Vec<FormKind> =
            names.iter().map(|name| FormKind::from_form(name).unwrap_or_else(|| panic!("{name}"))).collect();
        let every = [FormKind::TenK, FormKind::TenQ, FormKind::EightK, FormKind::TenKsb, FormKind::TenQsb];
        // This match names every kind, so a kind added to FormKind does not
        // compile here until it is added to the match, to `every` and to FORMS.
        for kind in every {
            match kind {
                FormKind::TenK | FormKind::TenQ | FormKind::EightK | FormKind::TenKsb | FormKind::TenQsb => {}
            }
        }
        assert_eq!(kinds, every, "{names:?}");
    }

    #[test]
    fn ten_k_skips_table_of_contents() {
        let body = BODY.repeat(40);
        let text = format!(
            "Apple Inc.\nFORM 10-K\nTABLE OF CONTENTS\nItem 1.\tBusiness\t1\nItem 1A.\tRisk Factors\t5\n\
             Item 1B.\tUnresolved Staff Comments\t17\nItem 7.\tManagement's Discussion\t20\n\
             PART I\nItem 1. Business\n{body}\nItem 1A. Risk Factors\n{body}\n\
             Investors should read Item 7 of this report.\n\
             Item 1B. Unresolved Staff Comments\nNone.\n\
             PART II\nITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\nSIGNATURES\nPursuant to..."
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_1b", "item_7"]);
        assert!(s[0].text.starts_with("Item 1. Business\nLorem"));
        assert!(s[1].text.contains("read Item 7 of this report"));
        assert_eq!(s[2].text, "Item 1B. Unresolved Staff Comments\nNone.\nPART II");
        assert!(!s[3].text.contains("SIGNATURES"));
        assert_eq!(s[3].heading, "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS");
    }

    #[test]
    fn running_headers_do_not_split_items() {
        let body = BODY.repeat(20);
        let text =
            format!("Item 1. Business\n{body}\nItem 1. Business (continued)\n{body}\nItem 1A. Risk Factors\n{body}\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a"]);
        assert!(s[0].text.contains("(continued)"));
    }

    #[test]
    fn ten_q_parts() {
        let body = BODY.repeat(20);
        let text = format!(
            "INDEX\nPART I\nItem 1. Financial Statements\t3\nItem 2. Management's Discussion\t20\nPART II\n\
             Item 1. Legal Proceedings\t30\nItem 1A. Risk Factors\t31\nItem 6. Exhibits\t33\n\
             PART I — FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}{body}{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}{body}\n\
             PART II — OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 1A. Risk Factors\n{body}\n\
             Item 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]);
        assert_eq!(s[2].part.as_deref(), Some("II"));
        assert_eq!(s[2].title, "Legal Proceedings");
    }

    #[test]
    fn eight_k_items() {
        let text = "FORM 8-K\nItem 2.02 Results of Operations and Financial Condition.\nOn May 2, the Company...\n\
                    Item 9.01 Financial Statements and Exhibits.\n(d) Exhibits\nSIGNATURE\nPursuant to...";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_9.01"]);
        assert_eq!(s[1].text, "Item 9.01 Financial Statements and Exhibits.\n(d) Exhibits");
    }

    #[test]
    fn no_headings_no_sections() {
        assert!(extract_sections("Nothing to see here.", FormKind::TenK).is_empty());
    }

    #[test]
    fn ten_q_contents_with_titles_on_next_line() {
        // Without matching the contents to the body, the contents would take
        // Part I and the body headings Part II.
        let body = BODY.repeat(30);
        let text = format!(
            "CONTENTS\nItem 1.\nFinancial Statements\t3\nItem 2.\nManagement's Discussion\t30\n\
             Item 1.\nLegal Proceedings\t60\nItem 6.\nExhibits\t61\n\
             ITEM 1. FINANCIAL STATEMENTS\n{body}{body}\n\
             ITEM 2. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}{body}\n\
             OTHER INFORMATION\nITEM 1. LEGAL PROCEEDINGS\nNone.\nITEM 6. EXHIBITS\n{body}\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[0].heading, "ITEM 1. FINANCIAL STATEMENTS");
    }

    #[test]
    fn ten_q_items_out_of_order() {
        let body = BODY.repeat(30);
        let text = format!(
            "Item 2. Management's Discussion and Analysis\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             Part I. Financial Information\nItem 1. Financial Statements\n{body}\n\
             Part II. Other Information\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\nList\nSignature\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_4", "part_1_item_1", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.ends_with("Part I. Financial Information"));
    }

    #[test]
    fn ten_k_page_headers_sub_items_and_odd_items() {
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1\nForward-looking statements.\nITEM 1. BUSINESS\n{body}\n\
             ITEM 4. MINE SAFETY DISCLOSURES\nNot applicable.\nITEM 4A. EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\n\
             Item 9 A. Controls and Procedures\n{body}\nItem 14. Exhibits\n(a) Documents filed\n\
             S I G N A T U R E S\nPursuant to...\n{body}\nITEM 14(d)\nFINANCIAL STATEMENT SCHEDULE\n{body}{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_4", "item_9a", "item_14"]);
        assert_eq!(s[0].heading, "ITEM 1. BUSINESS");
        assert_eq!(s[1].text, "ITEM 4. MINE SAFETY DISCLOSURES\nNot applicable.");
        assert_eq!(s[3].text, "Item 14. Exhibits\n(a) Documents filed");
    }

    #[test]
    fn item_list_after_signatures_ignored() {
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\n{body}{body}\nItem 2. Management's Discussion\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nItem 1. Legal Proceedings\nNone.\nItem 6. Exhibits\nList\n\
             SIGNATURE\nPursuant to...\nEXHIBIT INDEX\nItem 1. Financial Statements\nItem 2. Management's Discussion\n\
             Item 4. Controls and Procedures\nItem 1. Legal Proceedings\nItem 3. Defaults Upon Senior Securities\n\
             Item 6. Exhibits\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[4].text, "Item 6. Exhibits\nList");
    }

    #[test]
    fn second_contents_table_after_signatures_heading() {
        // A bare contents list, a signatures heading, a page-numbered contents
        // list, then the body.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\nItem 2. Properties\nItem 3. Legal Proceedings\nSIGNATURES\n\
             Item 1. Business 1\nItem 2. Properties 9\nItem 3. Legal Proceedings 10\n\
             Item 1. Business\n{body}\nItem 2. Properties\n{body}\nItem 3. Legal Proceedings\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3"]);
        assert!(s[0].text.starts_with("Item 1. Business\nLorem"));
    }

    #[test]
    fn signature_sentence_ends_last_item_without_heading() {
        let text = "Item 8.01 Other Events.\nWe did things.\n\
                    Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly caused \
                    this report to be signed.\nEXHIBIT INDEX\n99.1 Press release";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(s[0].text, "Item 8.01 Other Events.\nWe did things.");
    }

    #[test]
    fn signatures_line_in_contents_keeps_empty_reserved_item() {
        // The contents list "Signatures" too. The body's "Item 6. [Reserved]"
        // is no longer than its contents entry, and must not be taken for a
        // list printed after the signatures.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 5. Market for Registrant's Common Equity\nItem 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\nSignatures\n\
             Item 5. Market for Registrant's Common Equity\n{body}\nItem 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\n{body}\nSIGNATURES\nPursuant to...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_7"]);
        assert_eq!(s[1].text, "Item 6. [Reserved]");
    }

    #[test]
    fn signatures_line_in_contents_keeps_last_item() {
        // Contents that end with Item 16 and Signatures, then a note longer
        // than the body's Item 16.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\nItem 15. Exhibits and Financial Statement Schedules\nItem 16. Form 10-K Summary\n\
             Signatures\nForward-Looking Statements\n{body}{body}\nItem 1. Business\n{body}\n\
             Item 15. Exhibits and Financial Statement Schedules\n{body}\nItem 16. Form 10-K Summary\nNone.\n\
             SIGNATURES\nPursuant to...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_15", "item_16"]);
        assert_eq!(s[2].text, "Item 16. Form 10-K Summary\nNone.");
    }

    #[test]
    fn short_item_before_second_contents_list_is_kept() {
        // A Part II contents list after a short Item: the entries before a
        // dropped contents entry go with it, but not a body heading.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 3. Legal Proceedings\t15\nItem 4. Mine Safety Disclosures\t16\n\
             Item 5. Market for Registrant's Common Equity\t20\nItem 7. Management's Discussion\t22\n\
             Item 1. Business\n{body}\nItem 3. Legal Proceedings\n{body}\n\
             Item 4. Mine Safety Disclosures\nNot applicable.\n\
             PART II\nItem 5. Market for Registrant's Common Equity\t20\nItem 7. Management's Discussion\t22\n\
             Item 5. Market for Registrant's Common Equity\n{body}\nItem 7. Management's Discussion\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_3", "item_4", "item_5", "item_7"]);
        assert!(s[2].text.starts_with("Item 4. Mine Safety Disclosures\nNot applicable.\nPART II\n"));
        assert!(s[3].text.starts_with("Item 5. Market for Registrant's Common Equity\nLorem"));
    }

    #[test]
    fn bare_heading_with_title_below_is_not_a_running_header() {
        // "ITEM 1." with its title on the next line is the real heading, and
        // the titled line on the next page is the running header.
        let page = BODY.repeat(40);
        let text = format!(
            "ITEM 1.\nBUSINESS\n{page}\nItem 1. Business (continued)\n{page}\nITEM 1A.\nRISK FACTORS\n{page}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a"]);
        assert!(s[0].text.starts_with("ITEM 1.\nBUSINESS\nLorem"));
    }

    #[test]
    fn part_line_above_an_item_line_is_not_read_across_it() {
        // The contents end with "PART II" and Item 6, and the body starts
        // with no PART I line. The PART II line belongs to the contents entry.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t12\n\
             PART II\nItem 6. Exhibits\t20\n\
             Item 1. Financial Statements\n{body}{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
    }

    #[test]
    fn part_line_above_a_running_header_of_the_same_item_counts() {
        // Every page starts "PART II" and "Item 2"; the first page also has
        // the titled heading, which belongs to Part II.
        let body = BODY.repeat(30);
        let text = format!(
            "PART II. OTHER INFORMATION\nItem 2\nITEM 2. UNREGISTERED SALES OF EQUITY SECURITIES AND USE OF PROCEEDS\n\
             {body}\nPART II\nItem 2\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_2", "part_2_item_6"]);
        assert!(s[0].heading.starts_with("ITEM 2. UNREGISTERED"));
    }

    #[test]
    fn bare_heading_with_its_standard_title_is_kept() {
        // Part I Item 4 is printed bare with its title below, and Part II
        // leaves out Items 1 to 3, so the next heading is Part II Item 4.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 4.\nControls and Procedures\n{body}\nPART II\nItem 4. Mine Safety Disclosures\nNot applicable.\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_4", "part_2_item_6"]);
    }

    #[test]
    fn out_of_order_items_after_the_signatures_are_ignored() {
        // An exhibit after the signatures prints "PART I" and an Item that the
        // 10-Q itself left out.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n31.1 Certification\nSIGNATURES\nPursuant to...\n\
             Exhibit 99.1\nPART I\nItem 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[4].text, "Item 6. Exhibits\n31.1 Certification");
    }

    #[test]
    fn attached_annual_report_ends_last_item() {
        let mut contents = String::new();
        for (i, page) in (26..120).step_by(9).enumerate() {
            contents += &format!("{page}\tSection {i} of the annual report\n");
        }
        let report = BODY.repeat(800);
        let text = format!(
            "Item 14. Principal Accounting Fees\nSee proxy.\nItem 15. Exhibits\n31.1\tCertification\n\
             Financial:\n{contents}{report}\nSignatures\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_14", "item_15"]);
        assert_eq!(s[1].text, "Item 15. Exhibits\n31.1\tCertification");
    }

    // Layouts close to those each special rule was written for, where the
    // rule must not fire.

    #[test]
    fn attached_document_needs_a_long_paginated_contents() {
        let long = BODY.repeat(1000);
        let last_item = |text: &str| extract_sections(text, FormKind::TenK).pop().expect("one section").text;
        // Fewer than 8 page-numbered entries.
        let short = "Report of Independent Accountants\t45\nBalance Sheets\t46\nStatements of Income\t47\nNotes\t80\n";
        let text = format!("Item 15. Exhibits\n{short}{long}\nSIGNATURES\n");
        assert!(last_item(&text).ends_with(long.trim()));
        // Entries spanning fewer than 30 pages.
        let mut close = String::new();
        for page in 45..55 {
            close += &format!("Schedule {page} of the report\t{page}\n");
        }
        let text = format!("Item 15. Exhibits\n{close}{long}\nSIGNATURES\n");
        assert!(last_item(&text).ends_with(long.trim()));
        // A long contents list followed by less than four times the Item's own text.
        let mut wide = String::new();
        for page in (40..120).step_by(8) {
            wide += &format!("Schedule {page} of the report\t{page}\n");
        }
        let after = BODY.repeat(700);
        let text = format!("Item 15. Exhibits\n{long}\n{wide}{after}\nSIGNATURES\n");
        assert!(last_item(&text).ends_with(after.trim()));
    }

    #[test]
    fn signature_sentence_needs_duly_caused() {
        let text = "Item 8.01 Other Events.\nPursuant to the requirements of the Securities Exchange Act of 1934, we \
                    file reports with the SEC.\nMore text.\n";
        let s = extract_sections(text, FormKind::EightK);
        assert!(s[0].text.ends_with("More text."));
    }

    #[test]
    fn titled_sub_items_are_headings_and_bare_ones_are_not() {
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nItem 15. Exhibits\n{body}\nItem 15(a)(1)\n{body}\n\
             Item 16. Form 10-K Summary\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_15", "item_16"]);
        assert!(s[1].text.contains("\nItem 15(a)(1)\n"));
        let text =
            format!("PART I\nItem 1. Financial Statements\n{body}\nPART II\nITEM 6 (a). Exhibits\n{body}\nSIGNATURE\n");
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_6"]);
    }

    #[test]
    fn letter_after_a_period_is_not_part_of_the_item_number() {
        // "Item 9 A." is Item 9A, but in "ITEM 1. A. GENERAL" the letter starts the title.
        let body = BODY.repeat(30);
        let text =
            format!("ITEM 1. A. GENERAL DEVELOPMENT OF BUSINESS\n{body}\nItem 9 A. Controls and Procedures\n{body}\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_9a"]);
    }

    #[test]
    fn letter_joined_to_the_number_by_a_period_is_part_of_it() {
        // "Item 1.A." has no space after the period, so the letter belongs to the number.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nItem 1.A. Risk Factors\n{body}\nItem 1.B. Unresolved Staff Comments\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_1b"]);
    }

    #[test]
    fn temporary_item_mark_after_a_spaced_letter_joins_it_to_the_number() {
        // "Item 9 A (T)." prints the letter apart from the number, then the
        // "(T)" of the temporary Items: the letter belongs to the number.
        let body = BODY.repeat(30);
        for heading in [
            "Item 9 A (T). Controls and Procedures.",
            "Item 9 A(T) Controls and Procedures",
            "ITEM 9 A (t) – CONTROLS AND PROCEDURES",
        ] {
            let text = format!(
                "Item 8. Financial Statements and Supplementary Data\n{body}\n\
                 Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\nNone\n\
                 {heading}\n{body}\nItem 9 B. Other Information\nNone.\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_9a", "item_9b"], "{heading}");
            assert_eq!(s[2].heading, heading);
        }
        // Without "(T)" or a separator after it, the letter starts the title.
        let text = format!(
            "Item 8. Financial Statements and Supplementary Data\n{body}\n\
             Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\nNone\n\
             Item 9 A Controls and Procedures\n{body}\nItem 9 B. Other Information\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9", "item_9b"]);
        assert!(s[1].text.contains("\nItem 9 A Controls and Procedures\n"));
        assert_eq!(split_letter("9", " ", "A (T). Controls"), ("9A".to_owned(), "Controls"));
        assert_eq!(split_letter("1", ". ", "A (T). General"), ("1".to_owned(), "A (T). General"));
        assert_eq!(split_letter("9", " ", "A (Tax) Matters"), ("9".to_owned(), "A (Tax) Matters"));
    }

    #[test]
    fn comma_after_the_item_number_before_its_standard_title_is_a_heading() {
        // A comma after the Item number, then the Item's standard title or
        // nothing but the numbers of more Items, or their standard titles.
        // A heading that names several Items starts the first, as "Items 9
        // and 9A." does, and keeps the auditors' report on internal control.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nITEMS 1., 1A. and 2. BUSINESS, RISK FACTORS AND PROPERTIES\n{body}\n\
             Item 1B. Unresolved Staff Comments\nNone.\nPART II\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 8, Financial Statements and Supplementary Data\n{body}\nITEMS 9, 9A AND 9B.\nNone.\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nPART III\n\
             ITEMS 10., 11., 12. AND 13.\nSee the proxy statement.\n\
             ITEM 14, PRINCIPAL ACCOUNTANT FEES AND SERVICES.\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1b", "item_7", "item_8", "item_9", "item_10", "item_14"]);
        assert_eq!(s[3].heading, "Item 8, Financial Statements and Supplementary Data");
        assert!(s[4].text.contains("\nReport of Independent Registered Public Accounting Firm\n"));
        assert_eq!(s[5].text, "ITEMS 10., 11., 12. AND 13.\nSee the proxy statement.");
        // In a 10-Q, with the standard title of either part.
        let text = format!(
            "PART I\nItem 1, Financial Statements\n{body}\nPART II\nItem 1, Legal Proceedings\n{body}\n\
             Items 2, 3 & 4 — Unregistered Sales of Equity Securities and Use of Proceeds; Defaults Upon Senior \
             Securities; Mine Safety Disclosures\nNone.\nItem 6, Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_1", "part_2_item_2", "part_2_item_6"]);
        assert_eq!(listed_items("1", ", Legal Proceedings", TEN_Q), Some(("Legal Proceedings", false)));
        assert_eq!(listed_items("9", ", 9A AND 9B.", TEN_K), Some(("", true)));
        assert_eq!(
            listed_items("2.02", ", Results of Operations and Financial Condition.", EIGHT_K),
            Some(("Results of Operations and Financial Condition.", false))
        );
    }

    #[test]
    fn items_cited_with_a_comma_in_running_text_are_no_headings() {
        // Lines that start with an Item number and a comma, but go on past
        // the title, shorten it, name no title, list Items with no "and" (a
        // running header) or out of form order, or carry on a sentence.
        let body = BODY.repeat(30);
        for (label, after) in [
            ("8", ", which follows, holds the statements."),
            ("7", ", Management's Discussion and Analysis of Financial Condition and Results of Operations, of our"),
            ("7", ", \"Management's Discussion and Analysis.\""),
            ("8", ", Financial Statements and Supplementary Data\t45"),
            ("8", ", financial statements and supplementary data"),
            ("8", ","),
            ("9", ", 9A"),
            ("9B", ", 9C, 10, 11, 12, 13"),
            ("9", ", 9A and 9B are not applicable."),
            ("3", ", 4, and 5 are not applicable and have been omitted."),
            ("9B", ", 9A and 9C."),
            ("1", ", 1A, 2 and 3; Part II – Items 5, 6, 7,"),
            ("1", "., 1A. & 2.\tBusiness, Risk Factors and Properties\t1"),
            ("9", ", 9A AND 9B. CHANGES IN ACCOUNTANTS; CONTROLS AND PROCEDURES"),
            ("9", ", 9A and 99."),
        ] {
            assert_eq!(listed_items(label, after, TEN_K), None, "Item {label}{after}");
        }
        let text = format!(
            "PART II\nItem 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}\nItem 8, which follows, holds the statements.\nThe statements are incorporated pursuant to\n\
             Item 8, Financial Statements and Supplementary Data.\nItem 9, 9A\n{body}{body}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_8"]);
        assert!(s[0].text.contains("\nItem 8, Financial Statements and Supplementary Data.\nItem 9, 9A\n"));
    }

    #[test]
    fn range_of_items_with_a_dash_is_a_heading() {
        // "Item 1-5." in a 10-Q's Part II and "ITEMS 10 - 13." in a 10-K's
        // Part III start the first Item of the range, as "Items 1 through 5."
        // does.
        let body = BODY.repeat(30);
        for range in ["Item 1-5.   Not applicable.", "Items 1 - 5", "Items 1–5: None.", "Item 1. - 5. Not applicable."]
        {
            let text = format!(
                "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
                 Part II  OTHER INFORMATION\n{range}\nItem 6.     Exhibits and Reports on Form 8-K\nNone\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"], "{range}");
            assert_eq!(s[2].heading, range);
        }
        for range in ["ITEMS 10 - 13.", "ITEMS 10-14.", "Items 10—13. Omitted under General Instruction I."] {
            let text = format!(
                "Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\n\
                 None.\nPART III.\n{range}\nInformation called for by items 10 through 13 is omitted.\n\
                 PART IV\nITEM 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_10", "item_14"], "{range}");
            assert_eq!(
                s[0].text,
                "Item 9. Changes in and Disagreements with Accountants on Accounting and Financial \
                                   Disclosure\nNone.\nPART III."
            );
        }
    }

    #[test]
    fn numbers_after_a_dash_that_name_no_range_of_items_are_no_headings() {
        // A cross-reference, a range that goes down or past the form's Items,
        // a contents row whose page follows the range, a year or a number
        // after a dash, and a title that starts with a form's name.
        for (label, after) in [
            ("1", "-2 of the Form"),
            ("1", "-4 of Part I"),
            ("2", "-1. Properties"),
            ("1", "-99."),
            ("1", ". - 6.                    12"),
            ("7", " - 2005 compared with 2004"),
            ("5", " - 10,000 shares"),
            ("16", " - 10-K SUMMARY"),
        ] {
            assert_eq!(listed_items(label, after, TEN_K), None, "Item {label}{after}");
        }
        let body = BODY.repeat(30);
        let text = format!(
            "Item 7. Management's Discussion and Analysis\n{body}\nItem 1-2 of the Form requires this.\n\
             Item 7 - 2005 compared with 2004\n{body}\nItem 15. Exhibits\n{body}\nITEM 16 - 10-K SUMMARY\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_15", "item_16"]);
        assert!(s[0].text.contains("\nItem 1-2 of the Form requires this.\nItem 7 - 2005 compared with 2004\n"));
    }

    #[test]
    fn short_body_item_before_a_second_contents_list_is_kept() {
        // The empty body "Item 6. [Reserved]" sits just before a second
        // contents list; it is a body heading, not part of that list.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 5. Market for Registrant's Common Equity\t20\n\
             Item 6. [Reserved]\t21\nItem 7. Management's Discussion and Analysis\t22\n\
             Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\n{body}\n\
             Item 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
             Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"]);
    }

    #[test]
    fn short_body_item_missing_from_the_first_contents_is_kept() {
        // The first contents list leaves out Item 6, so its body heading
        // appears only once. It follows a body heading, not a contents entry,
        // so it does not join the second contents list.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 5. Market for Registrant's Common Equity\t20\n\
             Item 7. Management's Discussion and Analysis\t22\n\
             Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\n{body}\n\
             Item 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
             Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"]);
        assert!(!s[1].text.contains("Item 6"));
        assert!(s[2].text.starts_with("Item 6. [Reserved]\nItem 7. Management's Discussion and Analysis\t22\n"));
        assert!(s[3].text.starts_with("Item 7. Management's Discussion and Analysis\nLorem"));
    }

    #[test]
    fn short_body_items_after_a_short_body_item_are_kept() {
        // Neither contents list has Items 5 and 6, and both body Items are
        // short. A run of such headings before the second list stays.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 7. Management's Discussion and Analysis\t22\n\
             Item 8. Financial Statements\t40\n\
             Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\nNone.\n\
             Item 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
             Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"]);
        assert_eq!(s[1].text, "Item 5. Market for Registrant's Common Equity\nNone.");
    }

    #[test]
    fn contents_entries_without_body_headings_at_the_start_are_dropped() {
        // Part I Items 1 and 2 are listed in the contents but have no heading
        // in the body. They come first and lead up to dropped contents
        // entries, so they are contents entries too.
        let body = BODY.repeat(30);
        let text = format!(
            "Part I - Financial Information\nItem 1 Consolidated Financial Statements\t60\n\
             Item 2 Management's Discussion and Analysis\t3\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\t58\n\
             Item 4 Controls and Procedures\t58\nPart II - Other Information\n\
             Item 1 Legal Proceedings\t90\nItem 6 Exhibits\t92\n\
             Financial Highlights\n{body}{body}\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee Market Risk Management.\n\
             Item 4 Controls and Procedures\n{body}\nPart II Other Information\n\
             Item 1 Legal Proceedings\n{body}\nItem 6 Exhibits\n{body}\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[0].text.starts_with("Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee"));
    }

    #[test]
    fn contents_entries_between_dropped_entries_are_dropped() {
        // Items 1A and 1B are listed in the contents but have no heading in
        // the body. They sit between two dropped contents entries; kept, they
        // would outscore the short Item 1.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 1A. Risk Factors\t4\nItem 1B. Unresolved Staff Comments\t9\n\
             Item 2. Properties\t10\n\
             Item 1. Business\nSee the annual report.\nItem 2. Properties\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2"]);
        assert_eq!(s[0].text, "Item 1. Business\nSee the annual report.");
    }

    #[test]
    fn item_line_before_the_contents_does_not_keep_contents_entries() {
        // A 10-Q/A's explanatory note names Item 4 well before the contents,
        // whose rows for Part I Items 1 and 2 have no heading in the body.
        // Those rows come before the body, so they are still dropped.
        let body = BODY.repeat(30);
        let text = format!(
            "FORM 10-Q/A\nEXPLANATORY NOTE\nThis amendment restates the following items:\n\
             Item 4 Controls and Procedures (as amended)\n{body}\n\
             Part I - Financial Information\nItem 1 Consolidated Financial Statements\t60\n\
             Item 2 Management's Discussion and Analysis\t3\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\t58\n\
             Item 4 Controls and Procedures\t58\nPart II - Other Information\n\
             Item 1 Legal Proceedings\t90\nItem 6 Exhibits\t92\n\
             Financial Highlights\n{body}{body}\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee Market Risk Management.\n\
             Item 4 Controls and Procedures\n{body}\nPart II Other Information\n\
             Item 1 Legal Proceedings\n{body}\nItem 6 Exhibits\n{body}\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[0].text.starts_with("Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee"));
    }

    #[test]
    fn short_body_items_before_the_only_listed_headings_are_kept() {
        // No contents list names the first body headings, so the first body
        // heading a contents entry names comes late. The short body Items
        // before a second contents list follow Items with text of their own.
        let body = BODY.repeat(30);
        for first_list in ["", "TABLE OF CONTENTS\nPART I\t3\nPART II\t20\n"] {
            let text = format!(
                "{first_list}Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\n{body}\n\
                 Item 6. [Reserved]\n\
                 Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
                 Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"], "{first_list:?}");
            assert!(s[2].text.starts_with("Item 6. [Reserved]\nItem 7. Management's Discussion and Analysis\t22\n"));
        }
        // Short Items 9, 9A and 9B before a Part III contents list.
        let text = format!(
            "Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n\
             Item 9. Changes in and Disagreements with Accountants\nNone.\nItem 9A. Controls and Procedures\n\
             Our controls are effective.\nItem 9B. Other Information\nNone.\n\
             PART III\nItem 10. Directors, Executive Officers and Corporate Governance\t60\n\
             Item 11. Executive Compensation\t62\n\
             Item 10. Directors, Executive Officers and Corporate Governance\n{body}\n\
             Item 11. Executive Compensation\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_9", "item_9a", "item_9b", "item_10", "item_11"]);
        assert!(s[1].text.ends_with(body.trim()));
    }

    #[test]
    fn short_body_items_before_an_index_of_items_at_the_end_are_kept() {
        // A cross-reference index before the signatures lists every Item. Its
        // last row starts more text than the short Part II Item 6, so the
        // body's Item 6 reads as a contents entry and the row as its body
        // heading. The short Items before Item 6 follow MD&A, which has text
        // of its own.
        let body = BODY.repeat(30);
        let text = format!(
            "FORM 10-Q\nPART I — FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\nSee Market Risk in Item 2.\n\
             Item 4. Controls and Procedures\nOur controls are effective.\n\
             PART II — OTHER INFORMATION\nItem 1. Legal Proceedings\nSee Note 9.\nItem 1A. Risk Factors\nNone.\n\
             Item 6. Exhibits\nSee the Exhibit Index.\nFORM 10-Q CROSS-REFERENCE INDEX\n\
             Item 1. Financial Statements\t3\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\t30\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\t45\n\
             Item 4. Controls and Procedures\t46\nItem 1. Legal Proceedings\t47\nItem 1A. Risk Factors\t47\n\
             Item 6. Exhibits\t48\nSIGNATURES\nPursuant to the requirements of the Securities Exchange Act of 1934, \
             the registrant has duly caused this report to be signed.\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        let headings: Vec<(&str, &str)> = s.iter().map(|s| (s.id.as_str(), s.heading.as_str())).collect();
        assert_eq!(
            headings[2..6],
            [
                ("part_1_item_3", "Item 3. Quantitative and Qualitative Disclosures About Market Risk"),
                ("part_1_item_4", "Item 4. Controls and Procedures"),
                ("part_2_item_1", "Item 1. Legal Proceedings"),
                ("part_2_item_1a", "Item 1A. Risk Factors"),
            ]
        );
        assert_eq!(ids(&s)[..2], ["part_1_item_1", "part_1_item_2"]);
        assert!(s[1].text.ends_with(body.trim()));
        // An exhibit after the signatures repeats Item 4 with more text. Item
        // 3 follows MD&A and stays.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot applicable.\n\
             Item 4. Controls and Procedures\nOur disclosure controls are effective.\nPART II\n\
             Item 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n31.1 Certification\nSIGNATURES\nPursuant to...\n\
             Exhibit 99.1\nPART I\nItem 4. Controls and Procedures\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s)[..3], ["part_1_item_1", "part_1_item_2", "part_1_item_3"]);
        assert!(s[1].text.ends_with(body.trim()));
        assert!(s[2].text.starts_with("Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot"));
    }

    #[test]
    fn part_line_search_stops_at_a_part_prefixed_item_line() {
        // "PART II - Item 6. Exhibits" is a contents row for another Item, not
        // the PART line of the body's first heading.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I - Item 1. Financial Statements\t3\n\
             PART I - Item 2. Management's Discussion and Analysis\t12\n\
             PART II - Item 6. Exhibits\t20\n\
             Item 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             PART II - OTHER INFORMATION\nItem 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
    }

    #[test]
    fn references_to_undefined_items_do_not_end_an_item() {
        // Only a titled heading of an Item the form lacks ("Item 4A. Executive
        // Officers") ends the Item before it; references to one do not.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 4. Mine Safety Disclosures\nNot applicable.\nItem 4A of Part I lists our executive officers.\n\
             Item 401 of Regulation S-K requires this list.\nItem 5. Market for Registrant's Common Equity\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_4", "item_5"]);
        assert!(s[0].text.ends_with("requires this list."));
    }

    #[test]
    fn unmarked_ten_q_items_take_their_part_from_order_or_title() {
        // No PART lines. After Part II Item 1, Item 2 is in Part II.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 1. Legal Proceedings\n{body}\nItem 2. Unregistered Sales of Equity Securities\n{body}\n\
             Item 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_6"]);
        // With nothing before it, the title decides, not the tie-break.
        let text = format!(
            "Item 1. Legal Proceedings\n{body}\nItem 1A. Risk Factors\n{body}\n\
             Item 4. Mine Safety Disclosures\nNot applicable.\nItem 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_1a", "part_2_item_4", "part_2_item_6"]);
        // An unmarked heading whose title names neither part keeps the tie-break.
        let text = format!("Item 2. Results\n{body}\nItem 6. Exhibits\n{body}\n");
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_2_item_6"]);
    }

    #[test]
    fn short_part_marked_lines_are_not_added_out_of_order() {
        // A PART I line and an Item that the 10-Q left out, with only a few
        // words after them: a cross-reference, not the Item.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\n\
             PART I\nItem 3. Quantitative and Qualitative Disclosures About Market Risk\nSee Item 2.\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn ten_q_item_out_of_order_found_by_its_title() {
        // MD&A comes first under the only PART I line, and Item 1 follows it
        // with no PART line of its own. Its title names Part I.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1.\tFinancial Statements and Notes\t73\n\
             Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t4\n\
             Item 3.\tQuantitative and Qualitative Disclosures about Market Risk\t142\n\
             Item 4.\tControls and Procedures\t142\nPART II\nItem 1.\tLegal Proceedings\t143\n\
             Item 6.\tExhibits\t143\n\
             PART I—FINANCIAL INFORMATION\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}{body}{body}\nGlossary and Acronyms\n{body}\n\
             Item 1. Financial Statements and Notes\n{body}{body}\n\
             Item 3. Quantitative and Qualitative Disclosures about Market Risk\nSee Item 2.\n\
             Item 4. Controls and Procedures\n{body}\n\
             PART II—OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_2", "part_1_item_1", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]
        );
        assert_eq!(s[1].heading, "Item 1. Financial Statements and Notes");
        assert!(s[0].text.ends_with(body.trim()));
        // The statements after Item 4, with the title on the next line.
        let text = format!(
            "PART I\nItem 2. Management's Discussion and Analysis\n{body}{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nITEM 1.\nFINANCIAL STATEMENTS\n{body}{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_2", "part_1_item_3", "part_1_item_4", "part_1_item_1", "part_2_item_1", "part_2_item_6"]
        );
        assert!(s[2].text.ends_with(body.trim()));
    }

    #[test]
    fn titles_add_no_out_of_order_item_to_a_ten_q_in_order() {
        // Items in form order with a contents list, a running header that
        // repeats Item 1's heading and a cross-reference line that reads like
        // it. Every Item is already in the chain, so nothing is added.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t20\n\
             Item 4. Controls and Procedures\t30\nItem 1. Legal Proceedings\t31\nItem 6. Exhibits\t32\n\
             Item 1. Financial Statements\n{body}{body}\nItem 1. Financial Statements (continued)\n{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             Item 1. Financial Statements, Note 4, describes our debt.\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 1. Financial Statements, Note 4, describes our debt.\n"));
    }

    #[test]
    fn titles_add_no_contents_entry_or_short_cross_reference_out_of_order() {
        // The financial statements have no Item 1 heading. A cross-reference
        // line that reads "Item 1. Financial Statements" and an index of Items
        // before the signatures start only a few characters of text.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nConsolidated Financial Statements\n{body}{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n\
             See Market Risk in Item 2 and\nItem 1. Financial Statements, Note 7.\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n{body}\nForm 10-Q Cross-Reference Index\n\
             Item 1. Financial Statements\t3-40\nItem 2. Management's Discussion and Analysis\t41-60\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\t61\n\
             Item 4. Controls and Procedures\t61\nSIGNATURES\nPursuant to...\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.ends_with("Item 1. Financial Statements, Note 7."));
        // The contents list Item 4, which has no heading in the body, and a
        // long introduction follows the list: its entry starts a long stretch
        // of text, but before the first body heading.
        let text = format!(
            "PART I\nItem 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t20\n\
             Item 4. Controls and Procedures\t36\nForward-Looking Statements\n{body}\n\
             Item 1. Financial Statements\n{body}{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn titles_of_part_two_items_add_nothing_out_of_order() {
        // The 10-Q has no Item 1A heading. A line in MD&A starts with the
        // annual report's "Item 1A. Risk Factors", the same number and title
        // as Part II Item 1A; without a PART line it is not added.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             Item 1A. Risk Factors of our Annual Report on Form 10-K describes the risks we face.\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 1A. Risk Factors of our Annual Report"));
    }

    /// Capital One's 10-Q layout: a contents list in form order, one PART I
    /// line, Item 2 (MD&A), Item 1 (the statements) with heading `item_1`,
    /// then Items 3 and 4 and Part II. `mdna` and `statements` set the lengths
    /// of Items 2 and 1.
    fn item_2_before_item_1(item_1: &str, mdna: usize, statements: usize) -> String {
        let body = BODY.repeat(30);
        format!(
            "PART I\nItem 1.\tFinancial Statements and Notes\t73\n\
             Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t4\n\
             Item 3.\tQuantitative and Qualitative Disclosures about Market Risk\t142\n\
             Item 4.\tControls and Procedures\t142\nPART II\nItem 1.\tLegal Proceedings\t143\n\
             Item 6.\tExhibits\t143\n\
             PART I—FINANCIAL INFORMATION\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {}\n{item_1}\n{}\n\
             Item 3. Quantitative and Qualitative Disclosures about Market Risk\nSee Item 2.\n\
             Item 4. Controls and Procedures\n{body}\n\
             PART II—OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n",
            body.repeat(mdna),
            body.repeat(statements),
        )
    }

    const ITEM_2_BEFORE_ITEM_1: [&str; 6] =
        ["part_1_item_2", "part_1_item_1", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"];

    #[test]
    fn ten_q_item_2_before_item_1_is_kept_whichever_is_longer() {
        // The chain holds only one of the two and keeps the longer. When that
        // is Item 1, Item 2 comes before the chain's first heading, but it is
        // the first body heading that the contents names.
        for (mdna, statements) in [(4, 3), (3, 4)] {
            let text = item_2_before_item_1("Item 1. Financial Statements and Notes", mdna, statements);
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ITEM_2_BEFORE_ITEM_1, "MD&A {mdna}, statements {statements}");
            assert_eq!(s[0].end, s[1].start);
        }
    }

    #[test]
    fn qualified_financial_statements_titles_name_part_one() {
        // Item 1 printed after MD&A with no PART line of its own, under a
        // title that qualifies the financial statements.
        for title in [
            "Consolidated Financial Statements",
            "Condensed Consolidated Financial Statements (Unaudited)",
            "Unaudited Consolidated Condensed Financial Statements",
            "INTERIM FINANCIAL STATEMENTS",
        ] {
            assert_eq!(part_by_title("1", title, TEN_Q), Some("I"), "{title}");
            let text = item_2_before_item_1(&format!("Item 1. {title}"), 4, 3);
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ITEM_2_BEFORE_ITEM_1, "{title}");
            assert_eq!(s[1].heading, format!("Item 1. {title}"));
        }
        assert_eq!(part_by_title("1", "Consolidated Legal Proceedings", TEN_Q), Some("II"));
        assert_eq!(part_by_title("1", "Consolidated", TEN_Q), None);
        assert_eq!(part_by_title("2", "Consolidated Financial Statements", TEN_Q), None);
    }

    #[test]
    fn wrapped_cross_reference_to_part_one_is_not_added_out_of_order() {
        // A plain-text 10-Q with no Item 1 heading. Part II Item 1 cites Part
        // I Item 1 in a sentence that wraps so that a line starts with it.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nCONSOLIDATED BALANCE SHEETS\n{body}{body}{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}{body}\nPART II. OTHER INFORMATION\nItem 1. Legal Proceedings\n\
             The information in Note 5 to the Consolidated Financial Statements in Part I,\n\
             Item 1. Financial Statements, of this report is hereby updated as follows:\n{body}\n\
             Item 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 1. Financial Statements, of this report is hereby updated"));
    }

    #[test]
    fn wrapped_cross_reference_does_not_take_its_item() {
        // A 10-Q with no Item 1 heading, whose MD&A cites Item 1 in a wrapped
        // sentence: the line would fit the Item order and outscore Item 2.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nCONSOLIDATED BALANCE SHEETS\n{body}{body}{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}\nThe effect of adoption is described in Note 4 to the financial statements in Part I,\n\
             Item 1. Financial Statements, of this report. The adoption did not have a material\n\
             effect on results of operations.\n{body}{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             PART II. OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_6"]);
        assert!(s[0].text.contains("\nItem 1. Financial Statements, of this report."));
        // Part I has no Item headings, and Part II Item 1 cites Part I Item 2.
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nCONSOLIDATED STATEMENT OF INCOME\n{body}{body}\n\
             MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\nPART II. OTHER INFORMATION\n\
             Item 1. Legal Proceedings\nThe discussion of the litigation in Part I,\n\
             Item 2. Management's Discussion and Analysis, of this report is hereby updated:\n{body}\n\
             Item 4. Submission of Matters to a Vote of Security Holders\n{body}\n\
             Item 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_4", "part_2_item_6"]);
    }

    #[test]
    fn part_one_headings_with_other_lowercase_words_stay() {
        // Sentence case, notes in brackets, a wrapped title, a run-in sentence
        // after a period and the period covered do not make a heading read as
        // a cross-reference.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Condensed consolidated financial statements (unaudited)\n{body}\n\
             Item 2. Management's discussion and analysis of financial condition and\nresults of operations\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk. Not applicable.\n\
             Item 4. Controls and procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1"]);
        assert!(!runs_on_as_sentence("1", "Financial Statements and Notes Thereto", TEN_Q));
        assert!(!runs_on_as_sentence("1", "Financial statements for the three months ended March 31, 2020", TEN_Q));
        assert!(runs_on_as_sentence("1", "Financial Statements, Note 5, of this report is hereby updated as", TEN_Q));
        assert!(runs_on_as_sentence(
            "2",
            "Management's Discussion and Analysis, under \"Litigation,\" in Part I",
            TEN_Q
        ));
    }

    #[test]
    fn part_one_headings_that_go_on_with_words_of_their_own_stay() {
        // Each heading continues its standard title with words that are not
        // in it: other wording, the period covered, a possessive, a run-in
        // sentence. With or without PART lines, every Item is found.
        let body = BODY.repeat(30);
        let standard = [
            "Item 1. Financial Statements",
            "Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations",
            "Item 3. Quantitative and Qualitative Disclosures About Market Risk",
            "Item 4. Controls and Procedures",
        ];
        let variants = [
            (0, "Item 1. Financial statements for the thirteen weeks ended May 2, 2020"),
            (0, "Item 1. Financial statements for the fiscal quarter ended June 30, 2005"),
            (0, "Item 1. Condensed consolidated financial statements for the quarterly period ended March 31, 2021"),
            (0, "Item 1. Financial statements of the registrant"),
            (0, "Item 1. Financial Statements of iRobot Corporation"),
            (0, "Item 1. Financial statements and supplementary data"),
            (0, "Item 1. Financial statement"),
            (0, "Item 1. Financial Statements of the Company's Operations"),
            (1, "Item 2. Management's discussion and analysis of financial condition and results of operation"),
            (1, "Item 2. Management's discussion and analysis of financial position and results of operations"),
            (1, "Item 2. Management's discussion and analysis or plan of operation"),
            (
                1,
                "Item 2. Management's discussion and analysis of financial condition and results of operations and \
                 quantitative and qualitative disclosures about market risk",
            ),
            (2, "Item 3. Quantitative and qualitative disclosure about market risk"),
            (2, "Item 3. Quantitative and qualitative disclosures about market risks"),
            (2, "Item 3. Quantitative and qualitative disclosures regarding market risk"),
            (2, "Item 3. Quantitative and Qualitative Disclosures About the Company's Market Risk"),
            (2, "Item 3. Quantitative and Qualitative Disclosures About Market Risk Not applicable."),
            (2, "Item 3. Quantitative and Qualitative Disclosures About Market Risk The Company is not exposed."),
            (3, "Item 4. Controls and Procedures We maintain disclosure controls and procedures that are designed."),
        ];
        for (at, heading) in variants {
            let mut items = standard;
            items[at] = heading;
            for (part_1, part_2) in [("PART I. FINANCIAL INFORMATION\n", "PART II. OTHER INFORMATION\n"), ("", "")] {
                let text = format!(
                    "FORM 10-Q\n{part_1}{}\n{body}{body}\n{}\n{body}\n{}\n{body}\n{}\n{body}\n\
                     {part_2}Item 1. Legal Proceedings\n{body}\nItem 1A. Risk Factors\n{body}\n\
                     Item 6. Exhibits\n{body}\nSIGNATURES\n",
                    items[0], items[1], items[2], items[3]
                );
                let s = extract_sections(&text, FormKind::TenQ);
                assert_eq!(
                    ids(&s),
                    [
                        "part_1_item_1",
                        "part_1_item_2",
                        "part_1_item_3",
                        "part_1_item_4",
                        "part_2_item_1",
                        "part_2_item_1a",
                        "part_2_item_6"
                    ],
                    "{heading} {part_1:?}"
                );
                assert_eq!(s[at].heading, heading);
            }
        }
        // A cross-reference with no comma still names the report.
        assert!(runs_on_as_sentence("1", "Financial Statements of this report is hereby updated as follows", TEN_Q));
        assert!(runs_on_as_sentence("1", "Financial Statements in Part I of this report for more information", TEN_Q));
        assert!(runs_on_as_sentence("2", "Management's Discussion and Analysis included in our Annual Report", TEN_Q));
        assert!(runs_on_as_sentence(
            "3",
            "Quantitative and Qualitative Disclosures About Market Risk is in Item 2",
            TEN_Q
        ));
        // A comma followed by a date, a qualifier or "continued" is a heading.
        for title in [
            "Financial Statements, March 31, 2020 and December 31, 2019",
            "Financial Statements, Unaudited",
            "Financial Statements, continued",
            "Management's Discussion and Analysis of Financial Condition, and Results of Operations",
        ] {
            assert!(!runs_on_as_sentence(if title.starts_with("Fin") { "1" } else { "2" }, title, TEN_Q), "{title}");
        }
    }

    #[test]
    fn item_line_that_carries_on_a_sentence_is_not_a_heading() {
        // A 10-K whose MD&A cites Item 8 in a sentence wrapped so that a line
        // starts with it, and goes on for longer than Item 8 itself.
        let body = BODY.repeat(30);
        let long = BODY.repeat(60);
        for above in [
            "The fair value of the notes is described in Note 5 to the financial statements in",
            "The notes are described in Part II,",
            "For the fair value of the notes, see",
            "The fair value of the notes is set out in Note 5 (see",
            "The fair value of the notes is shown under",
        ] {
            let text = format!(
                "PART I\nItem 1. Business\n{body}\nItem 2. Properties\n{body}\nPART II\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 {body}\n{above}\nItem 8. Financial Statements and Supplementary Data, of this report. The notes\n\
                 are due in 2030.\n{long}\nItem 8. Financial Statements and Supplementary Data\n{body}\n\
                 Item 9A. Controls and Procedures\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_2", "item_7", "item_8", "item_9a"], "{above}");
            assert!(s[2].text.contains("\nItem 8. Financial Statements and Supplementary Data, of this report."));
            assert_eq!(s[3].heading, "Item 8. Financial Statements and Supplementary Data");
        }
        // The issue's layout: no Item 7 heading, and Item 6 cites Item 7 of
        // the annual report.
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nItem 2. Properties\n{body}\nPART II\n\
             Item 6. Selected Financial Data\n{body}\nThe information set forth in Part II,\n\
             Item 7. Management's Discussion and Analysis, of the Company's Annual Report on Form 10-K for 1998 is \
             incorporated\nherein by reference.\nMANAGEMENT'S DISCUSSION AND ANALYSIS\n{long}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_6", "item_8"]);
        assert!(s[2].text.contains("\nMANAGEMENT'S DISCUSSION AND ANALYSIS\n"));
        // A 10-Q whose Part II Item 1 cites Item 1A of the annual report, and
        // goes on for longer than its own short Item 1A.
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}\nItem 4. Controls and Procedures\n{body}\nPART II. OTHER INFORMATION\n\
             Item 1. Legal Proceedings\n{body}\nThe risks of the litigation described in Part I,\n\
             Item 1A. Risk Factors, of our Annual Report on Form 10-K for 2019 have not\nchanged materially.\n\
             {long}\nItem 1A. Risk Factors\nThere have been no material changes.\nItem 6. Exhibits\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]
        );
        assert_eq!(s[4].heading, "Item 1A. Risk Factors");
        // An 8-K whose Item 2.02 cites Item 7.01 the same way.
        let text = format!(
            "Item 2.02 Results of Operations and Financial Condition\n{body}\n\
             The press release is also furnished under\nItem 7.01 Regulation FD Disclosure, of this report.\n\
             {long}\nItem 7.01 Regulation FD Disclosure\nSee Item 2.02.\n\
             Item 9.01 Financial Statements and Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_7.01", "item_9.01"]);
        assert_eq!(s[1].heading, "Item 7.01 Regulation FD Disclosure");
    }

    #[test]
    fn headings_after_lines_that_end_without_a_period_stay() {
        // Run-in answers with no period, a sentence that stops before a blank
        // line, a PART label printed with a comma or a dash, and a page header
        // in capitals that ends with "AND" do not make the next line part of a
        // sentence.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nItem 1A. Risk Factors\nNot applicable\n\
             Item 1B. Unresolved Staff Comments\nNone\nItem 2. Properties\n{body}\n\
             The properties are described in\n\nItem 3. Legal Proceedings\n{body}\nPART II,\n\
             Item 5. Market for Registrant's Common Equity, Related Stockholder Matters and Issuer Purchases of \
             Equity Securities\n{body}\nANNUAL REPORT ON FORM 10-K AND\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             PART IV -\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_1b", "item_2", "item_3", "item_5", "item_7", "item_15"]);
        let after = |above: &str| continues_sentence(&format!("{above}\nItem 7. Title"), above.len() + 1);
        for above in ["described in", "see", "(see", "as discussed under", "in Part II,", "in Part II,\r", "a, b and"] {
            assert!(after(above), "{above:?}");
        }
        for above in [
            "",
            "None",
            "Not applicable",
            "ends here.",
            "PART II,",
            "Part I -",
            "AND",
            "In",
            "in\n",
            "and",
            "  and\t",
            "or",
        ] {
            assert!(!after(above), "{above:?}");
        }
        assert!(!continues_sentence("Item 7. Title", 0));
    }

    #[test]
    fn headings_joined_by_a_line_with_only_and_are_both_items() {
        // Norfolk Southern 2002 underlines two headings and prints "and"
        // between them, for Items 1 and 2 and again for Items 12 and 13.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1.\tBusiness.\n------\t--------\nand\nItem 2.\tProperties.\n------\t----------\n{body}\n\
             Item 3.\tLegal Proceedings.\n{body}\nPART III\n\
             Item 12.    Security Ownership of Certain Beneficial Owners and\n-------     ------------\nManagement.\n\
             ----------\nand\nItem 13.    Certain Relationships and Related Transactions.\n-------     ----------\n{body}\n\
             PART IV\nItem 14.    Exhibits, Financial Statement Schedules, and Reports on Form 8-K.\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3", "item_12", "item_13", "item_14"]);
    }

    #[test]
    fn dashes_after_the_separator_belong_to_it() {
        // "Item 3. - Quantitative ..." prints a period and a dash, "ITEM 6 --
        // EXHIBITS" two hyphens: the title starts after the dashes.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2 - Management's Discussion and Analysis\n{body}\n\
             Item 3. - Quantitative and Qualitative Disclosures about Market Risk\n{body}\n\
             Item 4. – Controls and Procedures\n{body}\n\
             PART II\nItem 1: — Legal Proceedings\n{body}\nITEM 6 -- EXHIBITS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]
        );
        assert_eq!(s[2].heading, "Item 3. - Quantitative and Qualitative Disclosures about Market Risk");
        assert!(s[1].text.ends_with(body.trim()));
        let text = "Item 2.02. - Results of Operations and Financial Condition.\nOn May 2, the Company...\n\
                    Item 9.01 -- Financial Statements and Exhibits.\n(d) Exhibits\nSIGNATURE\n";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_9.01"]);
    }

    #[test]
    fn text_after_dashes_must_still_look_like_a_title() {
        // A cross-reference or a bare sub-item after a second separator is
        // not a heading, as after a single one.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nItem 1A. - see the risks described in Item 7 of this report.\n{body}\n\
             Item 2. Properties\n{body}\nItem 14. Principal Accountant Fees and Services\n{body}\n\
             Item 15. -- (a)(1)\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_14"]);
        assert!(s[0].text.contains("\nItem 1A. - see the risks"));
        let text = "Item 2.02 Results of Operations and Financial Condition.\nOn May 2, the Company...\n\
                    Item 7.01 -- furnished with Item 2.02 above.\nMore text.\n\
                    Item 9.01 Financial Statements and Exhibits.\n(d) Exhibits\nSIGNATURE\n";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_9.01"]);
    }

    #[test]
    fn dashes_between_part_and_item_are_one_separator() {
        // "PART I -- ITEM 2": two hyphens between the PART prefix and ITEM,
        // with the title on the same line or the next.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I -- ITEM 1. FINANCIAL STATEMENTS\n{body}\nPART I -- ITEM 2\nMANAGEMENT'S DISCUSSION AND ANALYSIS OF\n\
             FINANCIAL CONDITION AND RESULTS OF OPERATIONS\n{body}\nPART II -- ITEM 1 -- LEGAL PROCEEDINGS\n{body}\n\
             PART II—ITEM 6—EXHIBITS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[1].heading, "PART I -- ITEM 2");
        assert!(s[0].text.ends_with(body.trim()));
    }

    #[test]
    fn officers_section_numbered_as_a_sub_item_ends_the_item() {
        // "ITEM 4(a)" is the executive officers section, which the form puts
        // outside its Items, as with "Item 4A".
        let body = BODY.repeat(30);
        let text = format!(
            "Item 3.\tLegal Proceedings\t20\nItem 4.\tSubmission of Matters to a Vote of Security Holders\t35\n\
             Item 4(a)\tExecutive Officers of the Registrant\t35\nItem 5.\tMarket for the Registrant's Common Equity\t37\n\
             ITEM 3.\tLEGAL PROCEEDINGS\n{body}\n\
             ITEM 4.\tSUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNo matters were submitted.\n\
             ITEM 4(a). EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\nPART II\n\
             ITEM 5.\tMARKET FOR THE REGISTRANT'S COMMON EQUITY\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_4", "item_5"]);
        assert_eq!(
            s[1].text,
            "ITEM 4.\tSUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNo matters were submitted."
        );
        assert!(s[2].text.starts_with("ITEM 5.\tMARKET"));
        // With no Item 4 heading, the section is not Item 4 either.
        let text = format!(
            "Item 3. Legal Proceedings\n{body}\nItem 4 (a) - Executive Officers of the Registrant\n{body}\n\
             Item 5. Market for the Registrant's Common Equity\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_5"]);
        assert!(s[0].text.ends_with(body.trim()));
    }

    #[test]
    fn other_sub_items_and_unnumbered_officers_sections_stay_in_their_item() {
        // Paragraphs of an Item printed as titled sub-items, a sub-item of the
        // Item that covers executive officers, and an executive officers
        // section with no Item number all belong to the Item before them.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nInformation about our Executive Officers\n{body}\n\
             Item 4. Submission of Matters to a Vote of Security Holders\nNone.\n\
             Executive Officers of the Registrant\n{body}\nPART II\n\
             Item 5. Market for Registrant's Common Equity\n{body}\n\
             Item 10. Directors and Executive Officers of the Registrant\nSee the proxy statement.\n\
             Item 10(b). Executive Officers of the Registrant\n{body}\n\
             Item 11. Executive Compensation\n{body}\n\
             Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\n\
             Item 14(a). Financial Statements\n{body}\nItem 14(b). Reports on Form 8-K\nNone.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_4", "item_5", "item_10", "item_11", "item_14"]);
        assert!(s[0].text.contains("\nInformation about our Executive Officers\n"));
        assert!(s[1].text.contains("\nExecutive Officers of the Registrant\n"));
        assert!(s[3].text.contains("\nItem 10(b). Executive Officers of the Registrant\n"));
        assert!(s[5].text.ends_with("Item 14(b). Reports on Form 8-K\nNone."));
        // A 10-Q's "ITEM 6 (a). Exhibits" is Item 6, and "(b)" stays inside it.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nPART II\nITEM 6 (a). Exhibits\n{body}\n\
             ITEM 6 (b). Reports on Form 8-K\nNone.\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_6"]);
        assert!(s[1].text.ends_with("ITEM 6 (b). Reports on Form 8-K\nNone."));
    }

    #[test]
    fn lettered_headings_of_an_undefined_item_start_the_item_of_their_number() {
        // Item 5 is printed only as "Item 5A" and "Item 5B", and the list of
        // headings after the signatures repeats them.
        let body = BODY.repeat(30);
        let text = format!(
            "Part I\nItem 1. BUSINESS\n{body}\nItem 4. SUBMISSION OF MATTERS TO A VOTE OF SHAREHOLDERS\nNone.\n\
             Part II\nItem 5A. MARKET FOR REGISTRANT'S COMMON EQUITY AND RELATED SHAREHOLDER MATTERS\n{body}\n\
             Item 5B. EQUITY COMPENSATION PLAN INFORMATION\n{body}\nItem 6. SELECTED FINANCIAL DATA\n{body}\n\
             Signatures\nPursuant to...\nPart I Item 1. BUSINESS\nItem 4. SUBMISSION OF MATTERS TO A VOTE OF SHAREHOLDERS\n\
             Part II\nItem 5A. MARKET FOR REGISTRANT'S COMMON EQUITY AND RELATED SHAREHOLDER MATTERS\n\
             Item 5B. EQUITY COMPENSATION PLAN INFORMATION\nItem 6. SELECTED FINANCIAL DATA\nSignatures\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_4", "item_5", "item_6"]);
        assert_eq!(s[1].text, "Item 4. SUBMISSION OF MATTERS TO A VOTE OF SHAREHOLDERS\nNone.\nPart II");
        assert!(s[2].heading.starts_with("Item 5A. MARKET"));
        assert!(s[2].text.contains("\nItem 5B. EQUITY COMPENSATION PLAN INFORMATION\n"));
        assert!(s[2].text.ends_with(body.trim()));
        // In a 10-Q the form's order picks the part: Part II Item 2.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 2A. Unregistered Sales of Equity Securities and Use of Proceeds.\n{body}\n\
             Item 2B. Use of Proceeds from Registered Securities.\nNone.\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_6"]);
        assert!(s[3].heading.starts_with("Item 2A."));
        assert!(s[3].text.ends_with("Item 2B. Use of Proceeds from Registered Securities.\nNone."));
    }

    #[test]
    fn lettered_headings_do_not_replace_a_found_item_or_the_officers_section() {
        // With an Item 5 heading, "Item 5A" ends Item 5 as before. An
        // executive officers section numbered "Item 4A" never becomes Item 4,
        // even when the filing prints no Item 4 heading.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 4. Mine Safety Disclosures\nNot applicable.\nItem 4A. Executive Officers of the Registrant\n{body}\n\
             Item 5. Market for Registrant's Common Equity\nSee below.\n\
             Item 5A. Supplementary Market Information\n{body}\nItem 6. [Reserved]\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_4", "item_5", "item_6"]);
        assert_eq!(s[0].text, "Item 4. Mine Safety Disclosures\nNot applicable.");
        assert_eq!(s[1].text, "Item 5. Market for Registrant's Common Equity\nSee below.");
        let text = format!(
            "Item 3. Legal Proceedings\n{body}\nItem 4A. Executive Officers of the Registrant\n{body}\n\
             Item 5. Market for Registrant's Common Equity\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_5"]);
        assert!(s[0].text.ends_with(body.trim()));
        // After a 10-Q Item added out of order, the Items between it and the
        // next one in form order are already in the chain.
        let text = format!(
            "Item 2. Management's Discussion and Analysis\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             Part I. Financial Information\nItem 1. Financial Statements\n{body}\n\
             Item 2A. Supplementary Financial Information\n{body}\n\
             Part II. Other Information\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSignature\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_4", "part_1_item_1", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn officers_sub_item_keeps_its_number_when_the_lettered_item_exists_or_in_a_ten_q() {
        // "ITEM 1(a)" would read as Item 1A (Risk Factors), so it stays a
        // sub-item of Item 1. So does a 10-KSB's "ITEM 9(a)": its Item 9 names
        // executive officers, and in the 10-K Items it would read as Item 9A
        // (Controls and Procedures). A 10-Q has no executive officers caption.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\n{body}\nITEM 1(a). EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\n\
             ITEM 2. PROPERTIES\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2"]);
        assert!(s[0].text.contains("\nITEM 1(a). EXECUTIVE OFFICERS OF THE REGISTRANT\n"));
        for kind in [FormKind::TenKsb, FormKind::TenK] {
            for item_9 in ["", "ITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n"] {
                let text = format!(
                    "ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\nPART III\n{item_9}\
                     ITEM 9(a). DIRECTORS AND EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\n\
                     ITEM 9(b). COMPLIANCE WITH SECTION 16(a) OF THE EXCHANGE ACT\n{body}\n\
                     ITEM 10. EXECUTIVE COMPENSATION\n{body}\n"
                );
                let s = extract_sections(&text, kind);
                assert_eq!(ids(&s), ["item_8", "item_9", "item_10"], "{kind:?} {item_9}");
                assert!(s[1].text.contains("\nITEM 9(b). COMPLIANCE"), "{kind:?} {item_9}");
                assert!(s[1].text.ends_with(body.trim()), "{kind:?} {item_9}");
            }
        }
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 5(a). Appointment of Executive Officers\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_1", "part_2_item_5", "part_2_item_6"]);
        assert!(s[2].heading.starts_with("Item 5(a)."));
    }

    #[test]
    fn item_4a_is_the_officers_caption_whatever_its_title_says() {
        // With no Item 4 heading, a 10-K's "Item 4A" never starts Item 4: its
        // title may say only "Officers", wrap before "Executive Officers", or
        // be missing. A titled one still ends Item 3.
        let body = BODY.repeat(30);
        for (heading, ends_item_3) in [
            ("ITEM 4A. OFFICERS OF THE REGISTRANT", true),
            ("ITEM 4A. INFORMATION ABOUT OUR\nEXECUTIVE OFFICERS", true),
            ("ITEM 4A.\nName\tAge\tPosition", false),
        ] {
            let text = format!(
                "ITEM 3. LEGAL PROCEEDINGS\n{body}\n{heading}\n{body}\nPART II\n\
                 ITEM 5. MARKET FOR REGISTRANT'S COMMON EQUITY\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_3", "item_5"], "{heading}");
            assert_eq!(s[0].text.ends_with(body.trim()), ends_item_3, "{heading}");
        }
        // A 10-Q's "Item 4A(T). Controls and Procedures" is Part I Item 4.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 4A(T). Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_3", "part_1_item_4", "part_2_item_1"]);
    }

    #[test]
    fn lettered_ten_q_headings_take_their_part_from_a_part_line_or_their_title() {
        // Part I prints only Item 1. "Item 2A" and "Item 2B" are Part II Item
        // 2, as the PART line above them or the standard title says.
        let body = BODY.repeat(30);
        for (part_line, title) in [
            ("PART II - OTHER INFORMATION\n", "Sales of Unregistered Securities"),
            ("", "Unregistered Sales of Equity Securities and Use of Proceeds."),
        ] {
            let text = format!(
                "PART I - FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}\n\
                 {part_line}Item 2A. {title}\n{body}\nItem 2B. Use of Proceeds from Registered Securities\n{body}\n\
                 Item 6. Exhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_2", "part_2_item_6"], "{title}");
            assert!(s[1].text.contains("\nItem 2B. Use of Proceeds"), "{title}");
        }
        // Once "Item 2A" starts Part I Item 2, "Item 2B" stays inside it,
        // though Part II Item 2 is free as well.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2A. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 2B. Liquidity and Capital Resources\n{body}\nPART II\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 2B. Liquidity and Capital Resources\n"));
    }

    #[test]
    fn lettered_ten_q_headings_of_one_number_fill_both_parts() {
        // Part I prints MD&A as "Item 2A" and "Item 2B", and Part II prints
        // "Item 2A" again. The PART line just above it, its standard title or
        // the Part II Item before it puts the second "Item 2A" in Part II.
        let body = BODY.repeat(30);
        let legal = format!("Item 1. Legal Proceedings\n{body}\n");
        let no_item = format!("{body}\n{body}\n");
        for (between, title) in [
            ("", "Sales of Unregistered Securities"),
            (no_item.as_str(), "Unregistered Sales of Equity Securities and Use of Proceeds"),
            (legal.as_str(), "Sales of Unregistered Securities"),
        ] {
            let text = format!(
                "PART I\nItem 1. Financial Statements\n{body}\n\
                 Item 2A. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 {body}\nItem 2B. Liquidity and Capital Resources\n{body}\n\
                 PART II. OTHER INFORMATION\n{between}Item 2A. {title}\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            let mut want = vec!["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_6"];
            if between != legal {
                want.remove(2);
            }
            assert_eq!(ids(&s), want, "{title}");
            assert!(s[1].text.contains("\nItem 2B. Liquidity and Capital Resources\n"), "{title}");
            assert_eq!(s[want.len() - 2].heading, format!("Item 2A. {title}"));
        }
    }

    #[test]
    fn financial_statements_with_no_item_heading_end_the_item_before_them() {
        // No Item 8 heading: the statements follow Item 7A, which ends at the
        // auditors' report. The report on the schedule stays in Item 15.
        let body = BODY.repeat(30);
        let text = format!(
            "PART II\nITEM 7.MANAGEMENT'S DISCUSSION AND ANALYSIS.\n{body}\n\
             ITEM 7A.QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK.\n{body}\n\
             Report of Independent Registered Public Accounting Firm\nTo the Board of Directors\n{body}\n\
             THE COMPANY\nCONSOLIDATED BALANCE SHEETS\n{body}{body}\n\
             ITEM 9.CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE.\nNone.\n\
             ITEM 9A.CONTROLS AND PROCEDURES.\n{body}\nPART IV\n\
             ITEM 15.EXHIBITS, FINANCIAL STATEMENT SCHEDULES.\n(a)1.\tFinancial Statements\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_9", "item_9a", "item_15"]);
        assert_eq!(s[1].end, text.find("Report of Independent").unwrap());
        assert!(s[1].text.ends_with(body.trim()));
        assert!(s[4].text.contains("\nReport of Independent Registered Public Accounting Firm\n"));
        // The statements are printed between Item 9 and Part III, after an
        // Item 8 that only refers to them. Item 9 ends at the auditors' report.
        let text = format!(
            "Item 7. Management's Discussion and Analysis\n{body}\n\
             Item 8. Financial Statements\nSee Item 14 for index.\n\
             Item 9. Changes in and Disagreements with Accountants on Accounting and\nFinancial Disclosure\n\
             The information required by Item 304(a) was previously reported.\nExample Auditors LLP\n\
             Independent Auditors' Report\nThe Board of Directors\n{body}\nCONSOLIDATED BALANCE SHEETS\n{body}{body}\n\
             PART III\nItem 10. Directors and Executive Officers of the Registrant.\n{body}\nPART IV\n\
             Item 14. Exhibits, Financial Statement Schedules and Reports on Form 8-K\n(a) (1) Financial Statements\n\
             Independent Auditors' Reports.\nConsolidated Balance Sheets.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_9", "item_10", "item_14"]);
        assert!(s[2].text.ends_with("previously reported.\nExample Auditors LLP"));
        assert!(s[4].text.ends_with("Independent Auditors' Reports.\nConsolidated Balance Sheets."));
        // A report heading that names the firm first.
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             REPORT OF EXAMPLE AUDITORS LLP, INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             ITEM 10. DIRECTORS AND EXECUTIVE OFFICERS\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_10"]);
        assert_eq!(s[0].text, "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.");
    }

    #[test]
    fn blocks_printed_after_item_16_end_it() {
        // An exhibit index, a consent, or the financial statements printed
        // after the Form 10-K Summary and before the signatures.
        let body = BODY.repeat(30);
        let exhibits = "3.1\tCertificate of Incorporation\n10.1\tCredit Agreement\n23.1\tConsent of the auditors\n";
        let text = format!(
            "PART IV\nITEM 15. Exhibits and Financial Statement Schedules\n{body}\n\
             (b)Exhibits. The Exhibit Index, which immediately precedes the signature page, is incorporated.\n\
             Item 16. Form 10-K Summary\nNone\nEXHIBIT INDEX\nExhibits\n{exhibits}SIGNATURES\nIn accordance with ...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert!(s[0].text.ends_with("is incorporated."));
        assert_eq!(s[1].text, "Item 16. Form 10-K Summary\nNone");
        let text = format!(
            "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. FORM 10-K SUMMARY\nNone.\n\
             Consent of Independent Registered Public Accounting Firm\nWe hereby consent to the incorporation.\n\
             /s/ Example Auditors LLP\nSIGNATURES\nPursuant to ...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert_eq!(s[1].text, "ITEM 16. FORM 10-K SUMMARY\nNone.");
        let text = format!(
            "Item 15.\tExhibits, Financial Statement Schedules\n{body}\nItem 16.\tForm 10-K Summary\nNone.\n\
             EXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS\nPage\n\
             Report of Independent Registered Public Accounting Firm\t39\nConsolidated Statements of Operations\t40\n\
             Consolidated Balance Sheets\t41\nREPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             EXAMPLE CORP\nCONSOLIDATED STATEMENTS OF OPERATIONS\n{body}{body}{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert_eq!(s[1].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap());
        // Financial statement schedules after Item 16, below a page footer.
        for schedule in [
            "SCHEDULE I – CONDENSED\nPARENT COMPANY FINANCIAL STATEMENTS",
            "SCHEDULE I-SUMMARY OF INVESTMENTS-OTHER THAN\nINVESTMENTS IN RELATED PARTIES",
        ] {
            let text = format!(
                "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. FORM 10-K SUMMARY\nNone.\n\
                 2025 Form 10-K\t160\tExample Corp\n{schedule}\n{body}\nSCHEDULE II\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_15", "item_16"]);
            assert_eq!(s[1].text, "ITEM 16. FORM 10-K SUMMARY\nNone.\n2025 Form 10-K\t160\tExample Corp");
        }
    }

    #[test]
    fn item_16_titled_as_the_exhibits_item_keeps_its_statements_and_exhibit_index() {
        // Some 10-Ks for 2003 kept the numbering of 2002: Item 14 Controls and
        // Procedures, Item 15 Accountant Fees and Item 16 Exhibits, whose list
        // of statements and exhibit index belong to it.
        let body = BODY.repeat(30);
        for heading in [
            "Item 16. Exhibits, Financial Statement Schedules, and Reports on Form 8-K",
            "ITEM 16.\nFINANCIAL STATEMENTS, SCHEDULES, EXHIBITS AND REPORTS ON FORM 8-K",
        ] {
            let text = format!(
                "PART III\nItem 14. Controls and Procedures\n{body}\nItem 15. Principal Accountant Fees and Services\n\
                 {body}\nPART IV\n{heading}\n(a) 1. Financial Statements\nIndependent Auditors' Report\tF-2\n\
                 Consolidated Balance Sheets\tF-3\n2. Financial Statement Schedules\n\
                 Schedule II - Valuation and Qualifying Accounts\tS-1\n(b) Reports on Form 8-K\nNone.\n\
                 (c) Exhibits\nEXHIBIT INDEX\n3.1\tArticles of Incorporation\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15", "item_16"], "{heading}");
            assert!(s[2].text.ends_with("EXHIBIT INDEX\n3.1\tArticles of Incorporation"), "{heading}");
        }
    }

    #[test]
    fn statements_with_pages_of_their_own_end_the_exhibits_item() {
        // The statement pages, numbered F-1, F-2, ..., follow Item 15's text
        // with their own index and no Item heading. The Item ends at the
        // index; the company name printed above it as a page header stays.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 14.\tControls and Procedures\n{body}\nPART IV\n\
             Item 15.\tExhibits, Financial Statement Schedules, and Reports on Form 8-K\n\
             (a)\tThe following documents are filed as part of this report:\n\
             (1)\tFinancial Statements: See Index to Financial Statements on page F-1.\n\
             3.1\tAmended and Restated Certificate of Incorporation.\n23.1\tConsent of Example LLP, Independent Auditors.\n\
             (b)\tWe did not file any current reports on Form 8-K.\nEXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS\nPage\n\
             Independent Auditors' Report\tF-2\nBalance Sheets as of December 31, 2001 and 2002\tF-3\n\
             Statements of Operations for the years ended December 31, 2000, 2001 and 2002\tF-4\n\
             Notes to Financial Statements\tF-7\nINDEPENDENT AUDITORS' REPORT\nThe Board of Directors\n{body}\n\
             EXAMPLE CORP\nBALANCE SHEETS\n{body}\nEXAMPLE CORP\nSTATEMENTS OF OPERATIONS\n{body}\n\
             NOTES TO FINANCIAL STATEMENTS\n{body}{body}\nSIGNATURES\nPursuant to ...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_14", "item_15"]);
        assert_eq!(s[1].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap());
        // The Item lists the statements with their pages as its own text and
        // prints them after its exhibit list, so it keeps the list and ends at
        // the first statement. Pages alone on a line below wrapped titles
        // count, and so does a 10-K's Item 14 from before 2003.
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 14. EXHIBITS, FINANCIAL STATEMENTS, SCHEDULES AND REPORTS ON FORM 8-K\n(a)(1) Financial Statements:\n\
             Report of Independent Auditors (Example LLP)..............   F-2\n\
             Consolidated Balance Sheets at December 31, 1998 and 1997\nF-3\n\
             Consolidated Statements of Loss for the twelve-month periods\nended December 31, 1998, 1997 and 1996\nF-4\n\
             Notes to Consolidated Financial Statements\nF-5\n(b) Exhibits\n3.1 Articles of Incorporation (1)\n\
             (1) Previously filed.\nEXAMPLE CORP\nREPORT OF INDEPENDENT AUDITORS\n{body}\n\
             CONSOLIDATED BALANCE SHEETS\n{body}{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_14"]);
        assert_eq!(s[1].end, text.find("REPORT OF INDEPENDENT AUDITORS").unwrap());
        assert!(s[1].text.contains("\nReport of Independent Auditors (Example LLP)"));
        // The same with an index heading: the index is still the Item's list.
        let text = format!(
            "ITEM 15.\tEXHIBITS, FINANCIAL STATEMENT SCHEDULES\n(a)\tFinancial Statements\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Auditors\tF-2\nBalance Sheets\tF-3\nStatements of Operations\tF-4\n\
             (b)\tExhibits\n3.1\tArticles of Incorporation\nREPORT OF INDEPENDENT AUDITORS\n{body}\n\
             BALANCE SHEETS\n{body}{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[0].end, text.find("REPORT OF INDEPENDENT AUDITORS").unwrap());
        // A 10-KSB's exhibits Item, with the statements before Item 14.
        let text = format!(
            "ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K\n{body}\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Accountants\tF-1\nBalance Sheet\tF-2\nStatements of Operations\tF-3\n\
             REPORT OF INDEPENDENT ACCOUNTANTS\n{body}\nBALANCE SHEET\n{body}{body}\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_13", "item_14"]);
        assert_eq!(s[0].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap());
        assert_eq!(s[1].start, text.find("ITEM 14.").unwrap());
    }

    #[test]
    fn statements_stay_in_the_exhibits_item_unless_paginated_apart_and_printed_after_it() {
        let body = BODY.repeat(30);
        let printed = format!(
            "REPORT OF INDEPENDENT AUDITORS\n{body}\nCONSOLIDATED BALANCE SHEETS\n{body}\n\
             CONSOLIDATED STATEMENTS OF OPERATIONS\n{body}{body}\n"
        );
        let f_index = "Report of Independent Auditors\tF-2\nConsolidated Balance Sheets\tF-3\n\
                       Consolidated Statements of Operations\tF-4\nNotes to Consolidated Financial Statements\tF-5\n";
        let cases = [
            // Pages numbered as the report's, or as an annual report's.
            format!(
                "INDEX TO FINANCIAL STATEMENTS\tPage\nReport of Independent Auditors\t24\n\
                 Consolidated Balance Sheets\t25\nConsolidated Statements of Operations\t26\n{printed}"
            ),
            // Fewer than three F-page entries.
            format!("Report of Independent Auditors\tF-2\nConsolidated Balance Sheets\tF-3\n{printed}"),
            // Only one statement printed in the Item: the auditors' report on
            // a schedule paginated apart.
            format!(
                "{f_index}(a)(2) Schedule II - Valuation and Qualifying Accounts\tS-2\n(a)(3) Exhibits\n\
                 3.1\tArticles of Incorporation\nREPORT OF INDEPENDENT AUDITORS ON SCHEDULE\n{body}\n\
                 SCHEDULE II - VALUATION AND QUALIFYING ACCOUNTS\n{body}\n"
            ),
            // Entries of a second index, paginated another way, repeat the
            // statement titles.
            format!(
                "{f_index}(a)(2) Schedules of the parent company:\nReport of Independent Auditors\tS-1\n\
                 Consolidated Balance Sheets\tS-2\n(a)(3) Exhibits\n3.1\tArticles of Incorporation\n"
            ),
            // The statements come between the Item's text and its exhibit index.
            format!("{f_index}(a)(3) Exhibits: see the Exhibit Index.\n{printed}EXHIBIT INDEX\n3.1\tArticles\n"),
            // The statements follow the signatures.
            format!("{f_index}(a)(3) Exhibits\n3.1\tArticles of Incorporation\nSIGNATURES\n{printed}"),
        ];
        for (i, case) in cases.iter().enumerate() {
            let text = format!(
                "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a)(1) Financial Statements\n{case}SIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"], "case {i}");
            assert_eq!(s[1].end, text.find("SIGNATURES").unwrap(), "case {i}");
        }
        // The Items that hold the statements keep them: 10-K Item 8 and
        // 10-KSB Item 7.
        for (kind, statements, next) in [
            (FormKind::TenK, "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA", "ITEM 9. CHANGES IN ACCOUNTANTS"),
            (FormKind::TenKsb, "ITEM 7. FINANCIAL STATEMENTS", "ITEM 8. CHANGES IN ACCOUNTANTS"),
        ] {
            let text = format!("{statements}\nINDEX TO FINANCIAL STATEMENTS\n{f_index}{printed}{next}\nNone.\n");
            let s = extract_sections(&text, kind);
            assert_eq!(s.len(), 2, "{statements}");
            assert_eq!(s[0].end, text.find(next).unwrap(), "{statements}");
        }
    }

    #[test]
    fn glossary_after_the_item_after_the_exhibits_ends_it() {
        let body = BODY.repeat(30);
        let glossary = "In this report the following terms have the meanings specified below.\n\
                        Gross acre. An acre in which the trust owns a working interest.\n\
                        MMcf. Million cubic feet of natural gas.\n";
        for heading in ["GLOSSARY", "Glossary", "Glossary of Terms and Acronyms", "GLOSSARY OF SELECTED TERMS:"] {
            let text = format!(
                "Item 15. Exhibits and Financial Statement Schedules\n{body}\nItem 16. Form 10-K Summary\n\
                 Not applicable.\n{heading}\n{glossary}SIGNATURES\nPursuant to ...\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_15", "item_16"], "{heading}");
            assert_eq!(s[1].text, "Item 16. Form 10-K Summary\nNot applicable.", "{heading}");
        }
        // A glossary printed as a table before the exhibit index, and one
        // after a 10-KSB's Item 14.
        let text = format!(
            "Item 15. Exhibits and Financial Statement Schedules\n{body}\nItem 16. Form 10-K Summary\nNone.\n\
             Glossary\nThroughout this Form 10-K, the Company uses certain abbreviations, acronyms and terms.\n\
             ABO\tAccumulated Benefit Obligations\tFINRA\tFinancial Industry Regulatory Authority\n\
             Exhibit Index\n3.1\tCharter\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].text, "Item 16. Form 10-K Summary\nNone.");
        let text = format!(
            "ITEM 13. EXHIBITS\n{body}\nITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             GLOSSARY OF TERMS\n{glossary}SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_13", "item_14"]);
        assert_eq!(s[1].end, text.find("GLOSSARY OF TERMS").unwrap());
    }

    #[test]
    fn glossaries_inside_other_items_stay_there() {
        // A glossary at the start of Item 1, in MD&A, and in Item 7A of a
        // filing with no Item 8 heading, where a block heading would end it.
        // In Item 16, lines that only mention the glossary stay.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\nGLOSSARY OF TERMS\nMMcf. Million cubic feet.\n{body}\n\
             ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\nGlossary\nABO\tAccumulated Benefit Obligations\n{body}\n\
             ITEM 7A. QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK\n{body}\nGlossary of Market Terms\n\
             VaR\tValue at risk\n{body}\nITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. FORM 10-K SUMMARY\n\
             Glossary terms are defined in Item 1.\nGlossary of terms used in this summary appears on page 3.\n\
             glossary\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_7a", "item_9", "item_15", "item_16"]);
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(text.find("SIGNATURES").unwrap(), |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
    }

    #[test]
    fn sections_that_also_hold_the_next_item_keep_its_text() {
        // Item 9A's heading reads as Item 9, or one heading names both Items:
        // the auditors' report on internal control is Item 9A's text, which
        // the section of Item 9 holds. "ITEM 9(A)" with another title than
        // Item 9A's standard one stays a sub-item of Item 9.
        let body = BODY.repeat(30);
        for heading in [
            "ITEM 9(A). DISCLOSURE CONTROLS AND PROCEDURES",
            "ITEM 9 A – CONTROLS AND PROCEDURES",
            "Item 9(A)(T). Evaluation of Disclosure Controls",
        ] {
            let text = format!(
                "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\n\
                 ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\n\
                 None.\n{heading}\n{body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
                 ITEM 9B. OTHER INFORMATION\nNone.\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_9b"], "{heading}");
            assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap(), "{heading}");
        }
        let text = format!(
            "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\n\
             ITEMS 9 AND 9A. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS; CONTROLS AND PROCEDURES\nNone.\n\
             {body}\nReport of Independent Registered Public Accounting Firm\n{body}\nITEM 9B. OTHER INFORMATION\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9", "item_9b"]);
        assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap());
        // Item 7A's heading names Item 8, so the statements are Item 8's.
        let text = format!(
            "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\n\
             ITEMS 7A AND 8. QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK; FINANCIAL STATEMENTS\n\
             {body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_9"]);
        assert_eq!(s[1].end, text.find("ITEM 9.").unwrap());
        // A page header that repeats Item 9's heading is no second heading.
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS (continued)\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9A. CONTROLS AND PROCEDURES\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_9a"]);
        assert_eq!(s[0].end, text.find("Report of Independent").unwrap());
    }

    #[test]
    fn auditors_reports_and_exhibit_indexes_inside_an_item_stay_there() {
        // Items 7 and 7A when Item 8 has a heading, Item 8 itself, the report
        // on internal control in Items 9A and 9B, and Item 15's statements,
        // schedule, exhibit index and consent all keep these headings.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 7A. QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\nIndex to Consolidated Financial Statements\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 9A. CONTROLS AND PROCEDURES\n{body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 9B. OTHER INFORMATION\nManagement's Report on Internal Control\n{body}\n\
             REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\nPART IV\n\
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a) 1. Financial Statements\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             SCHEDULE II – VALUATION AND QUALIFYING ACCOUNTS\n{body}\nEXHIBIT INDEX\n{body}\n\
             Consent of Independent Registered Public Accounting Firm\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_8", "item_9", "item_9a", "item_9b", "item_15"]);
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(text.find("SIGNATURES").unwrap(), |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
        // Lines that only start like these headings, and an 8-K's Item 9.01.
        let text = format!(
            "Item 9. Changes in and Disagreements with Accountants\nThe former accountants' report and the\n\
             report of independent accountants on the 1997 statements contained no adverse opinion.\n\
             Item 9A. Controls and Procedures\n{body}\n\
             Item 16. Form 10-K Summary\nThe summary below refers to the Exhibit Index.\n\
             See Exhibit Index for the exhibits filed with this report.\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_9a", "item_16"]);
        assert!(s[0].text.ends_with("contained no adverse opinion."));
        assert!(s[2].text.ends_with(body.trim()));
        let text = "Item 2.01 Completion of Acquisition or Disposition of Assets\nThe Company acquired Example Corp.\n\
                    Item 9.01 Financial Statements and Exhibits\n(a) Financial statements of the business acquired.\n\
                    Report of Independent Auditors\nWe have audited the balance sheet.\nSIGNATURE\n";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.01", "item_9.01"]);
        assert!(s[1].text.ends_with("We have audited the balance sheet."));
    }

    /// Titles of the sections, in order.
    fn titles(sections: &[Section]) -> Vec<&str> {
        sections.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn ten_ksb_items_have_their_own_numbers_and_titles() {
        // Item 6 is MD&A or plan of operation, Item 7 the financial
        // statements with their auditors' report, and Items 8A and 8B are
        // Items of the form. Read with the 10-K Items, "ITEM 8A." and "ITEM
        // 8B." would end Item 8 and their text would be in no section.
        let body = BODY.repeat(30);
        let text = format!(
            "FORM 10-KSB\nPART I\nITEM 1. DESCRIPTION OF BUSINESS\n{body}\nITEM 2. DESCRIPTION OF PROPERTY\n{body}\n\
             PART II\nITEM 5. MARKET FOR COMMON EQUITY AND RELATED STOCKHOLDER MATTERS\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 8A. CONTROLS AND PROCEDURES\n{body}\nITEM 8B. OTHER INFORMATION\nNone.\n\
             PART III\nITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n{body}\n\
             ITEM 13. EXHIBITS\nEXHIBIT INDEX\n3.1\tArticles of Incorporation\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let kind = FormKind::from_form("10KSB").expect("a 10-KSB has Items");
        let s = extract_sections(&text, kind);
        assert_eq!(
            ids(&s),
            [
                "item_1", "item_2", "item_5", "item_6", "item_7", "item_8", "item_8a", "item_8b", "item_9", "item_13",
                "item_14"
            ]
        );
        assert_eq!(
            titles(&s),
            [
                "Description of Business",
                "Description of Property",
                "Market for Common Equity and Related Stockholder Matters and Small Business Issuer Purchases of Equity \
                 Securities",
                "Management's Discussion and Analysis or Plan of Operation",
                "Financial Statements",
                "Changes in and Disagreements with Accountants on Accounting and Financial Disclosure",
                "Controls and Procedures",
                "Other Information",
                "Directors, Executive Officers, Promoters, Control Persons and Corporate Governance; Compliance with \
                 Section 16(a) of the Exchange Act",
                "Exhibits",
                "Principal Accountant Fees and Services",
            ]
        );
        assert!(s[4].text.contains("\nReport of Independent Registered Public Accounting Firm\n"));
        assert!(s[5].text.ends_with("FINANCIAL DISCLOSURE\nNone."));
        assert!(s[6].text.ends_with(body.trim()));
        assert!(s[9].text.ends_with("EXHIBIT INDEX\n3.1\tArticles of Incorporation"));
    }

    #[test]
    fn ten_ksb_items_8a_and_8b_printed_as_sub_items() {
        // "Item 8(A)T." and "Item 8(B)." with the standard titles of Items 8A
        // and 8B are those Items, also in the contents.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 7. Financial Statements.\nItem 8. Changes in and Disagreements with Accountants\n\
             Item 8(A)T. Controls and Procedures.\nItem 8(B). Other Information.\nItem 9. Directors\n\
             Item 7. Financial Statements.\n{body}\n\
             Item 8. Changes in and Disagreements with Accountants\nNone; Not applicable.\n\
             Item 8(A)T. Controls and Procedures.\n{body}\nItem 8(B). Other Information.\n{body}\n\
             Item 9. Directors\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_8a", "item_8b", "item_9"]);
        assert_eq!(s[1].text, "Item 8. Changes in and Disagreements with Accountants\nNone; Not applicable.");
        assert_eq!(s[2].heading, "Item 8(A)T. Controls and Procedures.");
        // The same in a 10-K: "ITEM 9(A)" and "Item 9(A)(T)" with Item 9A's
        // title are Item 9A, whose report on internal control stays with it.
        for heading in ["ITEM 9(A). CONTROLS AND PROCEDURES", "Item 9(A)(T). Controls and Procedures"] {
            let text = format!(
                "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n{heading}\n{body}\n\
                 Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9a", "item_9b"], "{heading}");
            assert_eq!(s[0].text, "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.", "{heading}");
            assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap(), "{heading}");
        }
        // And in a 10-Q's Part II.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 1(A). Risk Factors\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]);
    }

    #[test]
    fn sub_items_with_other_titles_stay_in_their_item() {
        // A sub-item is a lettered Item only with that Item's standard title:
        // "ITEM 1(a). GENERAL DEVELOPMENT OF BUSINESS" is not Item 1A, a
        // 10-KSB's "Item 8(A). Evaluation of Disclosure Controls" is not Item
        // 8A, and "Item 7(a)" names no Item of a 10-KSB.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\nITEM 1(a). GENERAL DEVELOPMENT OF BUSINESS\n{body}\n\
             ITEM 1(b). RISK FACTORS RELATING TO OUR BUSINESS\n{body}\nITEM 2. PROPERTIES\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2"]);
        let text = format!(
            "ITEM 7. FINANCIAL STATEMENTS\n{body}\nITEM 7(a). FINANCIAL STATEMENTS OF BUSINESSES ACQUIRED\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             Item 8(A). Evaluation of Disclosure Controls\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9. DIRECTORS\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_9"]);
        assert!(s[0].text.contains("\nITEM 7(a). FINANCIAL STATEMENTS OF BUSINESSES ACQUIRED\n"));
        // Item 8's section holds Item 8A's text, report included.
        assert_eq!(s[1].end, text.find("ITEM 9.").unwrap());
    }

    #[test]
    fn ten_ksb_text_with_no_item_heading_ends_items_8_and_14() {
        // The 10-KSB Items that never hold financial statements or an exhibit
        // index are Item 8 (changes in accountants) and Item 14 (accountant
        // fees, after the exhibits). Statements printed after either with no
        // heading of their own belong to no Item.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 7. FINANCIAL STATEMENTS\nThe financial statements follow Item 8.\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             INDEPENDENT AUDITORS' REPORT\nTo the Board of Directors\n{body}\nBALANCE SHEETS\n{body}{body}\n\
             ITEM 8A. CONTROLS AND PROCEDURES\n{body}\nPART III\n\
             ITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n{body}\n\
             ITEM 13. EXHIBITS\n3.1\tArticles of Incorporation\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             EXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS\nReport of Independent Registered Public Accounting Firm\tF-1\n\
             {body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_8a", "item_9", "item_13", "item_14"]);
        assert_eq!(s[1].text, "ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.");
        assert_eq!(s[5].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap());
        assert!(s[5].text.ends_with("tempor. \nEXAMPLE CORP"));
    }

    #[test]
    fn ten_ksb_items_that_hold_statements_or_exhibits_keep_them() {
        // Item 7 (the statements), Item 8A with an auditors' report on
        // internal control, Item 13 (the exhibits) and an Item 14 titled as
        // the exhibits Item keep these headings. So does Item 6, MD&A, when
        // the statements follow it with no Item 7 heading, as 10-K Item 7 does.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nBALANCE SHEETS\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 8A. CONTROLS AND PROCEDURES\n{body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 13. EXHIBITS\n{body}\nINDEX TO EXHIBITS\n3.1\tArticles of Incorporation\n\
             ITEM 14. EXHIBITS, FINANCIAL STATEMENTS AND REPORTS ON FORM 8-K\n(a) Financial Statements\n\
             INDEX TO FINANCIAL STATEMENTS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_6", "item_8", "item_8a", "item_13", "item_14"]);
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(text.find("SIGNATURES").unwrap(), |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
        let text = format!(
            "ITEM 7. FINANCIAL STATEMENTS\nINDEX TO FINANCIAL STATEMENTS\nBalance Sheets\tF-2\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nBALANCE SHEETS\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8"]);
        assert_eq!(s[0].end, text.find("ITEM 8.").unwrap());
    }

    #[test]
    fn ten_ksb_officers_caption_is_no_item() {
        // As in a 10-K, "Item 4A" or "Item 4(a)" with an executive officers
        // title ends Item 4 and starts no Item, also when there is no Item 4
        // heading.
        let body = BODY.repeat(30);
        for officers in
            ["ITEM 4A. EXECUTIVE OFFICERS OF THE REGISTRANT", "ITEM 4(a). EXECUTIVE OFFICERS OF THE COMPANY"]
        {
            let text = format!(
                "ITEM 3. LEGAL PROCEEDINGS\n{body}\nITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\n\
                 None.\n{officers}\n{body}\nPART II\nITEM 5. MARKET FOR COMMON EQUITY\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenKsb);
            assert_eq!(ids(&s), ["item_3", "item_4", "item_5"], "{officers}");
            assert_eq!(s[1].text, "ITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.", "{officers}");
        }
        let text = format!(
            "ITEM 3. LEGAL PROCEEDINGS\n{body}\nITEM 4A.\nName\tAge\tPosition\n{body}\n\
             ITEM 5. MARKET FOR COMMON EQUITY\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_3", "item_5"]);
    }

    #[test]
    fn ten_qsb_items_have_their_own_titles_and_parts() {
        // Part I has three Items, Item 3 being controls and procedures. With
        // no PART lines, the titles put "Item 3. Controls and Procedures" in
        // Part I and "Item 1. Legal Proceedings" in Part II, and a wrapped
        // cross-reference to Part I Item 3 is not a heading.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\n{body}{body}\n\
             Item 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures, of this report describes our controls.\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nItem 1. Legal Proceedings\nNone.\n\
             Item 2. Changes in Securities\nNone.\nItem 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let kind = FormKind::from_form("10QSB").expect("a 10-QSB has Items");
        let s = extract_sections(&text, kind);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_2", "part_2_item_6"]
        );
        assert_eq!(
            titles(&s),
            [
                "Financial Statements",
                "Management's Discussion and Analysis or Plan of Operation",
                "Controls and Procedures",
                "Legal Proceedings",
                "Unregistered Sales of Equity Securities and Use of Proceeds",
                "Exhibits",
            ]
        );
        assert!(s[1].text.contains("\nItem 3. Controls and Procedures, of this report"));
        // Financial statements printed after MD&A under a PART I line are
        // added out of order, as in a 10-Q.
        let text = format!(
            "PART I\nItem 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nPART I\nItem 1. Financial Statements\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_1_item_1", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn titles_that_start_with_a_form_name_are_headings() {
        // "ITEM 16. 10-K SUMMARY" starts Item 16, which then ends at the
        // auditors' report printed after it.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. 10-K SUMMARY\nNone.\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert_eq!(s[1].text, "ITEM 16. 10-K SUMMARY\nNone.");
        // Numbers, and a form name followed by a lowercase word or by more
        // digits, start no heading.
        let text = format!(
            "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nItem 16 10,000 shares were issued.\n\
             Item 16 10-K filed in 1998.\nItem 16 10-K405 Annual Report\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15"]);
    }

    #[test]
    fn lettered_sub_item_with_its_title_on_the_next_line() {
        // "ITEM 9(A)." with Item 9A's standard title on the line below is
        // Item 9A, as it is with the title on its own line.
        let body = BODY.repeat(30);
        let frame = |heading: &str| {
            format!(
                "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n{heading}\n{body}\n\
                 Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n"
            )
        };
        for heading in [
            "ITEM 9(A).\nCONTROLS AND PROCEDURES",
            "Item 9(a).\n\nControls and Procedures",
            "ITEM 9(A)T\nCONTROLS AND PROCEDURES",
        ] {
            let text = frame(heading);
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9a", "item_9b"], "{heading}");
            assert_eq!(s[0].text, "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.", "{heading}");
            assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap(), "{heading}");
        }
        // Another title below, a sub-item reference, or running text keep the
        // line inside Item 9.
        for heading in [
            "ITEM 9(A).\nDISCLOSURE CONTROLS AND PROCEDURES",
            "Item 9(a)(1)\nControls and Procedures",
            "Item 9(a).\nThe Company's controls and procedures are described below.",
        ] {
            let s = extract_sections(&frame(heading), FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9b"], "{heading}");
        }
    }

    #[test]
    fn statements_in_the_middle_of_the_exhibits_item_stay_in_it() {
        // The exhibits Item prints its list of the statements, then the
        // statements with pages of their own, then more of its own text: the
        // schedules, its exhibit list under a caption or a table header, or
        // its reports on Form 8-K. The statements stay in the Item.
        let body = BODY.repeat(30);
        let index = "INDEX TO CONSOLIDATED FINANCIAL STATEMENTS\n\
                     Report of Independent Registered Public Accounting Firm\tF-2\nConsolidated Balance Sheets\tF-3\n\
                     Consolidated Statements of Operations\tF-4\nNotes to Consolidated Financial Statements\tF-5\n";
        let printed = format!(
            "REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\nCONSOLIDATED BALANCE SHEETS\n{body}\n\
             CONSOLIDATED STATEMENTS OF OPERATIONS\n{body}\nNOTES TO CONSOLIDATED FINANCIAL STATEMENTS\n\
             1. Summary of Significant Accounting Policies\n(a) Basis of presentation\n{body}\n"
        );
        let frame = |after: &str| {
            format!(
                "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a) 1. Financial Statements\n\
                 {index}{printed}{after}SIGNATURES\n"
            )
        };
        for after in [
            "2. Financial Statement Schedules\nSchedules are omitted because they are not applicable.\n",
            "3. Exhibits\nThe following exhibits are filed with this report:\n10.1\tCredit Agreement\n",
            "(a)(3)  Exhibits:\n        10.1   Credit Agreement\n",
            "(b) Reports on Form 8-K. None.\n",
            "LIST OF EXHIBITS\n10.1\tCredit Agreement\n",
            "Exhibits:\n10.1\tCredit Agreement\n",
            "Exhibit No.\tDescription\n10.1\tCredit Agreement\n",
        ] {
            let text = frame(after);
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"], "{after}");
            assert_eq!(s[1].end, text.find("SIGNATURES").unwrap(), "{after}");
        }
        // Numbered notes and their lettered paragraphs are the statements'
        // own text, so the statements still end the Item, at their index.
        let text = frame("2. Inventories\n(b) Use of estimates\nExhibit 10.1 to the credit agreement sets the rate.\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].end, text.find("INDEX TO CONSOLIDATED").unwrap());
    }

    /// Asserts that each section runs to the next one, and the last one to
    /// `last_end`.
    fn assert_contiguous(s: &[Section], last_end: usize) {
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(last_end, |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
    }

    #[test]
    fn ten_k_numbered_as_a_ten_ksb_is_split_with_its_items() {
        // Some small companies' first 10-Ks, for 2008, kept the 10-KSB's
        // Items: Item 7 holds the statements, here with pages of their own,
        // and Items 8A and 8B follow Item 8. Read with the 10-K Items, Item 7
        // would be MD&A, cut at its statements as if it were the exhibits
        // Item, and Item 8A's text would be in no section.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. DESCRIPTION OF BUSINESS\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Registered Public Accounting Firm\tF-1\nBalance Sheets\tF-2\n\
             Statements of Operations\tF-3\nNotes to Financial Statements\tF-4\n\
             REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\nBALANCE SHEETS\n{body}\n\
             STATEMENTS OF OPERATIONS\n{body}\nNOTES TO FINANCIAL STATEMENTS\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 8A(T). CONTROLS AND PROCEDURES\n{body}\nITEM 8B. OTHER INFORMATION\nNone.\n\
             ITEM 10. EXECUTIVE COMPENSATION\n{body}\nITEM 13. EXHIBITS\n3.1\tArticles of Incorporation\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(
            ids(&s),
            ["item_1", "item_6", "item_7", "item_8", "item_8a", "item_8b", "item_10", "item_13", "item_14"]
        );
        assert_eq!(s[2].title, "Financial Statements");
        assert_eq!(s[7].title, "Exhibits");
        assert_contiguous(&s, text.find("SIGNATURES").unwrap());
        // An amendment that prints only Item 8A(T) and Item 14, which both
        // forms number 14.
        let text = format!(
            "ITEM 8A(T). CONTROLS AND PROCEDURES\n{body}\nITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::from_form("10-K/A").expect("a 10-K/A has Items"));
        assert_eq!(ids(&s), ["item_8a", "item_14"]);
    }

    #[test]
    fn items_before_the_statements_item_are_never_the_exhibits_item() {
        // Headings that fit the 10-K and the 10-KSB numbering equally keep
        // the 10-K's Items. Item 7 printed as "Financial Statements" comes
        // before the 10-K's statements Item, so it is not the exhibits Item
        // and keeps statements whose pages are numbered apart.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Registered Public Accounting Firm\tF-1\nBalance Sheets\tF-2\n\
             Statements of Operations\tF-3\nREPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             BALANCE SHEETS\n{body}\nSTATEMENTS OF OPERATIONS\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 9A. CONTROLS AND PROCEDURES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_6", "item_7", "item_9", "item_9a"]);
        assert_eq!(s[1].title, "Management's Discussion and Analysis of Financial Condition and Results of Operations");
        assert_contiguous(&s, text.find("SIGNATURES").unwrap());
    }

    #[test]
    fn ten_ksb_numbered_as_a_ten_k_is_split_with_its_items() {
        // A 10-KSB printed with Form 10-K's Items: the statements are Item 8,
        // and Items 1A, 9A, 9B and 15 are Items. Read with the 10-KSB Items,
        // Item 8 would end at the auditors' report and Items 9A, 9B and 15
        // would end the Items before them, leaving their text in no section.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\n{body}\nITEM 1A. RISK FACTORS\n{body}\nITEM 2. PROPERTIES\n{body}\n\
             ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS\n{body}\n\
             ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nBALANCE SHEETS\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 9A(T). CONTROLS AND PROCEDURES\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n\
             ITEM 11. EXECUTIVE COMPENSATION\n{body}\nITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES\n\
             3.1\tArticles of Incorporation\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(
            ids(&s),
            ["item_1", "item_1a", "item_2", "item_7", "item_8", "item_9", "item_9a", "item_9b", "item_11", "item_15"]
        );
        assert_eq!(s[4].title, "Financial Statements and Supplementary Data");
        assert_eq!(s[9].title, "Exhibits and Financial Statement Schedules");
        assert_contiguous(&s, text.find("SIGNATURES").unwrap());
    }

    #[test]
    fn ten_ksb_risk_factors_and_item_15_are_items() {
        // 10-KSBs print Item 1A, Risk Factors, as 10-Ks do, and those for
        // 2003 Item 14, Controls and Procedures, then Item 15, Principal
        // Accountant Fees and Services. Item 15 comes after the exhibits, so
        // statements printed after it belong to no Item, as after Item 14.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. DESCRIPTION OF BUSINESS\n{body}\nITEM 1A. RISK FACTORS\n{body}\n\
             ITEM 2. DESCRIPTION OF PROPERTY\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\n{body}\nITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K\n3.1\tArticles of Incorporation\n\
             ITEM 14. CONTROLS AND PROCEDURES\n{body}\nITEM 15. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             INDEPENDENT AUDITORS' REPORT\n{body}\nBALANCE SHEETS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(
            ids(&s),
            ["item_1", "item_1a", "item_2", "item_6", "item_7", "item_8", "item_13", "item_14", "item_15"]
        );
        assert_eq!(s[1].title, "Risk Factors");
        assert_eq!(s[4].title, "Financial Statements");
        assert_eq!(s[8].title, "Principal Accountant Fees and Services");
        assert_contiguous(&s, text.find("INDEPENDENT AUDITORS' REPORT").unwrap());
    }

    #[test]
    fn ten_qsb_risk_factors_and_ten_q_numbering() {
        // A 10-QSB's Part II Item 1A, Risk Factors, is an Item, as in a 10-Q.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 1A. Risk Factors\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]
        );
        assert_eq!(s[2].title, "Controls and Procedures");
        assert_eq!(s[3].text, "Item 1. Legal Proceedings\nNone.");
        // A 10-QSB numbered as a 10-Q, with market risk as Part I Item 3 and
        // controls as Item 4, is split with the 10-Q's Items.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot applicable.\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]
        );
        assert_eq!(s[2].text, "Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot applicable.");
        assert_eq!(s[3].title, "Controls and Procedures");
        // And a 10-Q numbered as a 10-QSB with the 10-QSB's.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[2].title, "Controls and Procedures");
    }

    #[test]
    fn titles_that_both_numberings_print_keep_the_form_items() {
        // "Submission of Matters to a Vote of Security Holders" is 10-KSB
        // Item 4's title, and 10-Ks printed it as Item 4 before 2010. Both
        // forms' lists title Item 14 "Principal Accountant Fees and
        // Services". Neither moves a 10-K to the 10-KSB's Items.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 3. LEGAL PROCEEDINGS\n{body}\nITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.\n\
             ITEM 5. MARKET FOR REGISTRANT'S COMMON EQUITY\n{body}\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_4", "item_5", "item_14"]);
        assert_eq!(s[1].title, "Mine Safety Disclosures");
        // Nor does a 10-Q's Part II "Item 4. Submission of Matters ..." move
        // it to the 10-QSB's Items when Part I Item 4 is printed as "Item 4T".
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 4T. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 4. Submission of Matters to a Vote of Security Holders\nNone.\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_4", "part_2_item_6"]
        );
        assert_eq!(s[4].title, "Mine Safety Disclosures");
    }
}
