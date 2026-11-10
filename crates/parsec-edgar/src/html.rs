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
    /// starts a line of the text and keeps its line breaks.
    ///
    /// A row printed as one line is removed as a running header or footer
    /// like any other line. In a row of two or more cells printed cell by
    /// cell, no line is removed on its own as a page number or running
    /// header (in a row over 1,000 characters, this holds only for cells of
    /// one line), so values such as "390" stay. A short row printed cell by
    /// cell is removed as a whole when it repeats on five or more pages with
    /// only a rising page number changing, as a running footer does.
    pub rows: Vec<Vec<String>>,
    /// The grid column of each cell of `rows`, row by row: the first column
    /// of the table that the cell covers, counting from 0, with `colspan` and
    /// `rowspan` laid out as browsers lay them out. Empty cells, which `rows`
    /// leaves out, still take their columns, so a value lines up with the
    /// header above it even when the two rows have different numbers of
    /// cells. A cell hidden with `display:none` or the `hidden` attribute
    /// takes no column, as in a browser, and neither does anything in a row
    /// that is not a `td` or `th` cell, such as a hidden `<input>`. A cell
    /// merged from a `$` or `(` cell, its number and a `)` or `%` cell takes
    /// the column of its number. The cells of a table inside a cell of
    /// another one count the columns of their own table.
    #[serde(default)]
    pub columns: Vec<Vec<usize>>,
    /// How many grid columns each cell of `rows` covers, row by row: its
    /// `colspan`, or for a merged cell that of its number. A header covers
    /// the values whose column `c` has `columns <= c < columns + spans`.
    #[serde(default)]
    pub spans: Vec<Vec<usize>>,
}

/// Convert an HTML document to clean plain text.
pub fn html_to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let mut out = Out::default();
    render(doc.tree.root(), &mut out);
    join_lines(&kept_lines(&out.buf, &out.cell_lines, &out.cell_rows, &out.breaks))
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
    let lines = kept_lines(&out.buf, &out.cell_lines, &out.cell_rows, &out.breaks);
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

