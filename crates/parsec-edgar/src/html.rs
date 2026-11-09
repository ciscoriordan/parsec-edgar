//! Filing documents to clean plain text.
//!
//! One line per block (paragraph, heading, list item, table row). Table cells
//! are joined with tabs, with `$` and `)`/`%` cells merged into their numbers.
//! Hidden inline-XBRL headers, scripts, styles, page numbers, running page
//! headers and footers, and repeated "Table of Contents" links are dropped.
//!
//! [`html_to_text_and_tables`] also returns the tables as rows of cells, with
//! their byte offsets in the same text.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::LazyLock;

use ego_tree::{NodeId, NodeRef};
use regex::Regex;
use scraper::{Html, Node, node::Element};
use serde::{Deserialize, Serialize};

/// Longest table row (in characters) still treated as a data row. Longer rows
/// are layout tables (some filings wrap whole pages in a table) and are
/// rendered as ordinary blocks.
const MAX_ROW_CHARS: usize = 1000;

/// Tables nested deeper than this inside the cells of other tables are
/// printed as plain blocks, one line per cell, and not returned as tables.
/// Rendering a row recurses into its cells, and malformed or hostile input
/// can nest tables thousands deep, which would overflow the call stack.
const MAX_TABLE_DEPTH: usize = 50;

/// A table of a filing as rows of cells, with its place in the filing's clean text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// Byte offsets of the table in the text returned with it, from the start
    /// of its first row to just past the newline that ends its last row. A
    /// [`crate::Section`] holds the table when its `start..end` holds `start`.
    pub start: usize,
    pub end: usize,
    /// Indexes into `rows` of the header rows: rows inside `<thead>`, and rows
    /// whose cells are all `<th>` cells. Most filings mark no header rows.
    pub header_rows: Vec<usize>,
    /// Rows in document order, each a list of cells that read exactly as in
    /// the text: `$` and `)` or `%` cells merged into their numbers, empty
    /// cells left out. A row is a line of tab-separated cells in the text,
    /// unless a cell has several lines, such as a header "Year Ended" over
    /// "December 31,", or the row is over 1,000 characters: then each cell
    /// starts a line of the text and keeps its line breaks. The lines of the
    /// cells of a row with two or more cells are never removed as page numbers
    /// or running headers (in a row over 1,000 characters, only cells of one
    /// line).
    pub rows: Vec<Vec<String>>,
}

/// Convert an HTML document to clean plain text.
pub fn html_to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let mut out = Out::default();
    render(doc.tree.root(), &mut out);
    join_lines(&kept_lines(&out.buf, &out.cell_lines))
}

/// Convert an HTML document to clean plain text, exactly as [`html_to_text`]
/// does, and return its tables with their places in that text.
///
/// A table is a `<table>` element with at least one row printed as a line of
/// cells. Layout tables, whose rows are all printed as blocks of text (some
/// filings wrap whole pages in a table), are not returned, while tables
/// inside them are. A table inside a cell of another table is returned on its
/// own, within the outer table's offsets, unless it has a single row that
/// became one cell of a line of the outer table. Tables nested more than 50
/// deep are printed as plain blocks and not returned. Tables are ordered by
/// `start`.
pub fn html_to_text_and_tables(html: &str) -> (String, Vec<Table>) {
    let doc = Html::parse_document(html);
    let mut out = Out { rows: Some(Vec::new()), ..Out::default() };
    render(doc.tree.root(), &mut out);
    let lines = kept_lines(&out.buf, &out.cell_lines);
    let text = join_lines(&lines);
    let tables = locate_tables(&out.buf, &lines, &text, out.rows.unwrap_or_default());
    (text, tables)
}

/// Convert any EDGAR document (HTML, plain text, or a full SGML submission) to text.
pub fn document_to_text(raw: &str) -> String {
    let body = first_document_body(raw).unwrap_or(raw);
    if looks_like_html(body) { html_to_text(body) } else { plain_text(body) }
}

/// Convert any EDGAR document to text, exactly as [`document_to_text`] does,
/// and return its tables as [`html_to_text_and_tables`] does. Plain-text
/// documents have no tables.
pub fn document_to_text_and_tables(raw: &str) -> (String, Vec<Table>) {
    let body = first_document_body(raw).unwrap_or(raw);
    if looks_like_html(body) { html_to_text_and_tables(body) } else { (plain_text(body), Vec::new()) }
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
    /// How many table cells the text is rendered inside.
    depth: usize,
    /// A table row with text was printed into `buf`.
    has_table: bool,
    /// Start in `buf` of each line of the cells of rows printed cell by cell
    /// with two or more cells (in rows over 1,000 characters, only of cells of
    /// one line), in order. These lines are table data, never page numbers or
    /// page furniture.
    cell_lines: Vec<usize>,
    /// Table rows rendered into `buf`, recorded only when tables are wanted.
    rows: Option<Vec<RowRecord>>,
}

