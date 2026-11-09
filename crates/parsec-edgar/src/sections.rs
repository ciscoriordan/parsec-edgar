//! Item sections of 10-K, 10-Q and 8-K filings.
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
//! titled heading. In a 10-Q a `PART I` or `PART II` line just above a heading
//! fixes its part, and without one a title that only one part uses for that
//! Item number does. The last Item ends at the signatures or where a
//! separately paginated attached document (an annual report) begins.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Form families with Item sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormKind {
    TenK,
    TenQ,
    EightK,
}

impl FormKind {
    /// Classify a form type. Amendments and old variants (10-K405, 10-KSB, 10-QSB, 8-K12B) count.
    pub fn from_form(form: &str) -> Option<Self> {
        let f = form.trim().to_ascii_uppercase();
        let base = f.strip_suffix("/A").unwrap_or(&f);
        if base.starts_with("10-K") {
            Some(Self::TenK)
        } else if base.starts_with("10-Q") {
            Some(Self::TenQ)
        } else if base.starts_with("8-K") {
            Some(Self::EightK)
        } else {
            None
        }
    }
}

/// One Item section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Stable id: `item_1a` (10-K), `part_2_item_1a` (10-Q), `item_2.02` (8-K).
    pub id: String,
    /// `I` or `II` for 10-Q sections.
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

fn keys_for(kind: FormKind) -> &'static [Key] {
    match kind {
        FormKind::TenK => TEN_K,
        FormKind::TenQ => TEN_Q,
        FormKind::EightK => EIGHT_K,
    }
}

/// `Item 1A. Risk Factors`, `PART II - ITEM 1A RISK FACTORS`, `Item 4(T).`, `Items 1 and 2.`, `Item 9 A.`
static PERIODIC_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]*(?:(?i:part)[ \t]+[IV]{1,3}[ \t]*[,.:\-–—]?[ \t]*)?(?i:items?)[ \t]*(\d{1,2}[A-Ca-c]?)(?:[ \t]*\([Tt]\))?[ \t]*[.:\-–—]?[ \t]*([^\n]*)$",
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
    /// Distance to the next heading of a different Item.
    gap: usize,
    score: f64,
}

/// A real heading is followed by a capitalized title, nothing, or a joined Item
/// ("Items 1 and 2."). Cross-references ("Item 7 of this report") and
/// sub-items ("Item 14(d)") are not.
fn heading_tail_ok(tail: &str) -> bool {
    let t = tail.trim_start();
    match t.chars().next() {
        None => true,
        Some(c) => {
            (c.is_uppercase()
                || matches!(c, '"' | '“' | '\'' | '‘' | '(' | '[')
                || t.starts_with("and ")
                || t.starts_with("& ")
                || t.starts_with("through "))
                && !SUB_ITEM.is_match(t)
        }
    }
}

fn title_key(tail: &str) -> String {
    tail.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).take(30).collect()
}

/// `Item 9 A. Controls` and `Item 1.A. Risk Factors`: a letter set apart from
/// the number by spaces only, or by a separator with no space after it,
/// belongs to it (in `ITEM 1. A. GENERAL` it starts the title). Done here
/// rather than in the heading regex, which stays fast that way.
fn split_letter<'a>(label: &str, between: &str, tail: &'a str) -> (String, &'a str) {
    let b = tail.as_bytes();
    if label.bytes().all(|d| d.is_ascii_digit())
        && (between.trim().is_empty() || !between.ends_with(char::is_whitespace))
        && b.len() >= 2
        && matches!(b[0], b'A'..=b'C')
        && matches!(b[1], b'.' | b':')
    {
        (format!("{label}{}", b[0] as char), tail[2..].trim_start())
    } else {
        (label.to_owned(), tail)
    }
}