/// Whether [`document_to_text`] reads `raw` as HTML. A plain-text document,
/// such as most filings before 2001, has its tables only in the text, so
/// [`document_to_text_and_tables`] returns none for it.
pub fn document_is_html(raw: &str) -> bool {
    looks_like_html(first_document_body(raw).unwrap_or(raw))
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

/// Old plain-text filings: drop SGML layout tags, then the same line cleanup
/// as HTML. Some filers number the page tags, as in "<Page 2>". A page tag
/// starts a printed page, so it is where the page before it ends.
fn plain_text(s: &str) -> String {
    static TAGS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)</?(?:(PAGE)(?:[ \t]+\d{1,3})?|S|C|CAPTION|TABLE|FN)>").unwrap());
    let mut text = String::with_capacity(s.len());
    let mut breaks = Vec::new();
    let mut last = 0;
    for tag in TAGS.captures_iter(s) {
        let all = tag.get(0).expect("group 0 is the whole match");
        text.push_str(&s[last..all.start()]);
        if tag.get(1).is_some() {
            breaks.push(text.len());
        }
        last = all.end();
    }
    text.push_str(&s[last..]);
    join_lines(&kept_lines(&text, &[], &[], &breaks))
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
    /// page furniture on their own.
    cell_lines: Vec<usize>,
    /// Where in `buf` each row printed cell by cell with two or more cells and
    /// at most 1,000 characters is, in order. Such a row is removed as a whole
    /// when it repeats on several pages as a running header or footer.
    cell_rows: Vec<Range<usize>>,
    /// Where in `buf` a printed page ends, in order: before or after each
    /// element styled to break the page there.
    breaks: Vec<usize>,
    /// Table rows rendered into `buf`, recorded only when tables are wanted.
    rows: Option<Vec<RowRecord>>,
    /// For each row group (`<tbody>`, `<thead>` or `<tfoot>`) whose rows are
    /// recorded, the cells of the rows so far that span rows below: the grid
    /// columns they cover and how many more rows they cover.
    spanning: HashMap<NodeId, Vec<(Range<usize>, usize)>>,
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
    /// The grid columns each cell covers.
    columns: Vec<Range<usize>>,
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

/// Whether an element's style starts a printed page before it and whether it
/// ends one after it: `page-break-before: always` or `break-before: page`,
/// and the same with "after".
fn page_breaks(el: &Element) -> (bool, bool) {
    let style = el.attr("style").unwrap_or_default();
    if !style.as_bytes().windows(5).any(|w| w.eq_ignore_ascii_case(b"break")) {
        return (false, false);
    }
    let s: String = style.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    let before = s.contains("page-break-before:always") || s.contains("break-before:page");
    let after = s.contains("page-break-after:always") || s.contains("break-after:page");
    (before, after)
}

/// Depth-first render with an explicit stack: some filings nest tags thousands
/// deep, which would overflow the call stack with naive recursion.
fn render(root: NodeRef<Node>, out: &mut Out) {
    enum Step<'a> {
        Enter(NodeRef<'a, Node>),
        Exit,
        /// The end of an element styled to break the page after it.
        Break,
    }
    let mut stack = vec![Step::Enter(root)];
    while let Some(step) = stack.pop() {
        let node = match step {
            Step::Exit => {
                out.newline();
                continue;
            }
            Step::Break => {
                out.breaks.push(out.buf.len());
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
                let (before, after) = page_breaks(el);
                if before {
                    out.breaks.push(out.buf.len());
                }
                // Pushed before the element's own steps, so it is taken after them.
                if after {
                    stack.push(Step::Break);
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
    /// The grid columns the cell covers, when tables are wanted.
    columns: Range<usize>,
}

impl Cell {
    fn text(&self) -> &str {
        &self.out.buf[self.text.clone()]
    }
}

/// Print a table row: as one line of tab-separated cells, or, when a cell has
/// several lines or the row is long, with each cell starting a line.
fn render_row(tr: NodeRef<Node>, out: &mut Out) {
    let elements: Vec<(NodeRef<Node>, &Element)> = tr
        .children()
        .filter_map(|child| Some((child, child.value().as_element()?)))
        .filter(|(_, el)| !is_skipped(el))
        .collect();
    // Grid columns are laid out only when tables are wanted. Empty cells take
    // theirs too. Only `td` and `th` are cells, as in the HTML table model,
    // so a hidden `<input>` or an empty `<form>` that the parser leaves in a
    // row takes no column.
    let mut columns = match out.rows {
        Some(_) => {
            let group = tr.parent().map_or(tr.id(), |p| p.id());
            let spans = elements
                .iter()
                .map(|(_, el)| if matches!(el.name(), "td" | "th") { colspan_rowspan(el) } else { (0, 1) });
            place_cells(out.spanning.entry(group).or_default(), spans)
        }
        None => Vec::new(),
    }
    .into_iter();
    let mut cells = Vec::new();
    for (child, el) in elements {
        let columns = columns.next().unwrap_or(0..0);
        let mut cell = Out { depth: out.depth + 1, rows: out.rows.as_ref().map(|_| Vec::new()), ..Out::default() };
        render(child, &mut cell);
        let text = cell.buf.len() - cell.buf.trim_start().len()..cell.buf.trim_end().len();
        if !text.is_empty() {
            cells.push(Cell { th: el.name() == "th", out: cell, text, columns });
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
    let (mut ranges, mut grid) = (Vec::new(), Vec::new());
    out.newline();
    let row_start = out.buf.len();
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
            // its cells. Otherwise the lines, rows and page breaks inside a
            // cell move with its text.
            if !data_row {
                let moved = |p: usize| p - cell.text.start + shift;
                let within = |p: usize| p.clamp(cell.text.start, cell.text.end);
                out.breaks.extend(cell.out.breaks.iter().map(|&p| moved(within(p))));
                if !all_lines {
                    out.cell_lines.extend(cell.out.cell_lines.iter().map(|&p| moved(p)));
                    out.cell_rows.extend(cell.out.cell_rows.iter().map(|r| moved(r.start)..moved(r.end)));
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
            // A merged cell takes the columns of its number, the first of its
            // cells that is not a `$` or `(`.
            let number = group.clone().find(|&i| !is_prefix(cells[i].text())).unwrap_or(group.start);
            grid.push(cells[number].columns.clone());
        }
    }
    if all_lines {
        out.cell_rows.push(row_start..out.buf.len());
    }
    if let (Some(rows), Some(table)) = (&mut out.rows, enclosing_table(tr)) {
        let in_thead = tr.parent().and_then(|p| p.value().as_element().map(|e| e.name() == "thead"));
        let header = in_thead == Some(true) || cells.iter().all(|c| c.th);
        rows.push(RowRecord { table, header, line: data_row, cells: ranges, columns: grid });
    }
    out.has_table = true;
    out.newline();
}

fn enclosing_table(tr: NodeRef<Node>) -> Option<NodeId> {
    tr.ancestors().find(|n| n.value().as_element().is_some_and(|e| e.name() == "table")).map(|n| n.id())
}

/// The `colspan` and `rowspan` of a table cell, read as browsers read them:
/// the leading digits, 1 when there are none or they are 0, and at most 1,000
/// and 65,534. A `rowspan` of 0, which browsers stretch to the end of the row
/// group, counts as 1.
fn colspan_rowspan(el: &Element) -> (usize, usize) {
    let read = |name: &str, max: usize| {
        let value = el.attr(name).unwrap_or("").trim_start();
        let value = value.strip_prefix('+').unwrap_or(value);
        let digits = &value[..value.find(|c: char| !c.is_ascii_digit()).unwrap_or(value.len())];
        match digits.parse::<usize>() {
            Ok(0) => 1,
            Ok(n) => n.min(max),
            Err(_) if digits.is_empty() => 1,
            Err(_) => max,
        }
    };
    (read("colspan", 1000), read("rowspan", 65534))
}

/// A rowspan is followed only for a cell that starts before this grid
/// column. Tables in filings are far narrower, and the limit keeps the work
/// for a row in proportion to its cells even in a table made of thousands of
/// rows that each span to the end.
const MAX_SPANNING_COLUMN: usize = 1000;

/// Lay out one row of a table's grid: the columns that each cell, given as
/// its colspan and rowspan, covers. A cell takes the first column that no
/// cell of the rows above covers with its rowspan; a colspan of 0, given
/// for an element that is not a cell, places an empty range there.
/// `spanning` holds those cells of the rows above, as the columns they cover
/// and how many more rows they cover, in the order of their first column,
/// and is updated for the next row.
fn place_cells(
    spanning: &mut Vec<(Range<usize>, usize)>,
    cells: impl Iterator<Item = (usize, usize)>,
) -> Vec<Range<usize>> {
    let mut placed = Vec::new();
    let mut below = Vec::new();
    let (mut column, mut next) = (0, 0);
    for (colspan, rowspan) in cells {
        // Cells keep moving right, so one pass over `spanning` finds every
        // column that the rows above cover.
        while let Some((above, _)) = spanning.get(next) {
            if above.start > column {
                break;
            }
            column = column.max(above.end);
            next += 1;
        }
        placed.push(column..column + colspan);
        if rowspan > 1 && column < MAX_SPANNING_COLUMN {
            below.push((column..column + colspan, rowspan - 1));
        }
        column += colspan;
    }
    spanning.retain_mut(|(_, rows)| {
        *rows -= 1;
        *rows > 0
    });
    if !below.is_empty() {
        spanning.extend(below);
        spanning.sort_by_key(|(columns, _)| columns.start);
    }
    placed
}

/// Financial tables put `$` and closing `)` or `%` in cells of their own: the
/// runs of `cells` that each make one cell, in order. A `$` or `(` cell joins
/// the cell after it when that cell starts like a number, with a digit, a
/// dash or a "."; a `%` cell joins a run that ends like a number, with a
/// digit or a dash; and a `)` cell joins such a run or one that leaves a "("
/// open, such as "AA (3rd of 21". So "$" and "N/A", or a header "Amount"
/// and the "%" column after it, stay apart. Only cells for which `joins`
/// holds are merged; a header "Year Ended" over "June 30, 2025" followed by
/// a "%" column is not a number.
fn merge_cells(cells: &[&str], joins: impl Fn(usize) -> bool) -> Vec<Range<usize>> {
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(cells.len());
    // Start of the `$` or `(` cells waiting for their number.
    let mut prefix = None;
    // The last run leaves a "(" open, and it ends like a number.
    let (mut paren, mut number) = (false, false);
    for (i, &c) in cells.iter().enumerate() {
        let joins_this = joins(i);
        let closes = match c {
            ")" | ")%" | "%)" => paren || number,
            "%" => number,
            _ => false,
        };
        if joins_this && is_prefix(c) {
            prefix.get_or_insert(i);
        } else if joins_this && closes && prefix.is_none() {
            merged.last_mut().expect("a run to close").end = i + 1;
            paren &= c == "%";
        } else {
            let start = match prefix.take() {
                Some(start) if joins_this && starts_like_number(c) => start,
                Some(start) => {
                    merged.push(start..i);
                    i
                }
                None => i,
            };
            merged.push(start..i + 1);
            let open: isize = cells[start..=i]
                .iter()
                .map(|s| s.matches('(').count() as isize - s.matches(')').count() as isize)
                .sum();
            paren = joins_this && open > 0;
            number = joins_this && ends_like_number(c);
        }
    }
    if let Some(start) = prefix {
        merged.push(start..cells.len());
    }
    merged
}

/// A cell that waits for the number after it: a currency sign or "(".
fn is_prefix(cell: &str) -> bool {
    matches!(cell, "$" | "€" | "£" | "¥" | "(" | "$(" | "($")
}

const DASHES: [char; 7] = ['-', '‐', '‑', '‒', '–', '—', '−'];

/// After any currency signs, "(" and spaces, the cell starts with a digit, a
/// dash or a ".", as in "1,234", "(2.5" or "—".
fn starts_like_number(cell: &str) -> bool {
    let rest = cell.trim_start_matches(|c: char| matches!(c, '$' | '€' | '£' | '¥' | '(') || c.is_whitespace());
    rest.starts_with(|c: char| c.is_ascii_digit() || c == '.' || DASHES.contains(&c))
}

/// Before any closing ")" and "%", the cell ends with a digit or a dash.
fn ends_like_number(cell: &str) -> bool {
    let rest = cell.trim_end_matches(|c: char| matches!(c, ')' | '%') || c.is_whitespace());
    rest.ends_with(|c: char| c.is_ascii_digit() || DASHES.contains(&c))
}

/// A bare page number, such as "12", "Page 12", "F-3", "F - 3", "- 7 -",
/// "ii" or "- ii -", or "Page" and a page number with a prefix, "Page E-1",
/// with the number in the first group that matched (roman numerals in the
/// second and fifth). Old text filings escape a line that starts with a dash
/// as "- -", so "- -7-" is one too.
static PAGE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:(?:(?i:page)\s+)?",
        r"(?:(\d{1,3})|([ivxlc]{1,7})|F\s*[-‐‑‒–—−]\s*(\d{1,3})",
        r"|(?:-\s+)?[-–—]\s*(?:(\d{1,3})|([ivxlc]{1,7}))\s*[-–—])",
        r"|(?i:page)\s+[A-Z]{1,3}\s*[-‐‑‒–—−]\s*(\d{1,3}))$",
    ))
    .unwrap()
});

/// A page number alone on its line after a dash and a prefix of one to three
/// capital letters ("FS - 12", "S-3", "II-4"), a chapter number, which may
/// end in a letter as the Items do ("7-3", "7A-1"), or nothing ("- 4"), with
/// an optional final ".". The prefix is in the first group, the chapter in
/// the second and the page in the third.
static PAGE_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:([A-Z]{1,3})|(\d{1,2}[A-Z]?))?\s*[-‐‑‒–—−]\s*(\d{1,3})\.?$").unwrap());

/// A line of text with a page number with a prefix at its start or end, as
/// in "Bank of America 2013 E-1" or "II-2 | Goldman Sachs", with the text in
/// whichever of the two groups matched.
static LABELED: LazyLock<Regex> = LazyLock::new(|| {
    let label = r"(?:[A-Z]{1,3}|\d{1,2}[A-Z]?)\s*[-‐‑‒–—−]\s*\d{1,3}";
    Regex::new(&format!(r"^(?:{label}(?:\s*\|\s*|\s+)(\S.*)|(.*\S)(?:\s*\|\s*|\s+){label})$")).unwrap()
});

/// Trim the lines of `text` and drop empty lines, bare page numbers, "Table
/// of Contents" links and running page headers and footers. Returns the lines
/// kept, as slices of `text` in order. The lines starting at the sorted
/// offsets `cell_lines` belong to table cells (see `Out::cell_lines`) and are
/// not dropped as page numbers or page furniture on their own, but those of a
/// row of `cell_rows` (byte ranges of `text`, see `Out::cell_rows`) are
/// dropped together when the row is. A printed page ends at each of the
/// sorted offsets `breaks`, and at the end of the text when there are any.
fn kept_lines<'a>(text: &'a str, cell_lines: &[usize], cell_rows: &[Range<usize>], breaks: &[usize]) -> Vec<&'a str> {
    debug_assert!(cell_lines.is_sorted(), "cell lines are recorded in text order");
    debug_assert!(cell_rows.is_sorted_by_key(|r| r.start), "cell rows are recorded in text order");
    debug_assert!(breaks.is_sorted(), "page breaks are recorded in text order");
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
    // The kept lines of each row.
    let starts: Vec<usize> = lines.iter().map(|l| l.as_ptr() as usize - text.as_ptr() as usize).collect();
    let rows: Vec<Range<usize>> = cell_rows
        .iter()
        .map(|r| starts.partition_point(|&s| s < r.start)..starts.partition_point(|&s| s < r.end))
        .filter(|r| !r.is_empty())
        .collect();
    // Whether a page ends after a line and before the next kept line. In a
    // document printed on pages, the last line ends the last page.
    let ends_page: Vec<bool> = (0..lines.len())
        .map(|i| {
            let next = starts.get(i + 1).copied().unwrap_or(text.len());
            let after = breaks.partition_point(|&b| b < starts[i] + lines[i].len());
            breaks.get(after).is_some_and(|&b| b <= next) || (i + 1 == lines.len() && !breaks.is_empty())
        })
        .collect();
    let furniture = page_furniture(&lines, &is_cell, &rows, &ends_page);
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
/// `lines` are the lines of `raw` that [`kept_lines`] kept, as slices of
/// `raw`, and `text` is those lines joined. Each cell becomes what its own
/// lines became in `text`, so lines that `kept_lines` dropped, for whatever
/// reason, leave the cells and rows they were in.
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
        // The cells with any text kept, and their grid columns.
        let (cells, columns): (Vec<Range<usize>>, Vec<Range<usize>>) =
            row.cells.iter().zip(row.columns).filter_map(|(cell, columns)| Some((map(cell)?, columns))).unzip();
        let (Some(first), Some(last)) = (cells.first(), cells.last()) else { continue };
        let i = *index.entry(row.table).or_insert_with(|| {
            let table = Table {
                start: first.start,
                end: last.end,
                header_rows: Vec::new(),
                rows: Vec::new(),
                columns: Vec::new(),
                spans: Vec::new(),
            };
            found.push((table, false));
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
        table.spans.push(columns.iter().map(ExactSizeIterator::len).collect());
        table.columns.push(columns.into_iter().map(|c| c.start).collect());
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
/// a row of three cells with the page number in the middle one,
/// "2021 Form 10-K<TAB>95<TAB>WEC Energy Group, Inc.", or a page number with
/// a prefix alone on its line, "FS - 12". Front matter pages may be numbered
/// "i", "ii" and so on.
///
/// Each family of copies, the same text with the page number in the same
/// place, needs five copies, and the page numbers must mostly increase
/// through the document, which data rows do not do. A page numbered in roman
/// numerals followed by one in digits counts as an increase, and so does a
/// "Page 1 of M" after any page, as when each exhibit numbers its own pages
/// (the page must not be past the count, "Page 4 of 3"). The text beside the
/// number needs 10 letters and a space, because short lines such as
/// cross-references ("See page 4") and captions ("Note 4") also come with
/// rising numbers. Fewer letters are enough when the text names the form
/// ("2021 Form 10-K 100") or the line starts with "Page N of M" ("Page 3 of
/// 26"). Any text, even one short word, is enough when a "|" separates the
/// number from it ("ACME | 23"), or when the number is at the start of some
/// copies and at the end of others, at least two of each, as on facing pages
/// ("24 ACME", "ACME 25"): captions do neither. With fewer than 10 letters
/// the copies must also mostly not be on consecutive lines, as the rows of a
/// table are. Rows with the number in the middle must also mostly not be on
/// consecutive lines, whatever their length.
///
/// A page number with a prefix alone on its line (see `PAGE_LABEL`) belongs
/// to the family of its prefix, or of all chapter numbers ("7-3", "7A-1").
/// Its copies must mostly not be on consecutive lines, and must mostly come
/// one to three pages after the copy before (see `page_steps`), as page
/// numbers do and lists of exhibits ("EX-21", "EX-23", "EX-31") do not.
/// Copies with a prefix of letters and no final "." also go, however few,
/// when those that end a printed page (`ends_page`) are numbered 1, 2, 3 and
/// so on, as the pages of signatures ("S-1", "S-2") or of an exhibit index
/// are. Only those copies go, so a reference to a page ("S-1" in an index)
/// stays where it does not end a page, and so does the end of a sentence
/// wrapped onto its own line ("... begins on page" over "E-1."), or the
/// rest of a line that ends with a colon or with a word that such a name
/// follows: Form, Exhibit, Annex, Appendix or page ("... Registration
/// Statement on Form" over "S-1", "... rated by S&P:" over "A-1").
///
/// A running header or footer that goes also takes the lines with its text
/// and a page number with a prefix in place of its page number, as on the
/// pages of an exhibit index ("Bank of America 2013 E-1").
///
/// A page number in parentheses or followed by a period alone on its line
/// ("(12)", "12.") counts only where it ends a printed page (`ends_page`),
/// as the last line before a page break, and its copies must pass the same
/// two tests. Numbered footnotes ("(1)" over the footnote) and notes ("3."
/// over the note's title) look the same, but do not end the pages.
///
/// Lines of table cells (`is_cell`) are data and never count on their own.
/// A short row printed cell by cell (`rows`, as ranges of `lines`) counts as
/// one copy: its lines, with the page number taken out of one of them, must
/// repeat as the text of a line does, as in a footer with the company name
/// over the report name and the page number in the next cell. Its copies
/// must also mostly not be next to each other, as the rows of a table are.
fn page_furniture(lines: &[&str], is_cell: &[bool], rows: &[Range<usize>], ends_page: &[bool]) -> HashSet<usize> {
    const MIN_REPEATS: usize = 5;
    const MIN_LETTERS: usize = 10;
    const MAX_LEN: usize = 120;
    // Filings also spell the dash as a non-breaking hyphen (U+2011) or an en dash.
    static FORM_NAME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b(?:10[-‐‑‒–—][KQ]|8[-‐‑‒–—]K|20[-‐‑‒–—]F|40[-‐‑‒–—]F)").unwrap());
    let letters = |s: &str| s.chars().filter(|c| c.is_alphabetic()).count();
    // Whether at least 90% of `total` copies or steps between copies pass a test.
    let mostly = |count: usize, total: usize| count * 10 >= total * 9;
    let mut drop = HashSet::new();
    // Whether a line is the rest of the line above, which ends with a colon
    // or with a word that the name of a form, an exhibit or a page follows.
    let goes_on = |i: usize| {
        i.checked_sub(1).is_some_and(|above| {
            let above = lines[above];
            let last = above.rsplit(|c: char| !c.is_alphanumeric()).next().unwrap_or("");
            above.ends_with(':')
                || ["form", "exhibit", "annex", "appendix", "page"].iter().any(|w| last.eq_ignore_ascii_case(w))
        })
    };

    // Line indexes and page numbers, keyed as page_numbers keys them.
    let mut families: HashMap<PageKey, Vec<(usize, PageNumber)>> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        if line.len() > MAX_LEN || is_cell[i] {
            continue;
        }
        for page in page_numbers(line).into_iter().flatten() {
            if page.key.2 != Place::Alone || ends_page[i] {
                families.entry(page.key).or_default().push((i, page));
            }
        }
    }
    // The text beside the page number of each running header or footer that
    // goes, for the copies with a prefixed page number in its place.
    let mut texts = HashSet::new();
    for (&(before, after, place), seen) in &families {
        let n = seen.len();
        if place == Place::Label && !before.is_empty() {
            let ending = seen.iter().filter(|&&(i, _)| ends_page[i] && !lines[i].ends_with('.') && !goes_on(i));
            if ending.clone().zip(1..).all(|((_, p), k)| p.number == k) {
                drop.extend(ending.map(|&(i, _)| i));
            }
        }
        if n < MIN_REPEATS {
            continue;
        }
        let apart = mostly(seen.windows(2).filter(|w| w[1].0 > w[0].0 + 1).count(), n - 1);
        let furniture = match place {
            Place::Label | Place::Chapter | Place::Alone => apart && mostly(page_steps(seen), n - 1),
            Place::Edge | Place::PageOf | Place::Middle => {
                let short = letters(before) + letters(after) < MIN_LETTERS;
                let spaced = before.contains(' ') || after.contains(' ');
                let form = FORM_NAME.is_match(before) || FORM_NAME.is_match(after);
                let piped = mostly(seen.iter().filter(|(_, p)| p.piped).count(), n);
                let first = seen.iter().filter(|(_, p)| p.first).count();
                let facing = place == Place::Edge && first >= 2 && n - first >= 2;
                let text = (spaced && (!short || form)) || place == Place::PageOf || piped || facing;
                text && mostly(rising(seen), n - 1) && ((!short && after.is_empty()) || apart)
            }
        };
        if furniture {
            drop.extend(seen.iter().map(|&(i, _)| i));
            if place == Place::Edge {
                texts.insert(before);
            }
        }
    }
    for (i, line) in lines.iter().enumerate() {
        if texts.is_empty() || line.len() > MAX_LEN || is_cell[i] {
            continue;
        }
        let text = LABELED.captures(line).and_then(|c| c.get(1).or(c.get(2)));
        if text.is_some_and(|t| texts.contains(t.as_str().trim_matches(|c: char| c == '|' || c.is_whitespace()))) {
            drop.insert(i);
        }
    }

    // The same for rows printed cell by cell: indexes into `rows` and page
    // numbers, keyed by the lines of the row with the page number taken out.
    let mut row_families: HashMap<String, Vec<(usize, PageNumber)>> = HashMap::new();
    for (r, range) in rows.iter().enumerate() {
        let row = &lines[range.clone()];
        if row.iter().map(|l| l.len() + 1).sum::<usize>() > MAX_LEN {
            continue;
        }
        for (j, line) in row.iter().enumerate() {
            for page in page_numbers(line).into_iter().flatten().filter(|p| p.key.2 != Place::Alone) {
                let (before, after, place) = page.key;
                let mut key = String::new();
                for (k, l) in row.iter().enumerate() {
                    if k > 0 {
                        key.push('\n');
                    }
                    if k == j {
                        key.extend([before, "\0", after]);
                    } else {
                        key.push_str(l);
                    }
                }
                if letters(&key) < MIN_LETTERS && place != Place::PageOf && !FORM_NAME.is_match(&key) {
                    continue;
                }
                row_families.entry(key).or_default().push((r, page));
            }
        }
    }
    for seen in row_families.values() {
        let n = seen.len();
        let apart = seen.windows(2).filter(|w| rows[w[1].0].start > rows[w[0].0].end).count();
        if n >= MIN_REPEATS && mostly(rising(seen), n - 1) && mostly(apart, n - 1) {
            drop.extend(seen.iter().flat_map(|&(r, _)| rows[r].clone()));
        }
    }
    drop
}

/// How many copies of a running header or footer have a higher page number
/// than the copy before, counting a page numbered in roman numerals followed
/// by one in digits, as when the front matter gives way to the body, and a
/// "Page 1 of M" after any page, as when each exhibit numbers its own pages.
fn rising(seen: &[(usize, PageNumber)]) -> usize {
    seen.windows(2)
        .filter(|w| {
            let (a, b) = (&w[0].1, &w[1].1);
            if b.of.is_some() && b.number == 1 {
                true
            } else if a.roman == b.roman {
                b.number > a.number
            } else {
                a.roman
            }
        })
        .count()
}

/// How many copies of a page number with a prefix come one to three pages
/// after the copy before: later in the same chapter, or on one of the first
/// three pages of a later chapter at most three chapter numbers on ("1-8",
/// then "2-1"; "7-9", then "7A-1").
fn page_steps(seen: &[(usize, PageNumber)]) -> usize {
    seen.windows(2)
        .filter(|w| {
            let (a, b) = (&w[0].1, &w[1].1);
            if a.chapter == b.chapter {
                b.number > a.number && b.number - a.number <= 3
            } else {
                b.chapter > a.chapter && b.chapter.0 - a.chapter.0 <= 3 && b.number <= 3
            }
        })
        .count()
}

/// Where a page number sits in its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Place {
    /// Alone, or at the start or end of the line.
    Edge,
    /// In a leading "Page N of M".
    PageOf,
    /// In the middle cell of a row of three cells.
    Middle,
    /// Alone after a prefix of capital letters, or nothing, and a dash: "FS - 12", "- 4".
    Label,
    /// Alone after a chapter number and a dash: "7-3", "7A-1".
    Chapter,
    /// Alone in parentheses or followed by a period: "(12)", "12.".
    Alone,
}

/// The text before and after a page number, and where it sits. A number at
/// the start or end of a line leaves its text in the first place, so that
/// headers with the number on alternate sides of the page are one family. In
/// "Page N of M" the text is what follows M, so that exhibits of different
/// lengths are one family, and a page number with a prefix keeps only the
/// prefix.
type PageKey<'a> = (&'a str, &'a str, Place);

/// A page number found in a line, and the text around it.
struct PageNumber<'a> {
    number: u32,
    /// Written in roman numerals.
    roman: bool,
    /// The chapter of a page numbered as "7-3" or "7A-1", as its number and
    /// letter (0 for none), else (0, 0).
    chapter: (u32, u8),
    /// The page count M in "Page N of M".
    of: Option<u32>,
    /// The number is at the start of the line.
    first: bool,
    /// A "|" separates the number from the text.
    piped: bool,
    key: PageKey<'a>,
}