/// Where one table row was rendered in `Out::buf`.
struct RowRecord {
    /// The `<table>` element the row belongs to.
    table: NodeId,
    /// Inside `<thead>`, or every cell with text is a `<th>`.
    header: bool,
    /// Printed as one line of cells, not as blocks.
    line: bool,
    /// Byte range of each cell in `Out::buf`.
    cells: Vec<Range<usize>>,
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
                    "tr" if out.depth < MAX_TABLE_DEPTH => render_row(node, out),
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

/// A cell of a table row with text, rendered on its own.
struct Cell {
    th: bool,
    out: Out,
    /// The cell's text in `out.buf`, trimmed.
    text: Range<usize>,
}

impl Cell {
    fn text(&self) -> &str {
        &self.out.buf[self.text.clone()]
    }
}

/// Print a table row: as one line of tab-separated cells, or, when a cell has
/// several lines or the row is long, with each cell starting a line.
fn render_row(tr: NodeRef<Node>, out: &mut Out) {
    let mut cells = Vec::new();
    for child in tr.children() {
        let Node::Element(el) = child.value() else { continue };
        if is_skipped(el) {
            continue;
        }
        let mut cell = Out { depth: out.depth + 1, rows: out.rows.as_ref().map(|_| Vec::new()), ..Out::default() };
        render(child, &mut cell);
        let text = cell.buf.len() - cell.buf.trim_start().len()..cell.buf.trim_end().len();
        if !text.is_empty() {
            cells.push(Cell { th: el.name() == "th", out: cell, text });
        }
    }
    if cells.is_empty() {
        return;
    }
    let (short, data_row, groups) = {
        let texts: Vec<&str> = cells.iter().map(Cell::text).collect();
        let short = texts.iter().map(|c| c.chars().count()).sum::<usize>() <= MAX_ROW_CHARS;
        let data_row = short && texts.iter().all(|c| !c.contains('\n'));
        // In a row printed cell by cell, a cell of several lines is not a
        // number, and a cell holding a table must keep the lines its rows
        // were printed as.
        let joins = |i: usize| data_row || !(texts[i].contains('\n') || cells[i].out.has_table);
        (short, data_row, merge_cells(&texts, joins))
    };
    // The lines of a row printed cell by cell with two or more cells are table
    // data, not page numbers or page furniture. A row over 1,000 characters
    // is a layout table, whose cells can hold whole pages with their page
    // numbers: there only cells of one line count.
    let all_lines = !data_row && short && groups.len() > 1;
    let mut ranges = Vec::new();
    out.newline();
    for (g, group) in groups.iter().enumerate() {
        if !data_row {
            out.newline();
        } else if g > 0 {
            out.buf.push('\t');
        }
        let at = out.buf.len();
        let mut one_line = true;
        for cell in &mut cells[group.clone()] {
            let shift = out.buf.len();
            let text = cell.text();
            out.buf.push_str(text);
            one_line &= !text.contains('\n');
            // Tables inside the cells of a row printed as one line are part of
            // its cells. Otherwise the lines and rows inside a cell move with its text.
            if !data_row {
                let moved = |p: usize| p - cell.text.start + shift;
                if !all_lines {
                    out.cell_lines.extend(cell.out.cell_lines.iter().map(|&p| moved(p)));
                }
                if let (Some(rows), Some(inner)) = (&mut out.rows, &mut cell.out.rows) {
                    for mut row in inner.drain(..) {
                        for r in &mut row.cells {
                            *r = moved(r.start)..moved(r.end);
                        }
                        rows.push(row);
                    }
                }
            }
        }
        out.mid_line = true;
        if all_lines {
            out.cell_lines.push(at);
            out.cell_lines.extend(out.buf[at..].match_indices('\n').map(|(i, _)| at + i + 1));
        } else if !data_row && one_line && groups.len() > 1 {
            out.cell_lines.push(at);
        }
        if out.rows.is_some() {
            ranges.push(at..out.buf.len());
        }
    }
    if let (Some(rows), Some(table)) = (&mut out.rows, enclosing_table(tr)) {
        let in_thead = tr.parent().and_then(|p| p.value().as_element().map(|e| e.name() == "thead"));
        let header = in_thead == Some(true) || cells.iter().all(|c| c.th);
        rows.push(RowRecord { table, header, line: data_row, cells: ranges });
    }
    out.has_table = true;
    out.newline();
}

fn enclosing_table(tr: NodeRef<Node>) -> Option<NodeId> {
    tr.ancestors().find(|n| n.value().as_element().is_some_and(|e| e.name() == "table")).map(|n| n.id())
}

/// Financial tables put `$` and closing `)` or `%` in cells of their own: the
/// runs of `cells` that each make one cell, in order. Only cells for which
/// `joins` holds are merged; a header "Quarter Ended" over "June 30,"
/// followed by a "%" column is not a number.
fn merge_cells(cells: &[&str], joins: impl Fn(usize) -> bool) -> Vec<Range<usize>> {
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(cells.len());
    // Start of the `$` or `(` cells waiting for their number.
    let mut prefix = None;
    for (i, &c) in cells.iter().enumerate() {
        let joins_this = joins(i);
        if joins_this && matches!(c, "$" | "€" | "£" | "¥" | "(" | "$(" | "($") {
            prefix.get_or_insert(i);
        } else if joins_this
            && matches!(c, ")" | "%" | ")%" | "%)")
            && prefix.is_none()
            && merged.last().is_some_and(|m| joins(m.end - 1))
        {
            merged.last_mut().expect("checked non-empty").end = i + 1;
        } else {
            if let Some(start) = prefix.filter(|_| !joins_this) {
                merged.push(start..i);
                prefix = None;
            }
            merged.push(prefix.take().unwrap_or(i)..i + 1);
        }
    }
    if let Some(start) = prefix {
        merged.push(start..cells.len());
    }
    merged
}

/// Trim lines and drop empty lines, bare page numbers, "Table of Contents"
/// links and running page headers and footers.
fn finish(text: &str) -> String {
    join_lines(&kept_lines(text, &[]))
}

/// The lines [`finish`] keeps, trimmed, as slices of `text` in order. The
/// lines starting at the sorted offsets `cell_lines` belong to table cells
/// (see `Out::cell_lines`) and are not dropped as page numbers or page furniture.
fn kept_lines<'a>(text: &'a str, cell_lines: &[usize]) -> Vec<&'a str> {
    static PAGE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:(?i:page)\s+)?(?:\d{1,3}|[ivxlc]{1,7}|F-\d{1,3}|[-–—]\s*\d{1,3}\s*[-–—])$").unwrap()
    });
    debug_assert!(cell_lines.is_sorted(), "cell lines are recorded in text order");
    let mut cell_lines = cell_lines.iter().copied().peekable();
    let mut lines = Vec::new();
    let mut is_cell = Vec::new();
    for line in text.lines() {
        let start = line.as_ptr() as usize - text.as_ptr() as usize;
        while cell_lines.next_if(|&p| p < start).is_some() {}
        let cell = cell_lines.next_if_eq(&start).is_some();
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("table of contents") || (!cell && PAGE_NUMBER.is_match(line)) {
            continue;
        }
        lines.push(line);
        is_cell.push(cell);
    }
    let furniture = page_furniture(&lines, &is_cell);
    lines.into_iter().enumerate().filter(|(i, _)| !furniture.contains(i)).map(|(_, l)| l).collect()
}

