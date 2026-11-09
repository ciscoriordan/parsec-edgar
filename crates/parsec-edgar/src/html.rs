//! Filing documents to clean plain text.
//!
//! One line per block (paragraph, heading, list item, table row). Table cells
//! are joined with tabs, with `$` and `)`/`%` cells merged into their numbers.
//! Hidden inline-XBRL headers, scripts, styles, page numbers, running page
//! headers and footers, and repeated "Table of Contents" links are dropped.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use ego_tree::NodeRef;
use regex::Regex;
use scraper::{Html, Node, node::Element};

/// Longest table row (in characters) still treated as a data row. Longer rows
/// are layout tables (some filings wrap whole pages in a table) and are
/// rendered as ordinary blocks.
const MAX_ROW_CHARS: usize = 1000;

/// Convert an HTML document to clean plain text.
pub fn html_to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let mut out = Out::default();
    render(doc.tree.root(), &mut out);
    finish(&out.buf)
}

/// Convert any EDGAR document (HTML, plain text, or a full SGML submission) to text.
pub fn document_to_text(raw: &str) -> String {
    let body = first_document_body(raw).unwrap_or(raw);
    if looks_like_html(body) { html_to_text(body) } else { plain_text(body) }
}

/// For full submissions (`<SEC-DOCUMENT>`), the `<TEXT>` of the first `<DOCUMENT>`.
fn first_document_body(raw: &str) -> Option<&str> {
    let doc = raw.find("<DOCUMENT>")?;
    let rest = &raw[doc..];
    let start = rest.find("<TEXT>")? + "<TEXT>".len();
    let end = rest[start..].find("</TEXT>").map_or(rest.len(), |e| start + e);
    Some(&rest[start..end])
}

/// Old plain-text filings use SGML `<TABLE>`, `<S>` and `<C>` tags, so a bare
/// `<table` does not mean HTML: only tags that plain-text filings never use count.
fn looks_like_html(s: &str) -> bool {
    let mut end = s.len().min(64 * 1024);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let head = s[..end].to_ascii_lowercase();
    ["<html", "<body", "<div", "<p>", "<p ", "<font", "<br", "<tr>", "<tr ", "<td", "<span"]
        .iter()
        .any(|t| head.contains(t))
}

/// Old plain-text filings: drop SGML layout tags, then the same line cleanup as HTML.
fn plain_text(s: &str) -> String {
    static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</?(?:PAGE|S|C|CAPTION|TABLE|FN)>").unwrap());
    finish(&TAGS.replace_all(s, ""))
}

#[derive(Default)]
struct Out {
    buf: String,
    pending_space: bool,
    /// True at the start of the buffer and after each newline.
    mid_line: bool,
}

impl Out {
    fn text(&mut self, s: &str) {
        for c in s.chars() {
            match c {
                '\u{200b}' | '\u{ad}' | '\u{feff}' => {}
                c if c.is_whitespace() => self.pending_space = self.mid_line,
                c => {
                    if self.pending_space {
                        self.buf.push(' ');
                        self.pending_space = false;
                    }
                    self.buf.push(c);
                    self.mid_line = true;
                }
            }
        }
    }

    fn newline(&mut self) {
        self.pending_space = false;
        if self.mid_line {
            self.buf.push('\n');
            self.mid_line = false;
        }
    }

    /// Append pre-rendered text that already has its own line structure.
    fn raw(&mut self, s: &str) {
        self.newline();
        self.buf.push_str(s);
        self.mid_line = !s.is_empty() && !s.ends_with('\n');
    }
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "ul"
            | "ol"
            | "li"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "blockquote"
            | "pre"
            | "hr"
            | "center"
            | "dl"
            | "dt"
            | "dd"
            | "address"
            | "body"
            | "html"
            | "caption"
            | "form"
            | "fieldset"
            | "figure"
            | "figcaption"
            | "main"
            | "nav"
            | "aside"
            | "page"
            | "td"
            | "th"
            | "tr"
    )
}

fn is_skipped(el: &Element) -> bool {
    let name = el.name();
    if matches!(name, "script" | "style" | "head" | "title" | "noscript" | "template") || name.ends_with(":header") {
        return true;
    }
    if el.attr("hidden").is_some() {
        return true;
    }
    el.attr("style").is_some_and(|s| {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
        s.contains("display:none")
    })
}

