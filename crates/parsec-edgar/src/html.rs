//! Filing documents to clean plain text.
//!
//! One line per block (paragraph, heading, list item, table row). Table cells
//! are joined with tabs, with `$` and `)`/`%` cells merged into their numbers.
//! Hidden inline-XBRL headers, scripts, styles, page numbers and repeated
//! "Table of Contents" links are dropped.

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

fn looks_like_html(s: &str) -> bool {
    let head: String = s.chars().take(8192).collect::<String>().to_ascii_lowercase();
    ["<html", "<body", "<div", "<p>", "<p ", "<table", "<font", "<br"].iter().any(|t| head.contains(t))
}

/// Old plain-text filings: drop SGML layout tags, then the same line cleanup as HTML.
fn plain_text(s: &str) -> String {
    static TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)</?(?:PAGE|S|C|CAPTION|TABLE|FN)>").unwrap());
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
        "p" | "div" | "table" | "thead" | "tbody" | "tfoot" | "ul" | "ol" | "li" | "h1" | "h2" | "h3" | "h4"
            | "h5" | "h6" | "section" | "article" | "header" | "footer" | "blockquote" | "pre" | "hr" | "center"
            | "dl" | "dt" | "dd" | "address" | "body" | "html" | "caption" | "form" | "fieldset" | "figure"
            | "figcaption" | "main" | "nav" | "aside" | "page" | "td" | "th" | "tr"
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

/// Trim lines and drop empty lines, bare page numbers and "Table of Contents" links.
fn finish(text: &str) -> String {
    static PAGE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:(?i:page)\s+)?(?:\d{1,3}|[ivxlc]{1,7}|F-\d{1,3}|[-–—]\s*\d{1,3}\s*[-–—])$").unwrap()
    });
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || PAGE_NUMBER.is_match(line) || line.eq_ignore_ascii_case("table of contents") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
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
    fn deep_nesting_does_not_overflow() {
        let html = format!("{}deep{}", "<font>".repeat(100_000), "</font>".repeat(100_000));
        assert_eq!(html_to_text(&html), "deep\n");
    }

    #[test]
    fn full_submission_plain_text() {
        let raw = "<SEC-DOCUMENT>\n<DOCUMENT>\n<TYPE>10-K\n<TEXT>\n<PAGE>\nITEM 1. BUSINESS\n\n  We make things.\n</TEXT>\n</DOCUMENT>";
        assert_eq!(document_to_text(raw), "ITEM 1. BUSINESS\nWe make things.\n");
    }
}