/// Each line followed by a newline.
fn join_lines(lines: &[&str]) -> String {
    let mut out = String::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Turn the rows recorded while rendering `raw` into tables of `text`.
/// `lines` are the lines of `raw` that [`finish`] kept, as slices of `raw`,
/// and `text` is those lines joined. Each cell becomes what its own lines
/// became in `text`, so lines that `finish` dropped, for whatever reason,
/// leave the cells and rows they were in.
fn locate_tables(raw: &str, lines: &[&str], text: &str, rows: Vec<RowRecord>) -> Vec<Table> {
    // Start of each kept line in `raw` and in `text`, and its length.
    let mut spans = Vec::with_capacity(lines.len());
    let mut at = 0;
    for line in lines {
        let start = line.as_ptr() as usize - raw.as_ptr() as usize;
        debug_assert!(start + line.len() <= raw.len(), "kept lines are slices of the rendered text");
        spans.push((start, at, line.len()));
        at += line.len() + 1;
    }
    // The part of `text` that a range of `raw` became, if any of it was kept.
    let map = |r: &Range<usize>| {
        let first = spans.partition_point(|&(s, _, len)| s + len <= r.start);
        let last = spans.partition_point(|&(s, _, _)| s < r.end);
        (first < last).then(|| {
            let (s0, t0, _) = spans[first];
            let (s1, t1, len1) = spans[last - 1];
            t0 + r.start.saturating_sub(s0)..t1 + len1.min(r.end - s1)
        })
    };
    // Tables in the order their first row appears, and whether any row is a line of cells.
    let mut found: Vec<(Table, bool)> = Vec::new();
    let mut index: HashMap<NodeId, usize> = HashMap::new();
    for row in rows {
        let cells: Vec<Range<usize>> = row.cells.iter().filter_map(map).collect();
        let (Some(first), Some(last)) = (cells.first(), cells.last()) else { continue };
        let i = *index.entry(row.table).or_insert_with(|| {
            found.push((Table { start: first.start, end: last.end, header_rows: Vec::new(), rows: Vec::new() }, false));
            found.len() - 1
        });
        let (table, has_line) = &mut found[i];
        table.start = table.start.min(first.start);
        table.end = table.end.max(last.end);
        *has_line |= row.line;
        if row.header {
            table.header_rows.push(table.rows.len());
        }
        table.rows.push(cells.into_iter().map(|r| text[r].to_owned()).collect());
    }
    let mut tables: Vec<Table> = found.into_iter().filter(|(_, has_line)| *has_line).map(|(t, _)| t).collect();
    for t in &mut tables {
        // A cell ends at the end of a line: take in the newline after it.
        t.end = text[t.end..].find('\n').map_or(text.len(), |i| t.end + i + 1);
    }
    // A table nested at the very start of another one comes after it.
    tables.sort_by_key(|t| (t.start, std::cmp::Reverse(t.end)));
    tables
}

/// Running page headers and footers: short lines that repeat with only a page
/// number changing, such as "Apple Inc. | 2026 Form 10-K | 23",
/// "26  JPMorgan Chase & Co./2007 Annual Report", "Page 3 of 25 (Form 10-K)",
/// or a row of three cells with the page number in the middle one,
/// "2021 Form 10-K<TAB>95<TAB>WEC Energy Group, Inc." (where front matter
/// pages may be numbered "i", "ii" and so on).
///
/// The page numbers must mostly increase through the document, which data
/// rows do not do. The text beside the number needs 10 letters, because short
/// lines such as cross-references ("See page 4") also come with rising
/// numbers. Shorter text is enough when it names the form ("2021 Form 10-K
/// 100") or the line starts with "Page N of M" ("Page 3 of 26"), and the
/// copies are mostly not on consecutive lines, as the rows of a table are.
/// Rows with the number in the middle must also mostly not be on consecutive
/// lines, whatever their length. Lines of table cells (`is_cell`) are data
/// and never count.
fn page_furniture(lines: &[&str], is_cell: &[bool]) -> HashSet<usize> {
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
    // Filings also spell the dash as a non-breaking hyphen (U+2011) or an en dash.
    static FORM_NAME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b(?:10[-‐‑‒–—][KQ]|8[-‐‑‒–—]K|20[-‐‑‒–—]F|40[-‐‑‒–—]F)").unwrap());
    let letters = |s: &str| s.chars().filter(|c| c.is_alphabetic()).count();
    // Line indexes and page numbers, keyed as page_number keys them.
    let mut families: HashMap<PageKey, Vec<(usize, u32)>> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        if line.len() > 120 || is_cell[i] {
            continue;
        }
        let Some((num, key)) = page_number(line, &NUMBERED) else { continue };
        let (before, after, page_first) = key;
        let short = letters(before) + letters(after) < MIN_LETTERS;
        let form = FORM_NAME.is_match(before) || FORM_NAME.is_match(after);
        if !(before.contains(' ') || after.contains(' ')) || (short && !page_first && !form) {
            continue;
        }
        families.entry(key).or_default().push((i, num));
    }
    let mut drop = HashSet::new();
    for (&(before, after, _), seen) in &families {
        if seen.len() < MIN_REPEATS {
            continue;
        }
        let mostly = |count: usize| count * 10 >= (seen.len() - 1) * 9;
        let rising = seen.windows(2).filter(|w| w[1].1 > w[0].1).count();
        let apart = seen.windows(2).filter(|w| w[1].0 > w[0].0 + 1).count();
        let short = letters(before) + letters(after) < MIN_LETTERS;
        if mostly(rising) && ((!short && after.is_empty()) || mostly(apart)) {
            drop.extend(seen.iter().map(|&(i, _)| i));
        }
    }
    drop
}