fn candidates(text: &str, kind: FormKind) -> Vec<Candidate> {
    let re = match kind {
        FormKind::EightK => &*CURRENT_HEADING,
        FormKind::TenK | FormKind::TenQ => &*PERIODIC_HEADING,
    };
    let mut cands: Vec<Candidate> = re
        .captures_iter(text)
        .filter(|c| heading_tail_ok(&c[2]))
        .map(|c| {
            let m = c.get(0).expect("group 0 always matches");
            let heading: String = m.as_str().trim().chars().take(300).collect();
            let (number, tail) = (c.get(1).expect("Item number"), c.get(2).expect("tail"));
            let label = number.as_str().to_ascii_uppercase();
            let (label, tail) = if kind == FormKind::EightK {
                (label, tail.as_str())
            } else {
                split_letter(&label, &text[number.end()..tail.start()], tail.as_str())
            };
            let bare = !tail.chars().any(char::is_alphabetic);
            let title = if bare { title_key(next_line_title(text, m.end())) } else { title_key(tail) };
            let (part, title_part) = if kind == FormKind::TenQ {
                (part_above(text, m.start(), &label), part_by_title(&label, &title))
            } else {
                (None, None)
            };
            Candidate { pos: m.start(), label, heading, bare, title, part, title_part, gap: 0, score: 0.0 }
        })
        .collect();
    set_gaps(&mut cands, text.len());
    let mut cands = drop_running_headers(drop_contents_entries(cands, text), keys_for(kind));
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

/// The 10-Q part whose standard title for Item `label` matches `title`,
/// when exactly one part's does.
fn part_by_title(label: &str, title: &str) -> Option<&'static str> {
    let mut parts = TEN_Q.iter().filter(|k| k.item == label && same_title(title, &title_key(k.title))).map(|k| k.part);
    match (parts.next(), parts.next()) {
        (Some(part), None) => part,
        _ => None,
    }
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
/// contents whose body has no heading), unless they are body headings
/// themselves; short entries just after a dropped listing entry go with it.
fn drop_contents_entries(cands: Vec<Candidate>, text: &str) -> Vec<Candidate> {
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
    for i in (0..n.saturating_sub(1)).rev() {
        if contents[i + 1] && !paired[i] && cands[i + 1].pos - cands[i].pos <= SAME_BLOCK {
            contents[i] = true;
        }
    }
    let mut after = vec![false; n];
    if !later_copies.is_empty() {
        let body_start = (0..n).find(|&i| body[i] && !contents[i]).map_or(0, |i| cands[i].pos);
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
    cands.into_iter().enumerate().filter(|&(i, _)| !contents[i] && !after[i]).map(|(_, c)| c).collect()
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

/// Some 10-Qs print Items out of form order: banks often put the financial
/// statements (Part I Item 1) after MD&A. The ordered chain cannot hold such a
/// heading, so it is added afterwards when a `PART` line marks it, its Item is
/// still unused, it starts a real stretch of text rather than a contents
/// entry, and it comes before the signatures.
fn add_out_of_order(text: &str, cands: &[Candidate], keys: &[Key], picks: &mut Vec<(usize, usize)>) {
    const MIN_GAP: usize = 1000;
    let first = cands[picks[0].0].pos;
    let last = picks.iter().map(|&(c, _)| cands[c].pos).max().unwrap_or(first);
    let limit = signatures_after(text, last).unwrap_or(text.len());
    let mut extra: Vec<(usize, usize)> = Vec::new();
    for (ci, c) in cands.iter().enumerate() {
        if c.part.is_none()
            || c.gap < MIN_GAP
            || c.pos <= first
            || c.pos >= limit
            || picks.iter().any(|&(p, _)| p == ci)
        {
            continue;
        }
        let Some(ki) = keys.iter().position(|k| k.item == c.label && k.part == c.part) else { continue };
        if picks.iter().chain(&extra).any(|&(_, k)| k == ki) {
            continue;
        }
        extra.push((ci, ki));
    }
    picks.extend(extra);
    picks.sort_by_key(|&(c, _)| cands[c].pos);
}

/// Order chain states by total score. On a tie the earlier Item wins, so an
/// unmarked 10-Q "Item 2" between Part I Item 1 and Part II Item 6 stays in Part I.
fn better(a_total: f64, a_key: usize, b_total: f64, b_key: usize) -> std::cmp::Ordering {
    a_total.total_cmp(&b_total).then(b_key.cmp(&a_key))
}

/// Split filing text (from [`crate::html_to_text`]) into Item sections, in document order.
pub fn extract_sections(text: &str, kind: FormKind) -> Vec<Section> {
    let keys = keys_for(kind);
    let cands = candidates(text, kind);

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
    if kind == FormKind::TenQ {
        add_out_of_order(text, &cands, keys, &mut picks);
    }

    let starts: Vec<usize> = picks.iter().map(|&(c, _)| cands[c].pos).collect();
    let last_start = *starts.last().expect("chain is non-empty");
    let doc_end = signatures_after(text, last_start).unwrap_or(text.len());
    let doc_end = attached_document(text, last_start, doc_end).unwrap_or(doc_end);
    // Titled headings of Items the form does not define ("Item 4A. Executive
    // Officers") end the section before them.
    let stops: Vec<usize> = if kind == FormKind::TenK {
        cands.iter().filter(|c| !c.bare && !keys.iter().any(|k| k.item == c.label)).map(|c| c.pos).collect()
    } else {
        Vec::new()
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
}
