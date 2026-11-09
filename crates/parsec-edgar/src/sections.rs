//! Item sections of 10-K, 10-Q and 8-K filings.
//!
//! Every line that looks like an Item heading becomes a candidate. Tables of
//! contents and running page headers also produce candidates, so the extractor
//! picks one candidate per Item such that the chosen headings appear in the
//! form's Item order and the total score is highest. A candidate scores
//! `ln(1 + gap)`, where `gap` is the distance to the next heading of a
//! different Item: body headings start long sections, table-of-contents
//! entries start a few characters of text.

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
    k("5", "Market for Registrant's Common Equity, Related Stockholder Matters and Issuer Purchases of Equity Securities"),
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
    k("2.03", "Creation of a Direct Financial Obligation or an Obligation under an Off-Balance Sheet Arrangement of a Registrant"),
    k("2.04", "Triggering Events That Accelerate or Increase a Direct Financial Obligation or an Obligation under an Off-Balance Sheet Arrangement"),
    k("2.05", "Costs Associated with Exit or Disposal Activities"),
    k("2.06", "Material Impairments"),
    k("3.01", "Notice of Delisting or Failure to Satisfy a Continued Listing Rule or Standard; Transfer of Listing"),
    k("3.02", "Unregistered Sales of Equity Securities"),
    k("3.03", "Material Modification to Rights of Security Holders"),
    k("4.01", "Changes in Registrant's Certifying Accountant"),
    k("4.02", "Non-Reliance on Previously Issued Financial Statements or a Related Audit Report or Completed Interim Review"),
    k("5.01", "Changes in Control of Registrant"),
    k("5.02", "Departure of Directors or Certain Officers; Election of Directors; Appointment of Certain Officers; Compensatory Arrangements of Certain Officers"),
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

/// `Item 1A. Risk Factors`, `PART II - ITEM 1A RISK FACTORS`, `Item 4(T).`, `Items 1 and 2.`, `Item 9 A.`
static PERIODIC_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]*(?:(?i:part)[ \t]+[IV]{1,3}[ \t]*[,.:\-–—]?[ \t]*)?(?i:items?)[ \t]*(\d{1,2}[A-Ca-c]?)(?:[ \t]*\([Tt]\))?[ \t]*[.:\-–—]?[ \t]*([^\n]*)$",
    )
    .unwrap()
});

/// `Item 2.02 Results of Operations and Financial Condition`
static CURRENT_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*(?i:item)[ \t]*(\d{1,2}\.\d{2})[ \t]*[.:\-–—]?[ \t]*([^\n]*)$").unwrap()
});

/// `SIGNATURES`, `Signature`, `Signature(s)`, `S I G N A T U R E S`
static SIGNATURES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]*(?:S ?I ?G ?N ?A ?T ?U ?R ?E(?: ?S)?|Signatures?|Signature\(s\))[ \t]*[.:]?[ \t]*$",
    )
    .unwrap()
});

/// Sub-item references such as `Item 14(d)` or `Item 15(a)(1)`, which sit inside an Item.
static SUB_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\((?:[a-z]|\d{1,2}|[ivx]{1,4})\)").unwrap());

/// One entry of a page-numbered table of contents: `26<TAB>Five-year summary` or `Introduction<TAB>46`.
static TOC_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(\d{1,3})(?:\t| {2,})\S.*|\S.*?(?:\t| {2,}|\.{3,} ?)(\d{1,3}))$").unwrap()
});