/// The text before and after a page number, and whether the line starts with
/// "Page N of M". A number at the start or end of a line leaves its text in
/// the first place, so that headers with the number on alternate sides of the
/// page are one family.
type PageKey<'a> = (&'a str, &'a str, bool);

/// The page number of a line that may be a running header or footer, and the
/// text around it.
fn page_number<'a>(line: &'a str, numbered: &Regex) -> Option<(u32, PageKey<'a>)> {
    let edge = line.starts_with(|c: char| c.is_ascii_digit())
        || line.ends_with(|c: char| c.is_ascii_digit())
        || line.get(..4).is_some_and(|s| s.eq_ignore_ascii_case("page"));
    if let Some(c) = edge.then(|| numbered.captures(line)).flatten() {
        let num = c.get(1).or_else(|| c.get(3)).or_else(|| c.get(6))?;
        let rest = c.get(2).or_else(|| c.get(4)).or_else(|| c.get(5))?;
        let rest = rest.as_str().trim_matches(|c: char| c == '|' || c.is_whitespace());
        return Some((num.as_str().parse().ok()?, (rest, "", c.get(3).is_some())));
    }
    // A row of three cells with the page number in the middle one, in digits
    // or, in the front matter, in roman numerals.
    let mut cells = line.split('\t');
    let (Some(before), Some(num), Some(after), None) = (cells.next(), cells.next(), cells.next(), cells.next()) else {
        return None;
    };
    let n = if num.len() <= 3 && num.bytes().all(|b| b.is_ascii_digit()) { num.parse().ok() } else { roman(num) };
    Some((n?, (before, after, false)))
}