/// Depth-first render with an explicit stack: some filings nest tags thousands
/// deep, which would overflow the call stack with naive recursion.
fn render(root: NodeRef<Node>, out: &mut Out) {
    enum Step<'a> {
        Enter(NodeRef<'a, Node>),
        Exit,
    }
    let mut stack = vec![Step::Enter(root)];
    while let Some(step) = stack.pop() {
        let node = match step {
            Step::Exit => {
                out.newline();
                continue;
            }
            Step::Enter(node) => node,
        };
        match node.value() {
            Node::Text(t) => out.text(t),
            Node::Element(el) => {
                if is_skipped(el) {
                    continue;
                }
                match el.name() {
                    "br" => out.newline(),
                    "tr" => render_row(node, out),
                    name => {
                        if is_block(name) {
                            out.newline();
                            stack.push(Step::Exit);
                        }
                        stack.extend(node.children().rev().map(Step::Enter));
                    }
                }
            }
            Node::Document | Node::Fragment => stack.extend(node.children().rev().map(Step::Enter)),
            _ => {}
        }
    }
}

fn render_row(tr: NodeRef<Node>, out: &mut Out) {
    let mut cells = Vec::new();
    for child in tr.children() {
        let Node::Element(el) = child.value() else { continue };
        if is_skipped(el) {
            continue;
        }
        let mut cell = Out::default();
        render(child, &mut cell);
        let text = cell.buf.trim().to_owned();
        if !text.is_empty() {
            cells.push(text);
        }
    }
    if cells.is_empty() {
        return;
    }
    let data_row = cells.iter().all(|c| !c.contains('\n'))
        && cells.iter().map(|c| c.chars().count()).sum::<usize>() <= MAX_ROW_CHARS;
    if data_row {
        out.raw(&merge_cells(cells).join("\t"));
    } else {
        for c in cells {
            out.raw(&c);
        }
    }
    out.newline();
}

/// Financial tables put `$` and closing `)` or `%` in cells of their own.
fn merge_cells(cells: Vec<String>) -> Vec<String> {
    let mut merged: Vec<String> = Vec::with_capacity(cells.len());
    let mut prefix = String::new();
    for c in cells {
        if matches!(c.as_str(), "$" | "€" | "£" | "¥" | "(" | "$(" | "($") {
            prefix.push_str(&c);
        } else if matches!(c.as_str(), ")" | "%" | ")%" | "%)") && !merged.is_empty() && prefix.is_empty() {
            merged.last_mut().expect("checked non-empty").push_str(&c);
        } else {
            merged.push(std::mem::take(&mut prefix) + &c);
        }
    }
    if !prefix.is_empty() {
        merged.push(prefix);
    }
    merged
}