struct Candidate {
    pos: usize,
    label: String,
    heading: String,
    /// Lowercase letters of the title after the Item number, for matching a
    /// table-of-contents entry with its heading in the body.
    title: String,
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

/// `Item 9 A. Controls`: a letter set apart from the number belongs to it.
/// Done here rather than in the heading regex, which stays fast that way.
fn split_letter<'a>(label: &str, tail: &'a str) -> (String, &'a str) {
    let b = tail.as_bytes();
    if label.bytes().all(|d| d.is_ascii_digit()) && b.len() >= 2 && matches!(b[0], b'A'..=b'C') && matches!(b[1], b'.' | b':') {
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
    let cands: Vec<Candidate> = re
        .captures_iter(text)
        .filter(|c| heading_tail_ok(&c[2]))
        .map(|c| {
            let m = c.get(0).expect("group 0 always matches");
            let heading: String = m.as_str().trim().chars().take(300).collect();
            let label = c[1].to_ascii_uppercase();
            let (label, tail) = if kind == FormKind::EightK { (label, &c[2]) } else { split_letter(&label, &c[2]) };
            Candidate { pos: m.start(), label, heading, title: title_key(tail), score: 0.0 }
        })
        .collect();
    let mut cands = drop_running_headers(drop_contents_entries(cands));
    // Gap to the next heading of a different Item, so repeated running headers
    // of one Item do not split its score.
    for i in 0..cands.len() {
        let next = cands[i + 1..].iter().find(|c| c.label != cands[i].label).map_or(text.len(), |c| c.pos);
        cands[i].score = ((next - cands[i].pos) as f64).ln_1p();
    }
    cands
}

/// Drop table-of-contents entries whose heading reappears in the body.
///
/// An entry is a candidate whose Item and title come back later, after other
/// Items, at a heading that stands further from its neighbors. In a 10-Q this
/// matters most: a contents entry for Part I Item 2 could otherwise be taken
/// as Part I and the body heading as Part II. Entries just before a dropped
/// one belong to the same contents block and go too (an Item listed in the
/// contents whose body has no heading).
fn drop_contents_entries(cands: Vec<Candidate>) -> Vec<Candidate> {
    const SAME_BLOCK: usize = 2000;
    let n = cands.len();
    let nearest = |i: usize| {
        let prev = if i > 0 { cands[i].pos - cands[i - 1].pos } else { usize::MAX };
        let next = cands.get(i + 1).map_or(usize::MAX, |c| c.pos - cands[i].pos);
        prev.min(next)
    };
    let mut drop = vec![false; n];
    for a in 0..n {
        if cands[a].title.chars().count() < 5 {
            continue;
        }
        let mut other_between = false;
        for b in a + 1..n {
            if cands[b].label != cands[a].label {
                other_between = true;
            } else if other_between
                && (cands[a].title.starts_with(&cands[b].title) || cands[b].title.starts_with(&cands[a].title))
                && cands[b].title.chars().count() >= 5
                && nearest(b) > nearest(a)
            {
                drop[a] = true;
                break;
            }
        }
    }
    for i in (0..n.saturating_sub(1)).rev() {
        if drop[i + 1] && cands[i + 1].pos - cands[i].pos <= SAME_BLOCK {
            drop[i] = true;
        }
    }
    cands.into_iter().zip(drop).filter(|(_, d)| !d).map(|(c, _)| c).collect()
}

/// Drop a bare running page header ("Item 7") that comes just before the
/// titled heading of the same Item ("ITEM 7. MANAGEMENT'S DISCUSSION ...").
fn drop_running_headers(cands: Vec<Candidate>) -> Vec<Candidate> {
    const NEAR: usize = 5000;
    let keep: Vec<bool> = (0..cands.len())
        .map(|i| {
            let Some(next) = cands.get(i + 1) else { return true };
            !(cands[i].title.is_empty()
                && !next.title.is_empty()
                && next.label == cands[i].label
                && next.pos - cands[i].pos <= NEAR)
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

/// Split filing text (from [`crate::html_to_text`]) into Item sections, in document order.
pub fn extract_sections(text: &str, kind: FormKind) -> Vec<Section> {
    let keys = match kind {
        FormKind::TenK => TEN_K,
        FormKind::TenQ => TEN_Q,
        FormKind::EightK => EIGHT_K,
    };
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
        for (ki, _) in keys.iter().enumerate().filter(|(_, key)| key.item == cand.label) {
            let best = states
                .iter()
                .enumerate()
                .filter(|(_, s)| s.cand < ci && s.key < ki)
                .max_by(|(_, a), (_, b)| a.total.total_cmp(&b.total));
            states.push(State {
                cand: ci,
                key: ki,
                total: cand.score + best.map_or(0.0, |(_, s)| s.total),
                prev: best.map(|(i, _)| i),
            });
        }
    }
    let Some(mut cur) = states.iter().enumerate().max_by(|(_, a), (_, b)| a.total.total_cmp(&b.total)).map(|(i, _)| i)
    else {
        return Vec::new();
    };
    let mut chain = vec![cur];
    while let Some(p) = states[cur].prev {
        chain.push(p);
        cur = p;
    }
    chain.reverse();

    let starts: Vec<usize> = chain.iter().map(|&s| cands[states[s].cand].pos).collect();
    let last_start = *starts.last().expect("chain is non-empty");
    let doc_end = SIGNATURES.find_at(text, last_start).map_or(text.len(), |m| m.start());
    let doc_end = attached_document(text, last_start, doc_end).unwrap_or(doc_end);

    chain
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let key = &keys[states[s].key];
            let cand = &cands[states[s].cand];
            let start = cand.pos;
            let end = starts.get(i + 1).copied().unwrap_or(doc_end).max(start);
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
        let text = format!(
            "Item 1. Business\n{body}\nItem 1. Business (continued)\n{body}\nItem 1A. Risk Factors\n{body}\n"
        );
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
}