/// The value of a lowercase roman numeral of up to 7 letters, such as "xiv".
fn roman(s: &str) -> Option<u32> {
    let value = |b: u8| match b {
        b'i' => Some(1),
        b'v' => Some(5),
        b'x' => Some(10),
        b'l' => Some(50),
        b'c' => Some(100),
        _ => None,
    };
    if s.len() > 7 {
        return None;
    }
    let mut total = 0;
    let mut letters = s.bytes().peekable();
    while let Some(b) = letters.next() {
        let v = value(b)?;
        let next = letters.peek().map_or(Some(0), |&n| value(n))?;
        total += if next > v { -v } else { v };
    }
    u32::try_from(total).ok().filter(|&n| n > 0)
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
        // form, with any of the dashes filings use, or starts with "Page N of M".
        for header in [
            "2021 FORM 10-K {}",
            "Form 10-Q Page {}",
            "{} | Form 10-K",
            "Page {} of 125 (Form 10-K)",
            "PAGE {} OF 125",
            "FORM 10\u{2011}K {}",
            "Form 10\u{2013}Q {}",
        ] {
            let mut html = String::new();
            let mut want = String::new();
            for page in 98..104 {
                html += &format!("<p>Text of page {page}.</p><p>{}</p>", header.replace("{}", &page.to_string()));
                want += &format!("Text of page {page}.\n");
            }
            assert_eq!(html_to_text(&html), want, "{header}");
        }
    }

    /// Footers printed as a row of three cells with the page number in the
    /// middle one, from the roman-numbered front matter on, leave the text
    /// and the tables.
    #[test]
    fn running_footers_with_the_page_number_in_the_middle_dropped() {
        let mut html = String::new();
        let mut want = String::new();
        let pages = ["i", "ii", "iii"].map(String::from).into_iter().chain((1..13).map(|n| n.to_string()));
        for page in pages {
            html += &format!(
                "<p>Text of page {page}.</p><table><tr><td>2021 Form 10-K</td><td>&#160;</td>\
                 <td>{page}</td><td>WEC Energy Group, Inc.</td></tr></table>"
            );
            want += &format!("Text of page {page}.\n");
        }
        let (text, tables) = tables_of(&html);
        assert_eq!(text, want);
        assert!(tables.is_empty(), "{tables:?}");
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
        for n in 60..66 {
            html += &format!("<tr><td>Annual Report on Form 10-K</td><td>{n}</td><td>Exhibit index</td></tr>");
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
            ("Annual Report on Form 10-K\t", 6),
        ] {
            assert_eq!(text.lines().filter(|l| l.starts_with(start)).count(), copies, "{start}");
        }
    }

    #[test]
    fn deep_nesting_does_not_overflow() {
        let html = format!("{}deep{}", "<font>".repeat(100_000), "</font>".repeat(100_000));
        assert_eq!(html_to_text(&html), "deep\n");
    }

    /// Tables nested thousands deep, written out or left unclosed, convert on
    /// a thread with a 2 MiB stack, the default for spawned threads.
    #[test]
    fn deeply_nested_tables_do_not_overflow() {
        let convert = |html: String| {
            let thread = std::thread::Builder::new().stack_size(2 << 20);
            let handle = thread.spawn(move || (html_to_text(&html), html_to_text_and_tables(&html))).unwrap();
            handle.join().expect("no stack overflow")
        };
        let nested = format!("{}deep{}", "<table><tr><td>".repeat(10_000), "</td></tr></table>".repeat(10_000));
        let (text, (with_tables, tables)) = convert(nested);
        assert_eq!(text, "deep\n");
        assert_eq!(with_tables, text);
        let [t] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(rows(t), [["deep"]]);

        // Each unclosed row's last cell holds the next table.
        let unclosed = "<table><tr><td>Revenue</td><td>12,345</td></tr><tr><td>".repeat(5_000);
        let (text, (with_tables, tables)) = convert(unclosed);
        assert_eq!(text.matches("12,345").count(), 5_000);
        assert_eq!(with_tables, text);
        assert!(text.starts_with("Revenue\t12,345\nRevenue\t12,345\n"));
        assert_eq!(rows(&tables[0])[0], ["Revenue", "12,345"]);
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

    /// Text and tables of `html`, after checking that the text is what
    /// html_to_text returns and that each table's text is its rows: a line of
    /// tab-separated cells per row, or the cells one after another.
    fn tables_of(html: &str) -> (String, Vec<Table>) {
        let (text, tables) = html_to_text_and_tables(html);
        assert_eq!(text, html_to_text(html));
        for t in &tables {
            let mut at = t.start;
            for row in &t.rows {
                let line = format!("{}\n", row.join("\t"));
                let blocks: String = row.iter().map(|c| format!("{c}\n")).collect();
                let rest = &text[at..];
                at += if rest.starts_with(&line) {
                    line.len()
                } else if rest.starts_with(&blocks) {
                    blocks.len()
                } else {
                    panic!("row {row:?} is not at {at} in {text:?}")
                };
            }
            assert_eq!(at, t.end, "{t:?}");
        }
        (text, tables)
    }

    fn rows(t: &Table) -> Vec<Vec<&str>> {
        t.rows.iter().map(|r| r.iter().map(String::as_str).collect()).collect()
    }

    #[test]
    fn table_cells_merge_as_in_the_text() {
        let html = r#"<p>Results of operations</p><table>
            <tr><td></td><td>2024</td><td>2023</td></tr>
            <tr><td>Net sales</td><td>$</td><td>391,035</td><td>$</td><td>383,285</td></tr>
            <tr><td>Net loss</td><td>(1,234</td><td>)</td><td>12.5</td><td>%</td></tr>
        </table><p>After the table.</p>"#;
        let (text, tables) = tables_of(html);
        let [t] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(
            rows(t),
            [vec!["2024", "2023"], vec!["Net sales", "$391,035", "$383,285"], vec!["Net loss", "(1,234)", "12.5%"]]
        );
        assert_eq!(&text[t.start..t.end], "2024\t2023\nNet sales\t$391,035\t$383,285\nNet loss\t(1,234)\t12.5%\n");
        assert!(t.header_rows.is_empty());
    }

    /// A cell spanning several columns or rows is one cell, as in the text.
    #[test]
    fn spanning_cells_are_single_cells() {
        let html = r#"<table>
            <tr><td rowspan="2">Segment</td><td colspan="3">Year ended December 31,</td></tr>
            <tr><td>2024</td><td>2023</td><td>2022</td></tr>
            <tr><td>Americas</td><td colspan="2">167,045</td><td>162,560</td></tr>
        </table>"#;
        let (_, tables) = tables_of(html);
        assert_eq!(
            rows(&tables[0]),
            [
                vec!["Segment", "Year ended December 31,"],
                vec!["2024", "2023", "2022"],
                vec!["Americas", "167,045", "162,560"]
            ]
        );
    }

    #[test]
    fn empty_rows_and_cells_are_left_out() {
        let html = "<table><tr><td>&nbsp;</td><td></td></tr><tr><td>Revenue</td><td> </td><td>10</td></tr>\
                    <tr><td colspan=3><hr></td></tr><tr><td>Costs</td><td>4</td></tr><tr></tr></table>\
                    <table><tr><td>&#160;</td></tr></table>";
        let (text, tables) = tables_of(html);
        assert_eq!(text, "Revenue\t10\nCosts\t4\n");
        let [t] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(rows(t), [vec!["Revenue", "10"], vec!["Costs", "4"]]);
    }

    #[test]
    fn header_rows_come_from_thead_and_th() {
        let html = "<table><thead><tr><td>Name</td><td>Position</td></tr></thead><tbody>\
                    <tr><th></th><th>2024</th><th>2023</th></tr>\
                    <tr><th>Revenue</th><td>10</td><td>9</td></tr></tbody></table>";
        let (_, tables) = tables_of(html);
        // A row label in a <th> does not make its row a header.
        assert_eq!(tables[0].header_rows, [0, 1]);
        assert_eq!(rows(&tables[0])[1], ["2024", "2023"]);
    }

    /// A row with a cell of several lines is printed cell by cell, and its
    /// cells keep their line breaks.
    #[test]
    fn cells_of_several_lines() {
        let html = "<p>RSU activity:</p><table>\
                    <tr><td></td><td>Number of<br>RSUs</td><td><p>Fair</p><p>Value</p></td></tr>\
                    <tr><td>Granted</td><td>75,538</td><td>$</td><td>260.81</td></tr></table>";
        let (text, tables) = tables_of(html);
        let t = &tables[0];
        assert_eq!(rows(t), [vec!["Number of\nRSUs", "Fair\nValue"], vec!["Granted", "75,538", "$260.81"]]);
        assert_eq!(&text[t.start..t.end], "Number of\nRSUs\nFair\nValue\nGranted\t75,538\t$260.81\n");
    }

    /// In a row printed cell by cell, values that look like page numbers
    /// ("390", "x", the "2" under "49") are data, not page numbers, and so are
    /// lines that look like running footers.
    #[test]
    fn lines_of_row_cells_are_not_page_furniture() {
        let mut html = "<table><tr><td></td><td>Q3 2025</td><td>Q3 2024</td><td>Note</td></tr>\
                        <tr><td>Income from operations<br>before income taxes</td><td>390</td><td>16</td><td>x</td></tr>\
                        <tr><td>Weighted average<br>shares</td><td>177</td><td>171</td><td>ii</td></tr>\
                        <tr><td>Accounting change</td><td>As reported<br>Pro forma</td><td>$</td><td>49<br>2</td></tr>"
            .to_owned();
        for n in 1..7 {
            html += &format!("<tr><td>Segment<br>results</td><td>Operating income of segment {n}</td></tr>");
        }
        html += "</table><p>Text.</p><p>12</p><table><tr><td>26</td></tr></table>";
        let (text, tables) = tables_of(&html);
        assert!(text.starts_with(
            "Q3 2025\tQ3 2024\tNote\nIncome from operations\nbefore income taxes\n390\n16\nx\n\
             Weighted average\nshares\n177\n171\nii\nAccounting change\nAs reported\nPro forma\n$\n49\n2\n\
             Segment\nresults\nOperating income of segment 1\n"
        ));
        assert_eq!(text.matches("Operating income of segment").count(), 6);
        // Page numbers outside such rows still go.
        assert!(text.ends_with("Operating income of segment 6\nText.\n"));
        let [t] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(rows(t)[1], ["Income from operations\nbefore income taxes", "390", "16", "x"]);
        assert_eq!(rows(t)[2], ["Weighted average\nshares", "177", "171", "ii"]);
        assert_eq!(rows(t)[3], ["Accounting change", "As reported\nPro forma", "$", "49\n2"]);

        // A row over 1,000 characters is a layout table: a cell of one line
        // there is still data, but a page number under a cell's text goes.
        let long = "word ".repeat(300);
        let html = format!(
            "<table><tr><td>Christopher T. Hjelm</td><td>57</td><td>{long}</td></tr>\
             <tr><td><p>{long}</p><p>27</p></td><td>Side note</td></tr></table>"
        );
        let text = html_to_text(&html);
        assert!(text.starts_with("Christopher T. Hjelm\n57\nword word"));
        assert!(text.ends_with("word\nSide note\n"));
    }

    /// `$` and `)` or `%` cells merge into their numbers in rows printed cell
    /// by cell too, but not into cells of several lines or cells holding a
    /// table, and tables inside later cells keep their places.
    #[test]
    fn cells_merge_in_rows_printed_cell_by_cell() {
        let html = "<table><tr><td>Quarter Ended<br>June 30,</td><td>%</td><td>$</td><td>Six Months<br>Ended</td></tr>\
                    <tr><td></td><td>2025</td><td>2024</td></tr>\
                    <tr><td>Income (loss)<br>before taxes</td><td>$</td><td>(1,998</td><td>)</td>\
                    <td>$</td><td>1,006</td><td>12.5</td><td>%</td>\
                    <td><table><tr><td>A</td><td>$</td><td>1</td></tr><tr><td>B</td><td>2</td></tr></table></td></tr>\
                    <tr><td>Rate<br>change</td><td>$</td><td><table><tr><td>C</td><td>3</td></tr></table></td>\
                    <td>%</td></tr></table>";
        let (text, tables) = tables_of(html);
        assert_eq!(
            text,
            "Quarter Ended\nJune 30,\n%\n$\nSix Months\nEnded\n2025\t2024\n\
             Income (loss)\nbefore taxes\n$(1,998)\n$1,006\n12.5%\nA\t$1\nB\t2\nRate\nchange\n$\nC\t3\n%\n"
        );
        let [outer, inner, last] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(
            rows(outer),
            [
                vec!["Quarter Ended\nJune 30,", "%", "$", "Six Months\nEnded"],
                vec!["2025", "2024"],
                vec!["Income (loss)\nbefore taxes", "$(1,998)", "$1,006", "12.5%", "A\t$1\nB\t2"],
                vec!["Rate\nchange", "$", "C\t3", "%"]
            ]
        );
        assert_eq!(rows(inner), [["A", "$1"], ["B", "2"]]);
        assert_eq!(&text[inner.start..inner.end], "A\t$1\nB\t2\n");
        assert_eq!(rows(last), [["C", "3"]]);
    }

    #[test]
    fn layout_tables_are_not_returned_but_tables_inside_them_are() {
        let long = "word ".repeat(300);
        let html = format!(
            "<table><tr><td><p>ITEM 7. MD&amp;A</p><p>{long}</p>\
             <table><tr><td>Revenue</td><td>$</td><td>10</td></tr><tr><td>Costs</td><td>4</td></tr></table>\
             <p>More text.</p></td></tr><tr><td><p>{long}</p></td></tr></table>"
        );
        let (text, tables) = tables_of(&html);
        assert!(text.starts_with("ITEM 7. MD&A\nword word"));
        let [t] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(rows(t), [vec!["Revenue", "$10"], vec!["Costs", "4"]]);
        assert_eq!(&text[t.start..t.end], "Revenue\t$10\nCosts\t4\n");
    }

    #[test]
    fn nested_tables() {
        let html = "<table><tr><td>Segment</td><td>Total</td></tr>\
            <tr><td>Detail</td><td><table><tr><td>A</td><td>1</td></tr><tr><td>B</td><td>2</td></tr></table></td></tr>\
            <tr><td>Rate</td><td><table><tr><td>5</td><td>%</td></tr></table></td></tr></table>";
        let (text, tables) = tables_of(html);
        assert_eq!(text, "Segment\tTotal\nDetail\nA\t1\nB\t2\nRate\t5%\n");
        // The one-row table became a cell of the outer table's last row.
        let [outer, inner] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(rows(outer), [vec!["Segment", "Total"], vec!["Detail", "A\t1\nB\t2"], vec!["Rate", "5%"]]);
        assert_eq!(rows(inner), [vec!["A", "1"], vec!["B", "2"]]);
        assert!(outer.start < inner.start && inner.end <= outer.end);
        assert_eq!(&text[inner.start..inner.end], "A\t1\nB\t2\n");

        // A table at the very start of the outer one comes after it.
        let html = "<table><tr><td><table><tr><td>A</td><td>1</td></tr><tr><td>B</td><td>2</td></tr></table></td>\
                    <td>Total</td></tr><tr><td>Sum</td><td>3</td></tr></table>";
        let (_, tables) = tables_of(html);
        let [outer, inner] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(outer.start, inner.start);
        assert_eq!(rows(outer), [vec!["A\t1\nB\t2", "Total"], vec!["Sum", "3"]]);
        assert_eq!(rows(inner), [vec!["A", "1"], vec!["B", "2"]]);
    }

    /// Lines dropped as page numbers or running footers leave the rows and
    /// cells they were in, and the offsets of later tables follow.
    #[test]
    fn table_offsets_skip_removed_page_furniture() {
        let mut html = String::new();
        for page in 20..26 {
            html += &format!(
                "<p>Text of page {page}.</p><p>{page}</p><p>Table of Contents</p>\
                 <table><tr><td>Apple Inc. | 2026 Form 10-K | {page}</td></tr></table>"
            );
        }
        html += "<table><tr><td>26</td></tr><tr><td>Note<br>3</td><td>Amount<br>(in millions)</td></tr>\
                 <tr><td>Net sales</td><td>$</td><td>391,035</td></tr>\
                 <tr><td>Apple Inc. | 2026 Form 10-K | 26</td></tr></table>";
        let (text, tables) = tables_of(&html);
        assert!(!text.contains("Form 10-K") && !text.contains("Contents") && !text.contains("\n26\n"));
        let [t] = &tables[..] else { panic!("{tables:?}") };
        // The "3" is a line of a cell in a row of two cells, so it is data.
        assert_eq!(rows(t), [vec!["Note\n3", "Amount\n(in millions)"], vec!["Net sales", "$391,035"]]);
        assert_eq!(&text[t.start..], "Note\n3\nAmount\n(in millions)\nNet sales\t$391,035\n");
    }

    /// Sections and tables share the text, so a table belongs to the section
    /// whose range holds its start.
    #[test]
    fn tables_fall_in_sections_by_offset() {
        let body = "<p>We make and sell things in many markets around the world, every year.</p>".repeat(30);
        let html = format!(
            "<p>Item 7. Management's Discussion and Analysis</p>{body}\
             <table><tr><td>Net sales</td><td>100</td></tr></table>{body}\
             <p>Item 8. Financial Statements and Supplementary Data</p>{body}\
             <table><tr><td>Total assets</td><td>500</td></tr></table>{body}"
        );
        let (text, tables) = tables_of(&html);
        let sections = crate::extract_sections(&text, crate::FormKind::TenK);
        let holder = |t: &Table| sections.iter().find(|s| s.start <= t.start && t.start < s.end).map(|s| s.id.as_str());
        assert_eq!(tables.iter().map(holder).collect::<Vec<_>>(), [Some("item_7"), Some("item_8")]);
    }

    #[test]
    fn plain_text_documents_have_no_tables() {
        let raw = "<DOCUMENT>\n<TEXT>\n<TABLE>\n<S>      <C>\nNet sales   $ 6,134\n</TABLE>\n</TEXT>";
        assert_eq!(document_to_text_and_tables(raw), (document_to_text(raw), Vec::new()));
        let html = "<html><body><table><tr><td>Net sales</td><td>6,134</td></tr></table></body></html>";
        assert_eq!(document_to_text_and_tables(html), html_to_text_and_tables(html));
    }
}