impl<'a> PageNumber<'a> {
    /// A page number in digits, with nothing else known about it.
    fn new(number: u32, key: PageKey<'a>) -> Self {
        PageNumber { number, roman: false, chapter: (0, 0), of: None, first: false, piped: false, key }
    }
}

/// The page numbers of a line that may be a running header or footer, with
/// the text around them: a bare page number, or one with a prefix alone on
/// the line (`PAGE_LABEL`), or one alone in parentheses or followed by a
/// period ("(12)", "12."), or one at the start or end of the line, in a
/// leading "Page N of M", or in the middle cell of a row of three cells, in
/// digits or, in the front matter, in roman numerals. A line can have both of
/// the last two, as "2021 Annual Report<TAB>61<TAB>Part II, Item 8" does.
fn page_numbers<'a>(line: &'a str) -> [Option<PageNumber<'a>>; 2] {
    // The page number comes first, in a leading "Page N of M", or last, in
    // digits or in lowercase roman numerals.
    static NUMBERED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(concat!(
            r"^(?:(\d{1,3})(?:\s*\|\s*|\s+)(\S.*)",
            r"|(?i:page)\s+(\d{1,3})\s+(?i:of)\s+(\d{1,3})\b(.*)",
            r"|(.*\S)(?:\s*\|\s*|\s+)(\d{1,3})",
            r"|([ivxlc]{1,7})(?:\s*\|\s*|\s+)(\S.*)",
            r"|(.*\S)(?:\s*\|\s*|\s+)([ivxlc]{1,7}))$",
        ))
        .unwrap()
    });
    let none = || [None, None];
    if let Some(c) = PAGE_NUMBER.captures(line) {
        let roman = c.get(2).or(c.get(5)).is_some();
        let Some(m) = (1..7).find_map(|g| c.get(g)) else { return none() };
        let number = if roman { self::roman(m.as_str()) } else { m.as_str().parse().ok() };
        let key = (&line[..m.start()], &line[m.end()..], Place::Edge);
        return [number.map(|number| PageNumber { roman, ..PageNumber::new(number, key) }), None];
    }
    if let Some(c) = PAGE_LABEL.captures(line) {
        let Ok(number) = c[3].parse() else { return none() };
        let page = match c.get(2) {
            Some(chapter) => {
                let chapter = chapter.as_str();
                let digits = chapter.trim_end_matches(|c: char| c.is_ascii_uppercase());
                let letter = chapter.as_bytes().get(digits.len()).copied().unwrap_or(0);
                let chapter = (digits.parse().unwrap_or(0), letter);
                PageNumber { chapter, ..PageNumber::new(number, ("", "", Place::Chapter)) }
            }
            None => PageNumber::new(number, (c.get(1).map_or("", |m| m.as_str()), "", Place::Label)),
        };
        return [Some(page), None];
    }
    let (open, close) = if line.starts_with('(') { ("(", ")") } else { ("", ".") };
    let alone = line.strip_prefix(open).and_then(|rest| rest.strip_suffix(close));
    if let Some(digits) = alone.filter(|d| (1..=3).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit())) {
        return [digits.parse().ok().map(|number| PageNumber::new(number, (open, close, Place::Alone))), None];
    }
    let numeral = |c: char| c.is_ascii_digit() || matches!(c, 'i' | 'v' | 'x' | 'l' | 'c');
    let edge = line.starts_with(numeral)
        || line.ends_with(numeral)
        || line.get(..4).is_some_and(|s| s.eq_ignore_ascii_case("page"));
    let at_edge = edge.then(|| NUMBERED.captures(line)).flatten().and_then(|c| {
        if let (Some(page), Some(of)) = (c.get(3), c.get(4)) {
            let (page, of): (u32, u32) = (page.as_str().parse().ok()?, of.as_str().parse().ok()?);
            let key = (c.get(5).map_or("", |m| m.as_str().trim()), "", Place::PageOf);
            return (page <= of).then(|| PageNumber { of: Some(of), first: true, ..PageNumber::new(page, key) });
        }
        // The number and the text, in each order and spelling.
        let (number, rest, first) = [(1, 2, true), (7, 6, false), (8, 9, true), (11, 10, false)]
            .into_iter()
            .find_map(|(n, r, first)| Some((c.get(n)?, c.get(r)?, first)))?;
        let roman = !number.as_str().starts_with(|c: char| c.is_ascii_digit());
        let value = if roman { self::roman(number.as_str())? } else { number.as_str().parse().ok()? };
        let text = rest.as_str().trim_matches(|c: char| c == '|' || c.is_whitespace());
        // What separates the number from the text.
        let at = text.as_ptr() as usize - line.as_ptr() as usize;
        let between = if first { &line[number.end()..at] } else { &line[at + text.len()..number.start()] };
        let piped = between.contains('|');
        Some(PageNumber { roman, first, piped, ..PageNumber::new(value, (text, "", Place::Edge)) })
    });
    let mut cells = line.split('\t');
    let in_middle = match (cells.next(), cells.next(), cells.next(), cells.next()) {
        (Some(before), Some(number), Some(after), None) => {
            let key = (before, after, Place::Middle);
            if number.len() <= 3 && number.bytes().all(|b| b.is_ascii_digit()) {
                number.parse().ok().map(|n| PageNumber::new(n, key))
            } else {
                roman(number).map(|n| PageNumber { roman: true, ..PageNumber::new(n, key) })
            }
        }
        _ => None,
    };
    [at_edge, in_middle]
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
        let html = "<p>Text</p><p>12</p><p>Table of Contents</p><p>F-3</p><p>- 7 -</p><p>ii</p><p>More</p>\
                    <p>F - 4</p><p>F\u{2011}5</p><p>- -8-</p><p>- ii -</p><p>-iii-</p><p>Page E-1</p><p>End</p>";
        assert_eq!(html_to_text(html), "Text\nMore\nEnd\n");
        // Only a whole line is a page number.
        let html = "<p>- iv - Text</p><p>Page E-1 lists the exhibits.</p><p>See page A-2</p>";
        assert_eq!(html_to_text(html), "- iv - Text\nPage E-1 lists the exhibits.\nSee page A-2\n");
    }

    /// Text of `pages` pages, each "Text of page N." followed by `footer`
    /// with "{}" replaced by that page's number, and the text without the footers.
    fn paged(footer: &str, pages: impl IntoIterator<Item = String>) -> (String, String) {
        let (mut html, mut want) = (String::new(), String::new());
        for page in pages {
            html += &format!("<p>Text of page {page}.</p><p>{}</p>", footer.replace("{}", &page));
            want += &format!("Text of page {page}.\n");
        }
        (html, want)
    }

    fn numbered(pages: std::ops::Range<u32>) -> Vec<String> {
        pages.map(|n| n.to_string()).collect()
    }

    /// Page numbers alone on their line after a letter prefix, a chapter
    /// number or a dash go when five or more of them come a page or a few
    /// apart, with any of the dashes filings use.
    #[test]
    fn page_numbers_with_a_prefix_dropped() {
        for footer in ["FS - {}", "S-{}", "II-{}", "E \u{2013} {}", "K-{}", "F\u{2011}{}.", "- {}"] {
            let (html, want) = paged(footer, numbered(1..7));
            assert_eq!(html_to_text(&html), want, "{footer}");
        }
        // Pages skipped here and there, and chapter-page numbers.
        let (html, want) = paged("FS - {}", ["1", "2", "4", "5", "8", "9"].map(String::from));
        assert_eq!(html_to_text(&html), want);
        let chapters = ["1-1", "1-2", "1-3", "2-1", "2-2", "4-1", "4-2"].map(String::from);
        let (html, want) = paged("{}", chapters);
        assert_eq!(html_to_text(&html), want);
        // Chapters named after Items with a letter, such as Item 7A.
        let chapters = ["7-8", "7-9", "7A-1", "7A-2", "7A-3", "8-1", "9A-1"].map(String::from);
        let (html, want) = paged("{}", chapters);
        assert_eq!(html_to_text(&html), want);
    }

    /// Lines that look like page numbers with a prefix but are lists stay:
    /// exhibit numbers, which jump; ranges; labels on consecutive lines; too
    /// few copies; and numbered captions such as "1." over a note's title.
    #[test]
    fn prefixed_numbers_that_are_not_pages_stay() {
        let mut html = String::new();
        for n in [3, 4, 10, 21, 23, 31, 32] {
            html += &format!("<p>EX-{n}</p><p>Description of exhibit {n}.</p>");
        }
        for range in ["0-3", "4-12", "13-24", "25-36", "37-48", "49-60"] {
            html += &format!("<p>{range}</p><p>Months to maturity.</p>");
        }
        for n in 1..7 {
            html += &format!("<p>S-{n}</p>");
        }
        for n in 1..5 {
            html += &format!("<p>Text.</p><p>A-{n}</p>");
        }
        for label in ["10A-1", "3B-12", "7A-30", "2C-5", "11A-2", "4B-9"] {
            html += &format!("<p>Text.</p><p>{label}</p>");
        }
        for n in 1..9 {
            html += &format!("<p>{n}.</p><p>Title of note {n}</p><p>Text of note {n}.</p>");
        }
        let text = html_to_text(&html);
        for (start, copies) in [("EX-", 7), ("Months", 6), ("S-", 6), ("A-", 4)] {
            assert_eq!(text.lines().filter(|l| l.starts_with(start)).count(), copies, "{start}");
        }
        for label in ["10A-1", "3B-12", "7A-30", "2C-5", "11A-2", "4B-9"] {
            assert!(text.contains(&format!("\n{label}\n")), "{label}");
        }
        for range in ["0-3", "4-12", "13-24", "25-36", "37-48", "49-60"] {
            assert!(text.contains(&format!("\n{range}\n")), "{range}");
        }
        assert_eq!(text.lines().filter(|l| l.len() <= 2 && l.ends_with('.')).count(), 8);
    }

    /// Running headers numbered in roman numerals at the start or end of the
    /// line in the front matter go with the rest of their family.
    #[test]
    fn running_headers_with_roman_page_numbers_dropped() {
        let pages = ["i", "ii", "iii", "iv", "1", "2", "3"].map(String::from);
        for header in ["Acme Corporation Annual Report {}", "{} | Acme Corporation Annual Report"] {
            let (html, want) = paged(header, pages.clone());
            assert_eq!(html_to_text(&html), want, "{header}");
        }
        // Roman numerals beside fewer than 10 letters are captions, and ones
        // that do not rise are check marks.
        let mut html = String::new();
        for n in ["i", "ii", "iii", "iv", "v", "vi"] {
            html += &format!("<p>Proposal {n}</p><p>Text of the proposal.</p>");
            html += "<p>Indicate by check mark whether the registrant is a shell company. Yes ¨ No x</p><p>Text.</p>";
        }
        let text = html_to_text(&html);
        assert_eq!(text.matches("Proposal").count(), 6);
        assert_eq!(text.matches("No x").count(), 6);
    }

    /// Running headers of one short word that does not name the form go when
    /// a "|" separates the page number, or when the number is at the start of
    /// some copies and at the end of others, as on facing pages. Otherwise
    /// they cannot be told from numbered captions such as "Note 4", and stay.
    #[test]
    fn short_running_headers_dropped_when_their_layout_shows_it() {
        for header in ["ACME | {}", "{} | ACME"] {
            let (html, want) = paged(header, numbered(20..26));
            assert_eq!(html_to_text(&html), want, "{header}");
        }
        let mut html = String::new();
        let mut want = String::new();
        for page in 20..26 {
            let header = if page % 2 == 0 { format!("{page} ACME") } else { format!("ACME {page}") };
            html += &format!("<p>Text of page {page}.</p><p>{header}</p>");
            want += &format!("Text of page {page}.\n");
        }
        assert_eq!(html_to_text(&html), want);

        // One side only, or one copy on the other side.
        let mut html = String::new();
        for n in 1..8 {
            html += &format!("<p>Note {n}</p><p>Text of note {n}.</p><p>ACME {}</p>", n + 20);
        }
        html += "<p>Text.</p><p>28 ACME</p>";
        let text = html_to_text(&html);
        assert_eq!(text.lines().filter(|l| l.starts_with("Note ")).count(), 7);
        assert_eq!(text.matches("ACME").count(), 8);
    }

    /// "Page N of M" footers that start again at page 1 in each exhibit go,
    /// while other lines whose numbers start again stay.
    #[test]
    fn page_of_footers_that_restart_in_each_exhibit_dropped() {
        let pages = ["1 of 3", "2 of 3", "3 of 3", "1 of 2", "2 of 2", "1 of 1", "1 of 4", "2 of 4"].map(String::from);
        let (html, want) = paged("Page {}", pages);
        assert_eq!(html_to_text(&html), want);

        // Numbers that start again at 1 outside "Page N of M", and "Page N of
        // M" that goes back to a page other than 1.
        let mut html = String::new();
        for n in [1, 2, 3, 1, 2, 1, 2, 3] {
            html += &format!("<p>Supplemental Indenture No. {n}</p><p>Description of the indenture.</p>");
            html += &format!("<p>Page {} of 4</p><p>Text.</p>", n + 1);
        }
        let text = html_to_text(&html);
        assert_eq!(text.matches("Supplemental Indenture No.").count(), 8);
        assert_eq!(text.matches(" of 4").count(), 8);

        // Pages past the count are not page numbers, so too few copies are left.
        let (html, _) = paged("Page {} of 2", numbered(1..9));
        assert_eq!(html_to_text(&html).matches(" of 2").count(), 8);
    }

    /// Page numbers alone on their line in parentheses or followed by a
    /// period go when each ends a printed page: before a page break that a
    /// style sets, also inside the cell of a layout table, or before the page
    /// tag of a text filing.
    #[test]
    fn page_numbers_ending_printed_pages_dropped() {
        let want: String = (1..7).map(|n| format!("Text of page {n}.\n")).collect();
        for page in [
            "<p>Text of page {}.</p><div style=\"PAGE-BREAK-AFTER: always\"><p>({})</p><hr></div>",
            "<p>Text of page {}.</p><p>{}.</p><div style=\"break-after: page\"></div>",
            "<p>Text of page {}.</p><p>({})</p><p style=\"page-break-before:always\"></p>",
        ] {
            let pages: String = (1..7).map(|n| page.replace("{}", &n.to_string())).collect();
            assert_eq!(tables_of(&pages).0, want, "{page}");
            let layout = format!("<table><tr><td>{pages}</td></tr></table>");
            assert_eq!(tables_of(&layout).0, want, "{page} in a layout table");
        }
        let pages: String = (1..7).map(|n| format!("Text of page {n}.\n\n{:>40}.\n<PAGE>\n", n)).collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{pages}</TEXT>")), want);
    }

    /// Lines that are only a number in parentheses or followed by a period
    /// stay when they do not end the printed pages, as footnote markers over
    /// their footnotes and numbered notes that each start a page do, or when
    /// too few end pages, or their numbers do not step like pages, or they
    /// are cells of a table row.
    #[test]
    fn numbers_alone_that_do_not_end_pages_stay() {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        let new_page = "<div style=\"page-break-before: always\">";
        let row = "<table><tr><td>Claims<br>resolved</td>";
        let cases = [
            ((1..9).map(|n| format!("<p>({n})</p><p>Text of footnote {n}.</p>")).collect::<String>(), "(", 8),
            ((1..9).map(|n| format!("{new_page}<p>{n}.</p><p>Title of note {n}</p></div>")).collect(), ".", 8),
            ((20..24).map(|n| format!("<p>Text.</p><p>({n})</p>{page_break}")).collect(), "(", 4),
            ([12, 9, 14, 3, 20, 1].map(|n| format!("<p>Text.</p><p>{n}.</p>{page_break}")).concat(), ".", 6),
            ((30..36).map(|n| format!("{row}<td>({n})</td></tr></table>{page_break}")).collect(), "(", 6),
        ];
        for (html, mark, copies) in cases {
            let (text, _) = tables_of(&html);
            let alone = |l: &&str| l.len() <= 4 && (l.starts_with('(') || l.ends_with('.')) && l.contains(mark);
            assert_eq!(text.lines().filter(alone).count(), copies, "{html}");
        }
        // A numbered list in a text filing, with the page tag after the last item's text.
        let list: String = (1..7).map(|n| format!("{n}.\nItem {n} of the list.\n")).collect();
        let text = document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{list}<PAGE>\nMore.\n</TEXT>"));
        assert_eq!(text, format!("{list}More.\n"));
    }

    /// The few pages of signatures, schedules or an exhibit index numbered
    /// "S-1", "S-2", "E-1" and so on lose their page numbers when the copies
    /// that end printed pages count up from 1, also on the last line of the
    /// document and in text filings.
    #[test]
    fn page_numbers_with_a_prefix_on_a_few_last_pages_dropped() {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        let html = format!(
            "<p>Text.</p>{page_break}<p>Signatures.</p><p>S-1</p>{page_break}<p>More signatures.</p><p>S - 2</p>\
             {page_break}<p>Exhibit index.</p><p>E-1</p>{page_break}<p>More exhibits.</p><p>E-2</p>"
        );
        assert_eq!(tables_of(&html).0, "Text.\nSignatures.\nMore signatures.\nExhibit index.\nMore exhibits.\n");
        let raw =
            "<DOCUMENT>\n<TEXT>\nText.\n<PAGE>\nSignatures.\n\n                S-1\n<PAGE>\nIndex.\n   E-1\n</TEXT>";
        assert_eq!(document_to_text(raw), "Text.\nSignatures.\nIndex.\n");
        // A reference to page S-1 in an index stays, while the page's own number goes.
        let html = format!("<p>Schedule II</p><p>S-1</p><p>Text.</p>{page_break}<p>Schedule II.</p><p>S-1</p>");
        assert_eq!(tables_of(&html).0, "Schedule II\nS-1\nText.\nSchedule II.\n");
        // A form's name wrapped just before a page tag of a text filing stays.
        let raw = "<DOCUMENT>\n<TEXT>\nText.\n<PAGE>\nEXHIBIT INDEX\n3.1  Articles, incorporated by reference to \
                   the\n     Registration Statement on Form\n     S-1\n<PAGE>\n     (File No. 33-12345).\n</TEXT>";
        assert_eq!(
            document_to_text(raw),
            "Text.\nEXHIBIT INDEX\n3.1  Articles, incorporated by reference to the\nRegistration Statement on Form\nS-1\n\
             (File No. 33-12345).\n"
        );
    }

    /// Few lines that look like the page numbers of the last pages stay when
    /// they do not end pages, when those that do skip a number or do not
    /// start at 1, when they end a sentence, when the document has no page
    /// breaks, when they have no prefix of letters, or when they are cells.
    #[test]
    fn few_numbers_with_a_prefix_that_do_not_count_pages_stay() {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        let cases = [
            "<p>Exhibits</p><p>E-1</p><p>E-2</p><p>Text.</p>".to_string(),
            format!("<p>Text.</p><p>E-4</p>{page_break}<p>Text.</p>"),
            format!("<p>Text.</p><p>S-1</p>{page_break}<p>Text.</p><p>S-3</p>{page_break}<p>Text.</p>"),
            format!("<p>The index begins on page</p><p>E-1.</p>{page_break}<p>Text.</p>"),
            "<p>Signatures.</p><p>S-1</p>".to_string(),
            format!("<p>Text.</p><p>- 1</p>{page_break}<p>Text.</p><p>- 2</p>{page_break}<p>Text.</p>"),
            format!("<p>Text.</p><p>1-1</p>{page_break}<p>Text.</p><p>1-2</p>{page_break}<p>Text.</p>"),
            format!("<table><tr><td>Schedule<br>II</td><td>S-1</td></tr></table>{page_break}<p>Text.</p>"),
            // The rest of a line that ends with a colon or a word that such a
            // name follows, also on the last line of the document.
            format!("<p>Text.</p>{page_break}<p>Our commercial paper is rated by S&amp;P:</p><p>A-1</p>"),
            format!("<p>Filed as Exhibit</p><p>A-1</p>{page_break}<p>to the agreement.</p>"),
            format!("<p>Text.</p>{page_break}<p>The plan is filed as Appendix</p><p>B-1</p>"),
        ];
        for html in cases {
            // Every line of the input stays.
            let all: String = Html::parse_fragment(&html).root_element().text().map(|t| format!("{t}\n")).collect();
            assert_eq!(tables_of(&html).0, all, "{html}");
        }
    }

    /// Old text filings that number their page tags, as in "<Page 2>".
    #[test]
    fn numbered_page_tags_dropped() {
        let raw = "<DOCUMENT>\n<TEXT>\nPart I\n<Page  2>\nItem 1. Business\n<PAGE>\nText.\n<Page 3>\n</TEXT>";
        assert_eq!(document_to_text(raw), "Part I\nItem 1. Business\nText.\n");
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

    /// The pages of an exhibit index that print the running footer with a
    /// page number with a prefix lose it with the rest of the family, as a
    /// line or as a row of cells, on either side of the text.
    #[test]
    fn running_footers_with_a_prefixed_page_number_dropped() {
        for cell_by_cell in [false, true] {
            let mut html = String::new();
            let mut want = String::new();
            let pages = (20..26).map(|n| n.to_string()).chain(["E-1", "E-2", "II-1", "7A-1"].map(String::from));
            for (i, page) in pages.enumerate() {
                let cells =
                    if i % 2 == 0 { ["Acme Corporation 2013", &page] } else { [&page, "Acme Corporation 2013"] };
                let footer = match cell_by_cell {
                    false => format!("<p>{}</p>", cells.join(" ")),
                    true => format!("<table><tr><td>{}</td><td>{}</td></tr></table>", cells[0], cells[1]),
                };
                html += &format!("<p>Text of page {page}.</p>{footer}");
                want += &format!("Text of page {page}.\n");
            }
            let (text, tables) = tables_of(&html);
            assert_eq!(text, want, "{html}");
            assert!(tables.is_empty(), "{tables:?}");
        }

        // They stay when the lines with the same text and a number do not go
        // (the numbers do not count up like pages), when the text differs, and
        // in a cell of a row printed cell by cell.
        let mut html = String::new();
        for n in [12, 9, 14, 3, 20, 1] {
            html += &format!("<p>Text.</p><p>Acme Corporation 2013 {n}</p>");
        }
        html += "<p>Text.</p><p>Acme Corporation 2013 E-1</p>";
        for page in 30..36 {
            html += &format!("<p>Text of page {page}.</p><p>Acme Inc. Annual Report {page}</p>");
        }
        html += "<p>Text.</p><p>Acme Inc. Annual Report 2014 E-1</p><p>Exhibit 10 to Acme Inc. Annual Report E-1</p>";
        html += "<table><tr><td>Acme Inc. Annual Report E-2<br>Exhibit 10</td><td>Filed herewith</td></tr></table>";
        let text = html_to_text(&html);
        for line in [
            "Acme Corporation 2013 E-1",
            "Acme Inc. Annual Report 2014 E-1",
            "Exhibit 10 to Acme Inc. Annual Report E-1",
            "Acme Inc. Annual Report E-2",
        ] {
            assert!(text.contains(&format!("{line}\n")), "{line}");
        }
        assert!(!text.contains("Acme Inc. Annual Report 3"));
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

    /// A row of three cells with the page number in the middle one is a
    /// footer even when its last cell ends in a number, and footers numbered
    /// "i", "ii" and so on before the body's "1" rise like any others.
    #[test]
    fn running_footers_with_the_page_number_in_the_middle_ending_in_a_number_dropped() {
        let numbers = |pages: &[&str]| pages.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        for (footer, pages) in [
            ("2021 Annual Report</td><td>{}</td><td>Part II, Item 8", numbers(&["61", "62", "63", "64", "65", "66"])),
            ("2021 Form 10-K</td><td>{}</td><td>WEC Energy Group, Inc.", numbers(&["i", "ii", "iii", "1", "2", "3"])),
        ] {
            let mut html = String::new();
            let mut want = String::new();
            for page in pages {
                html += &format!(
                    "<p>Text of page {page}.</p><table><tr><td>{}</td></tr></table>",
                    footer.replace("{}", &page)
                );
                want += &format!("Text of page {page}.\n");
            }
            let (text, tables) = tables_of(&html);
            assert_eq!(text, want, "{footer}");
            assert!(tables.is_empty(), "{tables:?}");
        }
    }

    /// A running footer printed as a row of cells, one of them of several
    /// lines, goes as a whole, as one printed as a line of cells does. Rows
    /// printed cell by cell whose numbers do not rise like pages, or which
    /// follow each other in one table, stay.
    #[test]
    fn running_footers_printed_cell_by_cell_dropped() {
        for footer in [
            "<td>Acme Corporation Annual Report</td><td>{}</td>",
            "<td><p>Acme Corporation</p><p>2021 Annual Report</p></td><td>{}</td>",
            "<td>{}</td><td>Acme Corporation<br>2021 Annual Report</td><td>Part II</td>",
            "<td><p>Apple Inc.</p><p>2026 Form 10-K | {}</p></td><td>Investor Relations</td>",
            "<td>Acme<br>Annual Report</td><td>Page {}</td>",
        ] {
            let mut html = String::new();
            let mut want = String::new();
            for page in 20..26 {
                html += &format!(
                    "<p>Text of page {page}.</p><table><tr>{}</tr></table>",
                    footer.replace("{}", &page.to_string())
                );
                want += &format!("Text of page {page}.\n");
            }
            let (text, tables) = tables_of(&html);
            assert_eq!(text, want, "{footer}");
            assert!(tables.is_empty(), "{tables:?}");
        }

        let mut html = String::new();
        for n in [12, 9, 14, 3, 20, 1] {
            html +=
                &format!("<p>Text.</p><table><tr><td>Acme Corporation<br>Store count</td><td>{n}</td></tr></table>");
        }
        html += "<table>";
        for n in 1..7 {
            html += &format!("<tr><td>Acme Corporation<br>Store count</td><td>{n}</td></tr>");
        }
        html += "</table>";
        let text = html_to_text(&html);
        assert_eq!(text.matches("Acme Corporation\nStore count\n").count(), 12);
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
            // A grid column and a span for each cell, in order.
            assert_eq!(
                t.columns.iter().map(Vec::len).collect::<Vec<_>>(),
                t.rows.iter().map(Vec::len).collect::<Vec<_>>()
            );
            assert_eq!(
                t.spans.iter().map(Vec::len).collect::<Vec<_>>(),
                t.rows.iter().map(Vec::len).collect::<Vec<_>>()
            );
            for (columns, spans) in t.columns.iter().zip(&t.spans) {
                assert!(spans.iter().all(|&s| s >= 1), "{t:?}");
                assert!(columns.windows(2).enumerate().all(|(k, w)| w[0] + spans[k] <= w[1]), "{t:?}");
            }
        }
        (text, tables)
    }

    fn rows(t: &Table) -> Vec<Vec<&str>> {
        t.rows.iter().map(|r| r.iter().map(String::as_str).collect()).collect()
    }

    /// Cells keep the grid columns that colspan and rowspan give them, with
    /// empty and spanned-over columns counted and a `display:none` cell
    /// taking none, so values line up with their headers. A cell merged from
    /// `$`, its number and `%` takes the column of its number, and `$` beside
    /// "N/A" keeps its own.
    #[test]
    fn cells_keep_their_grid_columns() {
        let html = r#"<table>
            <tr><td rowspan="2">Segment</td><td colspan="3">Year ended December 31,</td><td></td>
                <td colspan="3">Change</td></tr>
            <tr><td colspan="2">2024</td><td>2023</td><td></td><td>Amount</td><td colspan="2">Percent</td></tr>
            <tr><td>Americas</td><td>$</td><td>167,045</td><td>162,560</td><td></td><td>4,485</td>
                <td>2.8</td><td>%</td></tr>
            <tr><td>Europe</td><td rowspan="2">$</td><td>98,000</td><td>95,000</td><td></td>
                <td colspan="2">3,000</td><td>%</td></tr>
            <tr><td>Asia</td><td>12,000</td><td style="display: none">hidden</td><td>11,000</td><td></td>
                <td>$</td><td>N/A</td></tr>
        </table>"#;
        let (text, tables) = tables_of(html);
        let [t] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(
            rows(t),
            [
                vec!["Segment", "Year ended December 31,", "Change"],
                vec!["2024", "2023", "Amount", "Percent"],
                vec!["Americas", "$167,045", "162,560", "4,485", "2.8%"],
                vec!["Europe", "$98,000", "95,000", "3,000%"],
                vec!["Asia", "12,000", "11,000", "$", "N/A"],
            ]
        );
        assert_eq!(
            t.columns,
            [vec![0, 1, 5], vec![1, 3, 5, 6], vec![0, 2, 3, 5, 6], vec![0, 2, 3, 5], vec![0, 2, 3, 5, 6]]
        );
        assert_eq!(t.spans, [vec![1, 3, 3], vec![2, 1, 1, 2], vec![1; 5], vec![1, 1, 1, 2], vec![1; 5]]);
        // The text does not change.
        assert!(text.contains("\nAmericas\t$167,045\t162,560\t4,485\t2.8%\n"));

        // Attributes as browsers read them: leading digits, and 1 for 0 or none.
        let html = r#"<table><tr><td colspan=" 2px">A</td><td colspan="0">B</td><td colspan="x">C</td>
            <td rowspan="0">D</td><td colspan="+2">E</td><td>F</td></tr>
            <tr><td>G</td><td>H</td><td>I</td><td>J</td><td>K</td></tr></table>"#;
        let (_, tables) = tables_of(html);
        assert_eq!(tables[0].columns, [vec![0, 2, 3, 4, 5, 7], vec![0, 1, 2, 3, 4]]);
        assert_eq!(tables[0].spans, [vec![2, 1, 1, 1, 2, 1], vec![1; 5]]);
        // A cell spanning rows to the left of one that already does.
        let html = r#"<table><tr><td>A</td><td rowspan="3">B</td></tr><tr><td rowspan="2">C</td><td>D</td></tr>
            <tr><td>E</td><td>F</td></tr></table>"#;
        let (_, tables) = tables_of(html);
        assert_eq!(tables[0].columns, [[0, 1], [0, 2], [2, 3]]);
        // Spans beyond what browsers allow, even past the largest integer, are cut to it.
        let html = r#"<table><tr><td colspan="99999999999999999999999">A</td><td colspan="1200">B</td>
            <td>C</td></tr></table>"#;
        let (_, tables) = tables_of(html);
        assert_eq!(tables[0].columns, [[0, 1000, 2000]]);
        assert_eq!(tables[0].spans, [[1000, 1000, 1]]);
    }

    /// Rows that each start with a cell spanning every row below take
    /// columns further and further right, up to the limit for spanning cells,
    /// and the layout stays fast.
    #[test]
    fn rowspans_without_end_stay_fast() {
        let html = format!("<table>{}</table>", r#"<tr><td rowspan="65534">x</td><td>y</td></tr>"#.repeat(3000));
        let (_, tables) = tables_of(&html);
        let [t] = &tables[..] else { panic!("{} tables", tables.len()) };
        assert_eq!(t.rows.len(), 3000);
        for (k, columns) in t.columns.iter().enumerate() {
            let first = k.min(MAX_SPANNING_COLUMN);
            assert_eq!(columns, &[first, first + 1], "row {k}");
        }
    }

    /// Rowspans count within their row group, and a cell dropped from a row
    /// with its text takes its column with it.
    #[test]
    fn grid_columns_follow_row_groups_and_dropped_cells() {
        let html = "<table><thead><tr><td rowspan=\"3\">Name</td><td>Title</td></tr></thead>\
                    <tbody><tr><td>Jane Doe</td><td>Chair</td></tr>\
                    <tr><td>Table of Contents</td><td>Net<br>sales</td><td>5</td></tr></tbody></table>";
        let (_, tables) = tables_of(html);
        let [t] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(rows(t), [vec!["Name", "Title"], vec!["Jane Doe", "Chair"], vec!["Net\nsales", "5"]]);
        assert_eq!(t.columns, [[0, 1], [0, 1], [1, 2]]);
    }

    /// Only `td` and `th` cells take grid columns. The parser leaves a hidden
    /// `<input>` and an empty `<form>` inside a row; neither moves the cells
    /// after it, and the text does not change.
    #[test]
    fn grid_columns_count_only_cells() {
        for extra in [r#"<input type="hidden" name="x">"#, r#"<form action="/x">"#, "<form><input type=hidden>"] {
            let html = format!(
                "<table><tr><td>Name</td><td>2024</td></tr>\
                 <tr>{extra}<td>Sales</td><td>5</td></tr>\
                 <tr><td>Cost</td>{extra}<td colspan=\"2\">3</td></tr></table>"
            );
            let (text, tables) = tables_of(&html);
            let [t] = &tables[..] else { panic!("{tables:?}") };
            assert_eq!(rows(t), [["Name", "2024"], ["Sales", "5"], ["Cost", "3"]], "{extra}");
            assert_eq!(t.columns, [[0, 1], [0, 1], [0, 1]], "{extra}");
            assert_eq!(t.spans, [[1, 1], [1, 1], [1, 2]], "{extra}");
            assert_eq!(text, "Name\t2024\nSales\t5\nCost\t3\n", "{extra}");
        }
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
        // The empty first cell takes column 0, and merged cells the column of their number.
        assert_eq!(t.columns, [vec![1, 2], vec![0, 2, 4], vec![0, 1, 3]]);
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

    /// `$` joins a cell that starts like a number, and `)` or `%` one that
    /// ends like a number (or, for `)`, leaves a "(" open), so header cells
    /// beside a "%" column keep their places, in rows printed as a line and
    /// in rows printed cell by cell.
    #[test]
    fn currency_and_percent_cells_merge_only_into_numbers() {
        let html = "<table><tr><td>Amount</td><td>%</td><td>Amount</td><td>%</td></tr>\
                    <tr><td>Insurance</td><td>$</td><td>1,234</td><td>12.5</td><td>%</td>\
                    <td>$</td><td>(56</td><td>)</td><td>(0.4</td><td>)%</td></tr>\
                    <tr><td>Other</td><td>$</td><td>N/A</td><td>+20</td><td>%</td>\
                    <td>AA (3rd of 21</td><td>)</td><td>$</td><td>—</td><td>Total</td><td>%</td></tr></table>\
                    <table><tr><td>(in millions)</td><td>Quarter ended Sept. 30,</td><td>%</td>\
                    <td>Nine months<br>ended Sept. 30,</td><td>%</td></tr>\
                    <tr><td>Revenue</td><td>12</td><td>5</td></tr></table>";
        let (text, tables) = tables_of(html);
        let [t, header] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(
            rows(t),
            [
                vec!["Amount", "%", "Amount", "%"],
                vec!["Insurance", "$1,234", "12.5%", "$(56)", "(0.4)%"],
                vec!["Other", "$", "N/A", "+20%", "AA (3rd of 21)", "$—", "Total", "%"]
            ]
        );
        assert_eq!(
            rows(header),
            [
                vec!["(in millions)", "Quarter ended Sept. 30,", "%", "Nine months\nended Sept. 30,", "%"],
                vec!["Revenue", "12", "5"]
            ]
        );
        assert_eq!(
            &text[header.start..],
            "(in millions)\nQuarter ended Sept. 30,\n%\nNine months\nended Sept. 30,\n%\nRevenue\t12\t5\n"
        );
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
        assert_eq!(outer.columns, [[0, 1], [0, 1], [0, 1]]);
        assert_eq!(inner.columns, [[0, 1], [0, 1]]);

        // Each table lays out its own grid: a rowspan inside the inner table
        // moves only its own cells, and the outer cell keeps its colspan.
        let html = "<table><tr><td colspan=\"2\">Detail</td><td><table>\
                    <tr><td rowspan=\"2\">North</td><td>East</td></tr><tr><td>West</td></tr>\
                    <tr><td>South</td><td>Central</td></tr></table></td></tr>\
                    <tr><td>Sum</td><td>Total</td><td>9</td></tr></table>";
        let (_, tables) = tables_of(html);
        let [outer, inner] = &tables[..] else { panic!("{tables:?}") };
        assert_eq!(rows(inner), [vec!["North", "East"], vec!["West"], vec!["South", "Central"]]);
        assert_eq!(inner.columns, [vec![0, 1], vec![1], vec![0, 1]]);
        assert_eq!(rows(outer)[1], ["Sum", "Total", "9"]);
        assert_eq!(outer.columns, [vec![0, 2], vec![0, 1, 2]]);
        assert_eq!(outer.spans, [vec![2, 1], vec![1, 1, 1]]);

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