/// Trim lines and drop empty lines, bare page numbers, "Table of Contents"
/// links and running page headers and footers.
fn finish(text: &str) -> String {
    static PAGE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:(?i:page)\s+)?(?:\d{1,3}|[ivxlc]{1,7}|F-\d{1,3}|[-–—]\s*\d{1,3}\s*[-–—])$").unwrap()
    });
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !PAGE_NUMBER.is_match(l) && !l.eq_ignore_ascii_case("table of contents"))
        .collect();
    let furniture = page_furniture(&lines);
    let mut out = String::with_capacity(text.len());
    for (i, line) in lines.iter().enumerate() {
        if !furniture.contains(&i) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Running page headers and footers: short lines that repeat with only a page
/// number changing, such as "Apple Inc. | 2026 Form 10-K | 23",
/// "26  JPMorgan Chase & Co./2007 Annual Report" or "Page 3 of 25 (Form 10-K)".
/// The page numbers must mostly increase through the document, which data rows
/// do not do. The text beside the number needs 10 letters, because short
/// lines such as cross-references ("See page 4") also come with rising
/// numbers. Shorter text is enough when it names the form ("2021 Form 10-K
/// 100") or the line starts with "Page N of M" ("Page 3 of 26"), and the
/// copies are mostly not on consecutive lines, as the rows of a table are.
fn page_furniture(lines: &[&str]) -> HashSet<usize> {
    const MIN_REPEATS: usize = 5;
    const MIN_LETTERS: usize = 10;
    // The page number comes first, in a leading "Page N of M", or last.
    static NUMBERED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(concat!(
            r"^(?:(\d{1,3})(?:\s*\|\s*|\s+)(\S.*)",
            r"|(?i:page)\s+(\d{1,3})\s+((?i:of)\s+\d{1,3}\b.*)",
            r"|(.*\S)(?:\s*\|\s*|\s+)(\d{1,3}))$",
        ))
        .unwrap()
    });
    static FORM_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:10-K|10-Q|8-K|20-F|40-F)").unwrap());
    // Keyed by the text beside the page number and whether the line starts with "Page N of M".
    let mut families: HashMap<(&str, bool), Vec<(usize, u32)>> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        let numbered = line.starts_with(|c: char| c.is_ascii_digit())
            || line.ends_with(|c: char| c.is_ascii_digit())
            || line.get(..4).is_some_and(|s| s.eq_ignore_ascii_case("page"));
        if line.len() > 120 || !numbered {
            continue;
        }
        let Some(c) = NUMBERED.captures(line) else { continue };
        let num = c.get(1).or_else(|| c.get(3)).or_else(|| c.get(6));
        let rest = c.get(2).or_else(|| c.get(4)).or_else(|| c.get(5));
        let (Some(num), Some(rest)) = (num, rest) else { continue };
        let rest = rest.as_str().trim_matches(|c: char| c == '|' || c.is_whitespace());
        let page_first = c.get(3).is_some();
        let short = rest.chars().filter(|c| c.is_alphabetic()).count() < MIN_LETTERS;
        if !rest.contains(' ') || (short && !page_first && !FORM_NAME.is_match(rest)) {
            continue;
        }
        if let Ok(n) = num.as_str().parse() {
            families.entry((rest, page_first)).or_default().push((i, n));
        }
    }
    let mut drop = HashSet::new();
    for (&(rest, _), seen) in &families {
        if seen.len() < MIN_REPEATS {
            continue;
        }
        let mostly = |count: usize| count * 10 >= (seen.len() - 1) * 9;
        let rising = seen.windows(2).filter(|w| w[1].1 > w[0].1).count();
        let apart = seen.windows(2).filter(|w| w[1].0 > w[0].0 + 1).count();
        let short = rest.chars().filter(|c| c.is_alphabetic()).count() < MIN_LETTERS;
        if mostly(rising) && (!short || mostly(apart)) {
            drop.extend(seen.iter().map(|&(i, _)| i));
        }
    }
    drop
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_and_inline() {
        let html = r#"<html><head><title>x</title><style>p{}</style></head><body>
            <div><span>ITEM&#160;1A.</span><span>RISK FACTORS</span></div>
            <p>Our   business is <b>risky</b>.<br>Second line.</p>
            <script>var x = 1;</script></body></html>"#;
        assert_eq!(html_to_text(html), "ITEM 1A.RISK FACTORS\nOur business is risky.\nSecond line.\n");
    }

    #[test]
    fn hidden_xbrl_header_dropped() {
        let html = r#"<body><div style="display: none"><ix:header><ix:hidden>dei:Secret</ix:hidden></ix:header></div>
            <p>Visible</p></body>"#;
        assert_eq!(html_to_text(html), "Visible\n");
    }

    #[test]
    fn financial_table_rows() {
        let html = r#"<table>
            <tr><td></td><td>2024</td><td>2023</td></tr>
            <tr><td>Net sales</td><td>$</td><td>391,035</td><td>$</td><td>383,285</td></tr>
            <tr><td>Net loss</td><td>(1,234</td><td>)</td><td>12.5</td><td>%</td></tr>
        </table>"#;
        assert_eq!(html_to_text(html), "2024\t2023\nNet sales\t$391,035\t$383,285\nNet loss\t(1,234)\t12.5%\n");
    }

    #[test]
    fn layout_table_rendered_as_blocks() {
        let long = "word ".repeat(300);
        let html = format!("<table><tr><td><p>ITEM 1. BUSINESS</p><p>{long}</p></td></tr></table>");
        let text = html_to_text(&html);
        assert!(text.starts_with("ITEM 1. BUSINESS\nword word"));
    }

    #[test]
    fn page_numbers_and_toc_links_dropped() {
        let html = "<p>Text</p><p>12</p><p>Table of Contents</p><p>F-3</p><p>- 7 -</p><p>ii</p><p>More</p>";
        assert_eq!(html_to_text(html), "Text\nMore\n");
    }

    #[test]
    fn running_page_footers_dropped() {
        let mut html = String::new();
        for page in 20..26 {
            html += &format!("<p>Text of page {page}.</p><p>Apple Inc. | 2026 Form 10-K | {page}</p>");
        }
        // Repeated rows whose numbers do not count up like pages stay.
        for _ in 0..6 {
            html += "<p>Total stores at year end 12</p>";
        }
        let text = html_to_text(&html);
        assert!(!text.contains("Form 10-K"));
        assert!(text.starts_with("Text of page 20.\nText of page 21.\n"));
        assert_eq!(text.matches("Total stores at year end 12").count(), 6);
    }

    #[test]
    fn short_page_headers_naming_the_form_dropped() {
        // Fewer than 10 letters beside the page number, but the line names the
        // form or starts with "Page N of M".
        for header in
            ["2021 FORM 10-K {}", "Form 10-Q Page {}", "{} | Form 10-K", "Page {} of 125 (Form 10-K)", "PAGE {} OF 125"]
        {
            let mut html = String::new();
            let mut want = String::new();
            for page in 98..104 {
                html += &format!("<p>Text of page {page}.</p><p>{}</p>", header.replace("{}", &page.to_string()));
                want += &format!("Text of page {page}.\n");
            }
            assert_eq!(html_to_text(&html), want, "{header}");
        }
    }

    #[test]
    fn page_furniture_keeps_lines_that_only_look_like_it() {
        let mut html = String::new();
        // The heading itself has no page number and stays, while the running
        // headers made from it go.
        html += "<p>Item 7. Management's Discussion and Analysis</p>";
        for page in 30..36 {
            html += &format!("<p>Text of page {page}.</p><p>Item 7. Management's Discussion and Analysis {page}</p>");
        }
        // Too few repeats.
        for page in 40..44 {
            html += &format!("<p>Annual Report on Form 10-K {page}</p><p>Text of page {page}.</p>");
            html += &format!("<p>Form 10-Q Page {page}</p><p>Text of page {page}.</p>");
        }
        // Numbers that do not rise like pages.
        for n in [12, 9, 14, 3, 20, 1] {
            html += &format!("<p>Number of employees at year end {n}</p>");
            html += &format!("<p>2021 Form 10-K {n}</p><p>Text that mentions the report.</p>");
        }
        // Short lines with rising numbers that name no form and do not start
        // with "Page N of M": note headings, cross-references and exhibit rows.
        // A sentence that starts with "Page N" is not a page footer either.
        for n in 1..9 {
            html += &format!(
                "<p>Note {n}</p><p>Text of note {n}.</p><p>See page {n}</p><p>For footnotes see page {n}.</p>\
                 <p>Page {n} of the Annual Report is incorporated herein by reference.</p>"
            );
        }
        html += "<table>";
        for n in [3, 10, 21, 23, 31, 32] {
            html += &format!("<tr><td>EXHIBIT {n}</td></tr>");
        }
        // Rows that name the form on consecutive lines are a table, not one line per page.
        for n in 50..56 {
            html += &format!("<tr><td>Form 10-Q</td><td>{n}</td></tr>");
        }
        html += "</table>";
        let text = html_to_text(&html);
        assert!(text.starts_with("Item 7. Management's Discussion and Analysis\nText of page 30.\n"));
        assert_eq!(text.matches("Management's Discussion").count(), 1);
        for (start, copies) in [
            ("Annual Report on Form 10-K ", 4),
            ("Form 10-Q Page ", 4),
            ("Number of employees", 6),
            ("2021 Form 10-K ", 6),
            ("Note ", 8),
            ("See page ", 8),
            ("For footnotes see page ", 8),
            ("Page ", 8),
            ("EXHIBIT ", 6),
            ("Form 10-Q\t", 6),
        ] {
            assert_eq!(text.lines().filter(|l| l.starts_with(start)).count(), copies, "{start}");
        }
    }

    #[test]
    fn deep_nesting_does_not_overflow() {
        let html = format!("{}deep{}", "<font>".repeat(100_000), "</font>".repeat(100_000));
        assert_eq!(html_to_text(&html), "deep\n");
    }

    #[test]
    fn full_submission_plain_text() {
        let raw = "<SEC-DOCUMENT>\n<DOCUMENT>\n<TYPE>10-K\n<TEXT>\n<PAGE>\nITEM 1. BUSINESS\n\n  We make things.\n</TEXT>\n</DOCUMENT>";
        assert_eq!(document_to_text(raw), "ITEM 1. BUSINESS\nWe make things.\n");
    }

    #[test]
    fn sgml_tables_stay_plain_text() {
        let raw = "<DOCUMENT>\n<TEXT>\n<PAGE>\nITEM 6. SELECTED FINANCIAL DATA\n<TABLE>\n<CAPTION>\n<S>      <C>\n\
                   Net sales   $ 6,134\n</TABLE>\nITEM 7. MANAGEMENT'S DISCUSSION\n</TEXT>";
        assert_eq!(
            document_to_text(raw),
            "ITEM 6. SELECTED FINANCIAL DATA\nNet sales   $ 6,134\nITEM 7. MANAGEMENT'S DISCUSSION\n"
        );
    }
}
