//! Filing documents to clean plain text.
//!
//! One line per block (paragraph, heading, list item, table row). Table cells
//! are joined with tabs, with `$` and `)`/`%` cells merged into their numbers.
//! Text inside `<pre>` keeps its own lines and the spaces inside them, as a
//! plain-text document does.
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
    out.typed_page_numbers();
    join_lines(&kept_lines(
        &out.buf,
        &out.cell_lines,
        &out.cell_rows,
        &data_rows(&out.table_rows),
        &out.breaks,
        &out.typed_breaks,
    ))
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
    out.typed_page_numbers();
    let lines = kept_lines(
        &out.buf,
        &out.cell_lines,
        &out.cell_rows,
        &data_rows(&out.table_rows),
        &out.breaks,
        &out.typed_breaks,
    );
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
    join_lines(&kept_lines(&text, &[], &[], &[], &breaks, &[]))
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
    /// one line), and of the lines of a `<pre>` element in which no page ends
    /// (see `Out::typed_data`), in order. These lines are table data, never
    /// page numbers or page furniture on their own.
    cell_lines: Vec<usize>,
    /// Where in `buf` each row printed cell by cell with two or more cells and
    /// at most 1,000 characters is, in order. Such a row is removed as a whole
    /// when it repeats on several pages as a running header or footer.
    cell_rows: Vec<Range<usize>>,
    /// Where in `buf` each table row of at most 1,000 characters is, in any
    /// order, with its table and whether it has two or more cells with text
    /// (see `data_rows`).
    table_rows: Vec<(Range<usize>, NodeId, bool)>,
    /// Where in `buf` a printed page ends, in order: before or after each
    /// element styled to break the page there, and at each `<PAGE>` tag
    /// inside `<pre>`.
    breaks: Vec<usize>,
    /// The `breaks` at `<PAGE>` tags inside `<pre>`, in order.
    typed_breaks: Vec<usize>,
    /// Table rows rendered into `buf`, recorded only when tables are wanted.
    rows: Option<Vec<RowRecord>>,
    /// For each row group (`<tbody>`, `<thead>` or `<tfoot>`) whose rows are
    /// recorded, the cells of the rows so far that span rows below: the grid
    /// columns they cover and how many more rows they cover.
    spanning: HashMap<NodeId, Vec<(Range<usize>, usize)>>,
    /// How many `<pre>` elements the text is rendered inside (see `render`).
    pre: usize,
    /// Start in `buf` of each bare number that ends a `<pre>` element in
    /// which no page ends, outside table cells, in order: a page number or
    /// table data (see `Out::typed_page_numbers`).
    typed_numbers: Vec<usize>,
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

    /// Typewriter text, the text inside `<pre>`: a line break ends the line,
    /// and every space and tab inside a line stays, as in a plain-text
    /// filing. The spaces at the start of a line are left out, as the lines
    /// are trimmed later anyway.
    fn typed(&mut self, s: &str) {
        for c in s.chars() {
            match c {
                '\u{200b}' | '\u{ad}' | '\u{feff}' => {}
                '\n' => self.newline(),
                _ if !self.mid_line && c.is_whitespace() => {}
                '\t' => self.buf.push('\t'),
                c if c.is_whitespace() => self.buf.push(' '),
                c => {
                    self.buf.push(c);
                    self.mid_line = true;
                }
            }
        }
    }

    /// The lines of a `<pre>` element in which no page ends, whose text
    /// starts at `start` in `buf` (at the start of a line), in place of the
    /// `cell_lines` after the first `recorded`, which are lines of the rows
    /// inside it. Such an element most often holds a table typed in columns,
    /// and its lines are table data (see `cell_lines`). One of a single line
    /// is an ordinary line, as a paragraph's. A bare number on the last line,
    /// or the only one, may be the page number at the foot of a page, as in
    /// filings that type each page number in a `<pre>` of its own: outside
    /// table cells it waits in `typed_numbers` until the document is
    /// rendered, inside them it is table data.
    fn typed_data(&mut self, start: usize, recorded: usize) {
        let end = self.buf.len();
        let breaks = self.buf[start..].match_indices('\n').map(|(i, _)| start + i + 1);
        let mut lines: Vec<usize> = std::iter::once(start).chain(breaks).filter(|&s| s < end).collect();
        self.cell_lines.truncate(recorded);
        let Some(last) = lines.pop() else { return };
        let several = !lines.is_empty();
        self.cell_lines.extend(lines);
        if PAGE_NUMBER.is_match(self.buf[last..].trim()) {
            if self.depth == 0 {
                self.typed_numbers.push(last);
            } else {
                self.cell_lines.push(last);
            }
        } else if several {
            self.cell_lines.push(last);
        }
    }

    /// Settle the `typed_numbers` of a rendered document: a number is a page
    /// number, and goes as other bare page numbers do, when a page break
    /// follows it, or when the number before it or after it among them
    /// counts the pages with it, written the same way and one to three
    /// pages apart, as "9" and "10" do. Any other is table data, as "790"
    /// after "10" in a list of amounts.
    fn typed_page_numbers(&mut self) {
        if self.typed_numbers.is_empty() {
            return;
        }
        let line = |start: usize| self.buf[start..].lines().next().unwrap_or_default().trim();
        let numbers: Vec<Option<(usize, u32)>> = self.typed_numbers.iter().map(|&s| page_value(line(s))).collect();
        let counts = |a: Option<(usize, u32)>, b: Option<(usize, u32)>| match (a, b) {
            (Some((wa, va)), Some((wb, vb))) => wa == wb && vb > va && vb - va <= 3,
            _ => false,
        };
        let mut data = Vec::new();
        for (i, &start) in self.typed_numbers.iter().enumerate() {
            let before = i > 0 && counts(numbers[i - 1], numbers[i]);
            let after = i + 1 < numbers.len() && counts(numbers[i], numbers[i + 1]);
            let end = start + self.buf[start..].find('\n').unwrap_or(self.buf.len() - start);
            let at_break = self
                .breaks
                .get(self.breaks.partition_point(|&b| b < end))
                .is_some_and(|&b| self.buf[end..b].trim().is_empty());
            if !(before || after || at_break) {
                data.push(start);
            }
        }
        if !data.is_empty() {
            self.cell_lines.extend(data);
            self.cell_lines.sort_unstable();
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
///
/// The text inside `<pre>` is typewriter text (see `Out::typed`), whatever
/// elements it is in. Some filings are a text filing inside
/// `<html><body><pre>`: a `<PAGE>` tag there ends a printed page as it does
/// in a plain-text filing, and the lines of a `<pre>` element in which a
/// page ends are cleaned as a text filing's are. Any other `<pre>` element
/// is part of a page, often a table typed in columns, and its lines are
/// kept as the lines of table cells are, never page numbers or running
/// headers on their own, but for one of a single line and a page number on
/// the last line (see `Out::typed_data`). The cells of a table row are
/// rendered on their own (see `render_row`), so a row inside `<pre>` is an
/// ordinary row, as in a browser, where a table in a document without a
/// doctype does not take the white space of the `<pre>` around it.
fn render(root: NodeRef<Node>, out: &mut Out) {
    enum Step<'a> {
        Enter(NodeRef<'a, Node>),
        Exit,
        /// The end of an element styled to break the page after it.
        Break,
        /// The end of a `<pre>` element, with, for one not inside another,
        /// where its text starts in `buf` and how many `cell_lines` and
        /// `breaks` there were then.
        EndPre(Option<(usize, usize, usize)>),
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
            Step::EndPre(outer) => {
                out.pre -= 1;
                if let Some((start, lines, breaks)) = outer
                    && out.breaks.len() == breaks
                {
                    out.typed_data(start, lines);
                }
                continue;
            }
            Step::Enter(node) => node,
        };
        match node.value() {
            Node::Text(t) if out.pre > 0 => out.typed(t),
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
                        if name == "pre" {
                            let outer =
                                (out.pre == 0).then_some((out.buf.len(), out.cell_lines.len(), out.breaks.len()));
                            out.pre += 1;
                            stack.push(Step::EndPre(outer));
                        } else if name == "page" && out.pre > 0 {
                            out.breaks.push(out.buf.len());
                            out.typed_breaks.push(out.buf.len());
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
                out.typed_breaks.extend(cell.out.typed_breaks.iter().map(|&p| moved(within(p))));
                if !all_lines {
                    out.cell_lines.extend(cell.out.cell_lines.iter().map(|&p| moved(p)));
                    out.cell_rows.extend(cell.out.cell_rows.iter().map(|r| moved(r.start)..moved(r.end)));
                }
                out.table_rows.extend(
                    cell.out.table_rows.iter().map(|(r, t, n)| (moved(within(r.start))..moved(within(r.end)), *t, *n)),
                );
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
    if short {
        let table = enclosing_table(tr).unwrap_or(tr.id());
        out.table_rows.push((row_start..out.buf.len(), table, cells.len() > 1));
    }
    if let (Some(rows), Some(table)) = (&mut out.rows, enclosing_table(tr)) {
        let in_thead = tr.parent().and_then(|p| p.value().as_element().map(|e| e.name() == "thead"));
        let header = in_thead == Some(true) || cells.iter().all(|c| c.th);
        rows.push(RowRecord { table, header, line: data_row, cells: ranges, columns: grid });
    }
    out.has_table = true;
    out.newline();
}

/// The rows of `table_rows` (see `Out::table_rows`) that hold table data: the
/// rows of tables with a row of two or more cells. A table whose rows have
/// one cell each only lays out text, as a running header printed in a table
/// of its own does, while a row of one cell in a table of data, such as a
/// header row over several columns, is data.
fn data_rows(table_rows: &[(Range<usize>, NodeId, bool)]) -> Vec<Range<usize>> {
    let tables: HashSet<NodeId> = table_rows.iter().filter(|(_, _, several)| *several).map(|(_, t, _)| *t).collect();
    table_rows.iter().filter(|(_, t, _)| tables.contains(t)).map(|(r, _, _)| r.clone()).collect()
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
/// "- F 7 -", "ii" or "- ii -", or "Page" and a page number with a prefix,
/// "Page E-1", with the number in the first group that matched (roman
/// numerals in the second and fifth, the number between dashes after an "F"
/// in the seventh). Old text filings escape a line that starts with a dash
/// as "- -", so "- -7-" is one too. A page number in parentheses after
/// "Page", "(Page 12)", is one only in a run of them (see `page_furniture`).
static PAGE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:(?:(?i:page)\s+)?",
        r"(?:(\d{1,3})|([ivxlc]{1,7})|F\s*[-‐‑‒–—−]\s*(\d{1,3})",
        r"|(?:-\s+)?[-–—]\s*(?:(\d{1,3})|([ivxlc]{1,7}))\s*[-–—])",
        r"|(?i:page)\s+[A-Z]{1,3}\s*[-‐‑‒–—−]\s*(\d{1,3})",
        r"|[-–—]\s*F\s*[-‐‑‒–—−]?\s*(\d{1,3})\s*[-–—])$",
    ))
    .unwrap()
});

/// How a bare page number (`PAGE_NUMBER`) is written, as the group of the
/// pattern for digits, roman numerals, "F-" numbers or other prefixed ones,
/// and its value.
fn page_value(line: &str) -> Option<(usize, u32)> {
    // The way of writing of each group of `PAGE_NUMBER`.
    const WRITTEN: [usize; 8] = [0, 1, 2, 0, 1, 3, 2, 0];
    let c = PAGE_NUMBER.captures(line)?;
    let (g, m) = (1..9).find_map(|g| Some((g, c.get(g)?)))?;
    let value = if WRITTEN[g - 1] == 1 { roman(m.as_str()) } else { m.as_str().parse().ok() };
    Some((WRITTEN[g - 1], value?))
}

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
/// of Contents" links, also in parentheses or brackets, and running page
/// headers and footers. Returns the lines kept, as slices of `text` in
/// order. The lines starting at the sorted offsets `cell_lines` belong to
/// table cells (see `Out::cell_lines`) and are not dropped as page numbers
/// or page furniture on their own, but those of a row of `cell_rows` (byte
/// ranges of `text`, see `Out::cell_rows`) are dropped together when the
/// row is. A printed page ends at each of the
/// sorted offsets `breaks`, and at the end of the text when there are any.
/// Those of them in `typed_breaks` are page tags inside `<pre>`: without
/// other breaks, the page numbers that count the pages end pages too (see
/// `numbered_pages`).
/// A page number with a prefix that ends a page stays when it is the page of
/// a contents entry (see `page_furniture`). The lines inside the byte ranges
/// `table_rows`, the rows of tables of data (see `data_rows`), are never
/// running headers.
fn kept_lines<'a>(
    text: &'a str,
    cell_lines: &[usize],
    cell_rows: &[Range<usize>],
    table_rows: &[Range<usize>],
    breaks: &[usize],
    typed_breaks: &[usize],
) -> Vec<&'a str> {
    debug_assert!(cell_lines.is_sorted(), "cell lines are recorded in text order");
    debug_assert!(cell_rows.is_sorted_by_key(|r| r.start), "cell rows are recorded in text order");
    debug_assert!(breaks.is_sorted(), "page breaks are recorded in text order");
    let mut cell_lines = cell_lines.iter().copied().peekable();
    let mut lines = Vec::new();
    let mut is_cell = Vec::new();
    // Whether no page ends between two offsets.
    let same_page = |from: usize, to: usize| breaks.get(breaks.partition_point(|&b| b < from)).is_none_or(|&b| b > to);
    // For each kept line, the bare page number dropped just before it on
    // its page (see `AfterNumber`).
    let mut after_number = Vec::new();
    // The bare page number dropped since the last kept line, with where it
    // ends and whether that line is on its page; and the last bare page
    // number that ended a page.
    let mut number: Option<(Option<u32>, usize, bool)> = None;
    let mut page: Option<u32> = None;
    let mut kept_end: Option<usize> = None;
    // Where each bare page number in digits that was dropped starts, and its value.
    let mut bare: Vec<(usize, u32)> = Vec::new();
    // Every bare page number that was dropped (see `PageMark`).
    let mut marks: Vec<PageMark> = Vec::new();
    for line in text.lines() {
        let start = line.as_ptr() as usize - text.as_ptr() as usize;
        while cell_lines.next_if(|&p| p < start).is_some() {}
        let cell = cell_lines.next_if_eq(&start).is_some();
        let end = start + line.len();
        let line = line.trim();
        let link = line.strip_prefix(['(', '[']).and_then(|l| l.strip_suffix([')', ']'])).unwrap_or(line);
        if line.is_empty() || link.eq_ignore_ascii_case("table of contents") {
            continue;
        }
        let ended = |number: Option<(Option<u32>, usize, bool)>, page: &mut Option<u32>| match number {
            Some((value, number_end, joined)) if same_page(number_end, start) => {
                Some(AfterNumber { value, page: *page, joined })
            }
            Some((value, _, _)) => {
                *page = value;
                None
            }
            None => None,
        };
        if !cell && let Some(c) = PAGE_NUMBER.captures(line) {
            ended(number.take(), &mut page);
            let value = c.get(1).or(c.get(4)).and_then(|m| m.as_str().parse().ok());
            bare.extend(value.map(|v| (start, v)));
            // The way each group of `PAGE_NUMBER` writes its number.
            const WRITTEN: [Written; 7] = {
                use Written::*;
                [Digits, Roman, Financial, Digits, Roman, Prefixed, Financial]
            };
            marks.extend((1..8).find_map(|g| Some((WRITTEN[g - 1], c.get(g)?.as_str()))).and_then(|(written, n)| {
                let value = if written == Written::Roman { roman(n) } else { n.parse().ok() };
                Some(PageMark { kept: lines.len(), written, value: value? })
            }));
            number = Some((value, end, kept_end.is_some_and(|k| same_page(k, start))));
            continue;
        }
        after_number.push(ended(number.take(), &mut page));
        kept_end = Some(end);
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
    unjoin_page_numbers(&mut lines, &starts, &ends_page, &bare);
    let mut gone = vec![false; lines.len()];
    for i in page_furniture(&lines, &is_cell, &rows, &ends_page, &after_number) {
        gone[i] = true;
    }
    // Where the pages end for running headers: at the page breaks, or
    // without any, at the page numbers that count the pages. The page tags
    // of a `<pre>` add to the latter.
    let numbered = if breaks.len() > typed_breaks.len() {
        None
    } else {
        numbered_pages(&lines, &marks).map(|ends| ends.iter().zip(&ends_page).map(|(&a, &b)| a || b).collect())
    };
    if let Some(ends_page) = numbered.as_ref().or(ends_page.contains(&true).then_some(&ends_page)) {
        // The table rows as sorted ranges that do not overlap.
        let mut table_rows = table_rows.to_vec();
        table_rows.sort_unstable_by_key(|r| r.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(table_rows.len());
        for r in table_rows {
            match merged.last_mut() {
                Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
                _ => merged.push(r),
            }
        }
        let in_table: Vec<bool> = starts
            .iter()
            .zip(&is_cell)
            .map(|(&s, &cell)| cell || merged.get(merged.partition_point(|r| r.end <= s)).is_some_and(|r| r.start <= s))
            .collect();
        for i in running_headers(&lines, &in_table, ends_page, &gone) {
            gone[i] = true;
        }
    }
    lines.into_iter().zip(gone).filter(|&(_, gone)| !gone).map(|(l, _)| l).collect()
}

/// Take the page number off the end of the last line of a page in a text
/// filing that prints it on the same line as the text, after a wide space:
/// "supplies.                             2". The line must end a page
/// (`ends_page`), and the bare page numbers on both sides of it must number
/// the pages around it (`bare`, as the offset in the text where each starts,
/// in order, and its value): the one printed before it must be the number
/// before or the same number, printed at the top of the page, and the one
/// printed after it the number after. The text before the space must have
/// no run of three spaces or a tab, as the columns of a table row do.
/// `starts` are the offsets of the lines in the text. HTML has no such
/// lines, as its spaces collapse.
fn unjoin_page_numbers(lines: &mut [&str], starts: &[usize], ends_page: &[bool], bare: &[(usize, u32)]) {
    // The narrowest space before the number: ten spaces.
    const SPACE: &str = "          ";
    static JOINED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\S.*?\S)( +)(\d{1,3})$").unwrap());
    for i in 0..lines.len() {
        if !ends_page[i] || !lines[i].ends_with(|c: char| c.is_ascii_digit()) || !lines[i].contains(SPACE) {
            continue;
        }
        let Some(c) = JOINED.captures(lines[i]) else { continue };
        let (words, number) = (c.get(1).expect("group 1 always matches"), &c[3]);
        if c[2].len() < SPACE.len() || words.as_str().contains("   ") || words.as_str().contains('\t') {
            continue;
        }
        let Ok(n) = number.parse::<u32>() else { continue };
        let at = bare.partition_point(|&(s, _)| s < starts[i]);
        let before = at.checked_sub(1).is_some_and(|b| bare[b].1 + 1 == n || bare[b].1 == n);
        let after = bare.get(at).is_some_and(|&(_, v)| v == n + 1);
        if before && after {
            lines[i] = &lines[i][..words.end()];
        }
    }
}

/// How a bare page number is written.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Written {
    /// "12", "- 12 -", "Page 12".
    Digits,
    /// "xii", "- xii -".
    Roman,
    /// "F-12", "- F 12 -".
    Financial,
    /// "Page E-12".
    Prefixed,
}

/// A bare page number that was dropped from the text.
struct PageMark {
    /// How many lines were kept before it.
    kept: usize,
    written: Written,
    value: u32,
}

/// The ends of the printed pages of a document that does not mark its page
/// breaks, read from its bare page numbers (`marks`, in order), as whether a
/// page ends after each of `lines`. Wherever on its page a number is
/// printed, the lines of a running header or footer around it are at the
/// edge of the pages on its two sides.
///
/// A number shows a page end when it counts the pages: the number written
/// in the same way before it is one less, or the one after it is one more,
/// and at least 500 bytes of text lie between the two. So the page
/// references of a table of contents and the cells of a table printed one
/// to a line show none. With fewer than five such page ends the pages are
/// not known, nor when most of the pages between them hold less than 1,000
/// bytes of text: those are the items of a numbered list, each a paragraph
/// or two, while a page of a filing that numbers its pages holds more. Nor
/// are they when fewer than five of the numbers are followed by a line that
/// the document prints five times or more, as a running header at the top
/// of the next page is: the items of a longer numbered list start with
/// their own titles, even when each ends with the same lines, as each
/// proposal of a proxy statement ends with "Vote Required".
///
/// Such a line that is a heading (see `Kind`), such as "Notes to Financial
/// Statements", counts only where at least two pages break inside a
/// sentence: the last line of the page ends with no stop, and the first
/// line of the next page that is not printed five times or more starts in
/// lowercase. The items of a list end where their text ends, so a caption
/// that starts each item after its number, as "Nominee for Election as
/// Director" over each nominee, is no running header. A name, a note, a
/// legend or a header that says it continues counts wherever it follows the
/// number.
fn numbered_pages(lines: &[&str], marks: &[PageMark]) -> Option<Vec<bool>> {
    const MIN_PAGES: usize = 5;
    const MIN_PAGE: usize = 500;
    const MIN_COPIES: usize = 5;
    const MIN_INSIDE: usize = 2;
    let before = bytes_before(lines);
    let counts = |a: &PageMark, b: &PageMark| a.value + 1 == b.value && before[b.kept] - before[a.kept] >= MIN_PAGE;
    let mut ends = vec![false; lines.len()];
    for written in [Written::Digits, Written::Roman, Written::Financial, Written::Prefixed] {
        let same: Vec<&PageMark> = marks.iter().filter(|m| m.written == written).collect();
        for (j, mark) in same.iter().enumerate() {
            let follows = j.checked_sub(1).is_some_and(|b| counts(same[b], mark));
            let followed = same.get(j + 1).is_some_and(|a| counts(mark, a));
            if (follows || followed) && mark.kept > 0 {
                ends[mark.kept - 1] = true;
            }
        }
    }
    // Where each page end is, in bytes of text, and the bytes of each page
    // between two of them.
    let at: Vec<usize> = (0..lines.len()).filter(|&i| ends[i]).map(|i| before[i + 1]).collect();
    if at.len() < MIN_PAGES || !full_pages(at.windows(2).map(|w| w[1] - w[0]).collect()) {
        return None;
    }
    let mut copies: HashMap<&str, usize> = HashMap::new();
    for &line in lines {
        *copies.entry(line).or_default() += 1;
    }
    let repeated = |i: usize| copies[lines[i]] >= MIN_COPIES;
    // The page ends that such a line follows, and how many of those lines
    // are headings.
    let headed: Vec<usize> = (0..lines.len()).filter(|&i| ends[i] && i + 1 < lines.len() && repeated(i + 1)).collect();
    let headings = headed.iter().filter(|&&i| Kind::of(lines[i + 1]) == Kind::Heading).count();
    if headed.len() - headings >= MIN_PAGES {
        return Some(ends);
    }
    if headed.len() < MIN_PAGES {
        return None;
    }
    // The first line at or after each line that is not printed five times
    // or more, to read the text that goes on past the header of a page.
    let mut text_from = vec![None; lines.len() + 1];
    for i in (0..lines.len()).rev() {
        text_from[i] = if repeated(i) { text_from[i + 1] } else { Some(i) };
    }
    let ends_sentence =
        |line: &str| line.trim_end_matches(['"', '\'', '”', '’', ')', ' ']).ends_with(['.', '?', '!', ':', ';']);
    let starts_lowercase = |line: &str| line.chars().find(|c| c.is_alphanumeric()).is_some_and(char::is_lowercase);
    let inside = (0..lines.len())
        .filter(|&i| {
            ends[i] && !ends_sentence(lines[i]) && text_from[i + 1].is_some_and(|j| starts_lowercase(lines[j]))
        })
        .count();
    (inside >= MIN_INSIDE).then_some(ends)
}

/// The bytes of text before each of `lines`, then the bytes of all of them.
fn bytes_before(lines: &[&str]) -> Vec<usize> {
    let mut before = Vec::with_capacity(lines.len() + 1);
    before.push(0);
    for line in lines {
        before.push(before[before.len() - 1] + line.len());
    }
    before
}

/// Whether most of the pages whose sizes in bytes of text are `sizes` hold
/// at least 1,000 bytes, as the pages of a filing do, while the items of a
/// numbered list, each a paragraph or two, hold less. `sizes` is not empty.
fn full_pages(mut sizes: Vec<usize>) -> bool {
    const MIN_MEDIAN_PAGE: usize = 1000;
    sizes.sort_unstable();
    sizes[sizes.len() / 2] >= MIN_MEDIAN_PAGE
}

/// Running page headers and footers that print no page number, such as
/// "MetLife, Inc." over "Notes to the Consolidated Financial Statements —
/// (Continued)" at the top of each page, as indexes into `lines`. Only a
/// document whose page ends are known (`ends_page`) has them, from its page
/// breaks or from its page numbers (see `numbered_pages`): the lines of
/// each page are the lines kept between two page ends, with the lines that
/// are `gone` (page numbers and other page furniture) left out.
///
/// The header block of a page is the run of up to six lines at its top whose
/// texts are running headers, and its footer block the same at its bottom
/// (what is left of the page below the header block). A text is a running
/// header when it has at least five copies, nine in ten of them are in such
/// blocks, and the pages of those copies have text of their own, a line
/// printed only once in the document: the pages with such a line, plus one
/// for any pages without, number at least half of the copies in blocks. So
/// a caption printed over many tables, which is seldom at the edge of a
/// page, stays, and so do pages printed again word for word, such as a
/// "[THIS PAGE INTENTIONALLY LEFT BLANK]" page. The texts are found as a set
/// that passes the test with the blocks it gives: starting from every text
/// with five copies, one at the edge of a page, texts that fail are taken
/// out one at a time, which shortens the blocks of their pages, until all
/// pass. Each page's blocks are worked out again only when a text at its
/// edge is taken out, so the work stays linear in the lines.
///
/// Copies that are lines of table data, which are never in a block, do not
/// count among the copies of a name, of a note or of a header that says it
/// continues, as long as five copies are left: Flagstone Reinsurance's 10-K
/// of 2011 prints its name over each page of the notes and also in the
/// heading rows of ten tables.
///
/// Only copies in a block go, never a copy in the middle of a page, and of
/// those only the ones that what the text is (see `Kind`) lets go. A header
/// that says it continues a heading goes from every block. A name or a
/// heading stays over the first page of each run of pages it heads and goes
/// from the others: a run is pages next to each other with the text in a
/// block, and a page without it ends the run. So each name stays over the
/// first page of each run of its filer's pages, as in the 10-K that US
/// Airways Group and US Airways file together, and over a page it heads
/// alone, as Amazon.com's name over "FORM 10-K". A text printed on every
/// other page is one run all the same where five or more pages two apart
/// hold it in a block and no page next to them does: JPMorgan Chase's 10-K
/// of 2014 prints "Notes to consolidated financial statements" over 74
/// such pages, and it stays over the first. A shorter chain is a run at
/// each page: "MORGAN STANLEY CHARTER SERIES" over three reports two pages
/// apart, in the 10-K the Morgan Stanley Charter funds filed in 2009, stays
/// over each. A legend never
/// goes. A note that qualifies the name or title above it ("(Unaudited)",
/// "AND SUBSIDIARIES") goes only as the second line of a header of two
/// lines, where the first goes: under a name or a heading that goes, or
/// under such a second line, when it is under that line on nine in ten of
/// the pages where the line is in a block, and not over the first page of a
/// run of its own. So "(A Wholly-Owned Subsidiary of MetLife, Inc.)" under
/// the name on each page stays once with the name, and a note under the
/// name on a few of its pages stays on each, as does a note under a header
/// that says it continues, which goes from every page. A note that opens a
/// parenthesis or a bracket and does not close it on its line stays where
/// the line under it in the block stays, which holds the rest of the note:
/// "(Information as of and for the Nine Months" over "Ended September 30,
/// 1997 is unaudited)", a legend, in FrontierVision's 10-K of 1999.
///
/// Lines that can never be running headers here, and end a block: lines of
/// table data (`in_table`: cells and the rows of tables of data, see
/// `data_rows`), so a table's header row repeated over its continued pages
/// stays a row of the table, and lines with a tab, as rows of several cells
/// in a text filing have; lines that start with "Item" or "Part", which
/// section splitting reads as Item headings and Part lines, so a header
/// below such a line stays too; lines over 120 characters or with fewer
/// than two letters; footnotes ("1 Includes ...", "(2) Excludes ...", "*
/// Restated") and the entries of a list, which start with a bullet ("•
/// Investment"); sentences, lines that end with a "." after a word of five
/// or more letters or with a number ("... Instruction I to Form 10-K."),
/// while "MetLife, Inc." and "J.P. Morgan Chase & Co." are headers.
///
/// Nor can a text with half or more of its copies on signed pages, pages
/// with a line that starts with "/s/" or "By:". Such a text heads parts of
/// the filing that are signed one by one rather than pages: "SIGNATURES"
/// and the names and titles of the officers who sign, over the signature
/// page of each of the registrants of United Refining's 10-Q of 2005, or
/// the title of each auditors' report on the pages of PPL's 10-K of 2011.
///
/// A legend or a note that passes the test of a running header is part of a
/// block all the same, as the date is in "Acme Corp." over "December 31,
/// 2024 and 2023" over "Note 2 - Accounting Policies (Continued)". Past a
/// legend a heading ends the block: a heading under a date or under the
/// units of a table stays.
fn running_headers(lines: &[&str], in_table: &[bool], ends_page: &[bool], gone: &[bool]) -> Vec<usize> {
    const MIN_COPIES: u32 = 5;
    const MIN_CHAIN: usize = 5;
    const MAX_DEPTH: usize = 6;
    const MAX_LEN: usize = 120;
    const NONE: usize = usize::MAX;
    const UNSEEN: usize = NONE - 1;
    const BULLETS: [char; 15] = ['•', '●', '▪', '■', '◦', '◾', '○', '□', '·', '➢', '►', '◆', '❖', '✓', '✔'];
    static FOOTNOTE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:\d{1,2}\s+|\(\d{1,2}\)\s*|[*†‡]+\s*)\p{L}").unwrap());
    // A line that signs a page: "/s/ Jane Doe", "By: /s/ Jane Doe", "*By:
    // Jane Doe". Most lines fail at the first character, which is checked
    // before the pattern.
    static SIGNS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\*?\s*(?:by:?\s*)?/s/|^\*?\s*by:\s").unwrap());
    let signs = |line: &str| {
        line.starts_with(|c: char| matches!(c, '/' | '*' | 'b' | 'B') || c.is_whitespace()) && SIGNS.is_match(line)
    };
    // Whether the words of a text let it be a running header (see above).
    let can_head = |line: &str| {
        line.len() <= MAX_LEN
            && !line.contains('\t')
            && !starts_like_heading(line)
            && line.chars().filter(|c| c.is_alphabetic()).take(2).count() == 2
            && !is_sentence(line)
            && !FOOTNOTE.is_match(line)
            && !line.starts_with(BULLETS)
    };
    // The kept lines, as indexes into `lines`, and the lines of each page, as
    // a range of them.
    let mut kept = Vec::with_capacity(lines.len());
    let mut pages = Vec::new();
    for (i, &ends) in ends_page.iter().enumerate() {
        if !gone[i] {
            kept.push(i);
        }
        if ends {
            let start = pages.last().map_or(0, |p: &Range<usize>| p.end);
            pages.push(start..kept.len());
        }
    }
    pages.push(pages.last().map_or(0, |p| p.end)..kept.len());
    // The text of each kept line, numbered in the order the texts first
    // appear; and for each text, its copies and, once a copy at the edge of
    // a page has been seen, its number among the texts that can be running
    // headers, or NONE when it cannot be one.
    let mut numbers: HashMap<&str, usize> = HashMap::with_capacity(kept.len());
    let mut copies: Vec<(u32, usize)> = Vec::new();
    // How many copies of each text are lines of table data.
    let mut tabled: Vec<u32> = Vec::new();
    let text: Vec<usize> = kept
        .iter()
        .map(|&i| {
            let n = *numbers.entry(lines[i]).or_insert_with(|| {
                copies.push((0, UNSEEN));
                tabled.push(0);
                copies.len() - 1
            });
            copies[n].0 += 1;
            tabled[n] += u32::from(in_table[i]);
            n
        })
        .collect();
    // How many copies of each text are on signed pages.
    let mut signed = vec![0u32; copies.len()];
    for page in &pages {
        if page.clone().any(|k| signs(lines[kept[k]])) {
            for k in page.clone() {
                signed[text[k]] += 1;
            }
        }
    }
    // The texts that can be running headers, with their copies and what each
    // is; the number of the text of each kept line at the edge of a page
    // that can be one, or NONE; and the pages where each text is at the
    // edge.
    let mut texts: Vec<(u32, Kind)> = Vec::new();
    let mut id = vec![NONE; kept.len()];
    let mut at_edge: Vec<Vec<usize>> = Vec::new();
    for (p, page) in pages.iter().enumerate() {
        let top = page.start..page.end.min(page.start + MAX_DEPTH);
        for k in top.clone().chain(page.end.saturating_sub(MAX_DEPTH).max(top.end)..page.end) {
            let line = lines[kept[k]];
            let (n, t) = &mut copies[text[k]];
            if *t == UNSEEN {
                *t = if *n >= MIN_COPIES && can_head(line) && signed[text[k]] * 2 < *n {
                    let kind = Kind::of(line);
                    // A copy in table data is never in a block. It does not
                    // count against a name, a note or a header that says it
                    // continues.
                    let tabled = tabled[text[k]];
                    let counted =
                        matches!(kind, Kind::Name | Kind::Note | Kind::Continues) && *n - tabled >= MIN_COPIES;
                    texts.push((if counted { *n - tabled } else { *n }, kind));
                    at_edge.push(Vec::new());
                    texts.len() - 1
                } else {
                    NONE
                };
            }
            if *t == NONE || in_table[kept[k]] {
                continue;
            }
            id[k] = *t;
            if at_edge[*t].last() != Some(&p) {
                at_edge[*t].push(p);
            }
        }
    }
    if texts.is_empty() {
        return Vec::new();
    }
    // Whether each page has text of its own: a line printed only once in the
    // document.
    let own: Vec<bool> = pages.iter().map(|page| page.clone().any(|k| copies[text[k]].0 == 1)).collect();
    // For each text, its copies in blocks, and the pages with those copies
    // that have text of their own and that do not.
    let mut edges = vec![0u32; texts.len()];
    let mut own_pages = vec![0u32; texts.len()];
    let mut other_pages = vec![0u32; texts.len()];
    let mut in_set = vec![true; texts.len()];
    // The lengths of the header and footer blocks of each page.
    let mut blocks = vec![(0, 0); pages.len()];
    let block_of = |page: &Range<usize>, in_set: &[bool]| {
        // The lines of a block, from the edge of the page inward. Past a
        // legend, a heading ends the block.
        let block = |from_edge: &mut dyn Iterator<Item = usize>| {
            let mut past_legend = false;
            let inside = |k: &usize| {
                let t = id[*k];
                let inside = t != NONE && in_set[t] && !(past_legend && texts[t].1 == Kind::Heading);
                past_legend |= inside && texts[t].1 == Kind::Legend;
                inside
            };
            from_edge.take(MAX_DEPTH).take_while(inside).count()
        };
        let top = block(&mut page.clone());
        let bottom = block(&mut (page.start + top..page.end).rev());
        (top, bottom)
    };
    // Add the copies in the blocks of a page to the counts, or take them
    // out, and return the texts of those copies.
    let mut in_blocks = Vec::with_capacity(2 * MAX_DEPTH);
    let mut count = |p: usize, (top, bottom): (usize, usize), add: bool, counts: [&mut [u32]; 3]| {
        let page = &pages[p];
        in_blocks.clear();
        in_blocks.extend((page.start..page.start + top).chain(page.end - bottom..page.end).map(|k| id[k]));
        in_blocks.sort_unstable();
        let [edges, own_pages, other_pages] = counts;
        let mut touched = Vec::new();
        for same in in_blocks.chunk_by(|a, b| a == b) {
            let t = same[0];
            let change = [same.len() as u32, u32::from(own[p]), u32::from(!own[p])];
            for (count, change) in [&mut edges[t], &mut own_pages[t], &mut other_pages[t]].into_iter().zip(change) {
                if add {
                    *count += change;
                } else {
                    *count -= change;
                }
            }
            touched.push(t);
        }
        touched
    };
    for (p, page) in pages.iter().enumerate() {
        blocks[p] = block_of(page, &in_set);
        count(p, blocks[p], true, [&mut edges, &mut own_pages, &mut other_pages]);
    }
    let passes = |t: usize, edges: &[u32], own_pages: &[u32], other_pages: &[u32]| {
        edges[t] * 10 >= texts[t].0 * 9 && (own_pages[t] + u32::from(other_pages[t] > 0)) * 2 >= edges[t]
    };
    let mut failing: Vec<usize> = (0..texts.len()).filter(|&t| !passes(t, &edges, &own_pages, &other_pages)).collect();
    while let Some(t) = failing.pop() {
        if !in_set[t] || passes(t, &edges, &own_pages, &other_pages) {
            continue;
        }
        in_set[t] = false;
        for &p in &at_edge[t] {
            let new = block_of(&pages[p], &in_set);
            if new == blocks[p] {
                continue;
            }
            let mut touched = count(p, blocks[p], false, [&mut edges, &mut own_pages, &mut other_pages]);
            touched.extend(count(p, new, true, [&mut edges, &mut own_pages, &mut other_pages]));
            blocks[p] = new;
            failing.extend(touched.into_iter().filter(|&u| in_set[u] && !passes(u, &edges, &own_pages, &other_pages)));
        }
    }
    // The pages where each text is in a block, and those where each note is
    // under each line it follows in a block, with the last such page.
    let mut block_pages = vec![0u32; texts.len()];
    let mut in_block: Vec<Vec<usize>> = vec![Vec::new(); texts.len()];
    let mut under: HashMap<(usize, usize), (u32, usize)> = HashMap::new();
    let mut last_page = vec![NONE; texts.len()];
    for (p, (page, &(top, bottom))) in pages.iter().zip(&blocks).enumerate() {
        for block in [page.start..page.start + top, page.end - bottom..page.end] {
            for k in block.clone() {
                let t = id[k];
                if last_page[t] != p {
                    block_pages[t] += 1;
                    in_block[t].push(p);
                }
                last_page[t] = p;
                if texts[t].1 == Kind::Note && k > block.start {
                    let (n, last) = under.entry((t, id[k - 1])).or_insert((0, NONE));
                    *n += u32::from(*last != p);
                    *last = p;
                }
            }
        }
    }
    // A text printed on every other page: a chain of pages two apart that
    // hold it in a block, with no page next to any of them holding it. A
    // chain of MIN_CHAIN pages or more is one run, and these are its pages
    // past the first, by text.
    let mut chained: HashSet<(usize, usize)> = HashSet::new();
    for (t, held) in in_block.iter().enumerate() {
        let mut start = 0;
        for chain in held.chunk_by(|a, b| a + 2 == *b) {
            let end = start + chain.len();
            // Whether the page before the chain's first page holds the
            // text, and the page after its last: such a page is not part of
            // the chain.
            let first = usize::from(start > 0 && held[start - 1] + 1 == chain[0]);
            let last = usize::from(end < held.len() && chain[chain.len() - 1] + 1 == held[end]);
            start = end;
            if chain.len() >= MIN_CHAIN + first + last {
                chained.extend(chain[first + 1..chain.len() - last].iter().map(|&p| (t, p)));
            }
        }
    }
    // The page where each text was last in a block.
    let mut last_page = vec![NONE; texts.len()];
    let mut found = Vec::new();
    for (p, (page, &(top, bottom))) in pages.iter().zip(&blocks).enumerate() {
        for block in [page.start..page.start + top, page.end - bottom..page.end] {
            // Whether the line above in the block goes as a name or a
            // heading does, or with one.
            let mut with_name = false;
            // Whether the line above in the block goes as a note that opens
            // a parenthesis or a bracket and does not close it.
            let mut open = false;
            for k in block.clone() {
                let t = id[k];
                // A copy starts a run of pages unless the text is in a block
                // on the page before or earlier on its page, or the page is
                // a later one of a chain of every other page.
                let starts_run = last_page[t] == NONE || last_page[t] + 1 < p && !chained.contains(&(t, p));
                last_page[t] = p;
                let goes = match texts[t].1 {
                    Kind::Heading | Kind::Name => !starts_run,
                    Kind::Continues => true,
                    // The second line of a header of two lines goes with
                    // the first: a note under the same line on nine in ten
                    // of that line's pages.
                    Kind::Note => {
                        with_name && !starts_run && under[&(t, id[k - 1])].0 * 10 >= block_pages[id[k - 1]] * 9
                    }
                    Kind::Legend => false,
                };
                with_name = goes && texts[t].1 != Kind::Continues;
                if goes {
                    found.push(kept[k]);
                } else if open {
                    // The rest of the note stays, so its first line does.
                    found.pop();
                }
                open = goes && texts[t].1 == Kind::Note && {
                    let line = lines[kept[k]];
                    line.matches(['(', '[']).count() > line.matches([')', ']']).count()
                };
            }
        }
    }
    found
}

/// What a running header is, which decides which of its copies in blocks go
/// (see `running_headers`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A heading that the pages after it repeat, such as "Notes to
    /// Consolidated Financial Statements", a statement's title or a standard
    /// Item title ("BUSINESS"): it stays over the first page of each run of
    /// pages it heads.
    Heading,
    /// A name (see `names_filer`): it stays over the first page of each run
    /// of pages it heads, as a heading does, but past a legend it stays in
    /// the block, which a heading ends, and its copies in table data do not
    /// count.
    Name,
    /// A note that qualifies the name or the title above it (see
    /// `qualifies`): it goes only with a name or a heading it is under on
    /// nearly all of that line's pages, and its copies in table data do not
    /// count.
    Note,
    /// A line that tells how to read the page (see `LEGEND`): it never goes,
    /// and a heading past it ends the block.
    Legend,
    /// A header that says it continues a heading (see `CONTINUED`): it goes
    /// from every block, and its copies in table data do not count.
    Continues,
}

impl Kind {
    /// What a running header is. A heading at which section splitting ends
    /// an Item (see `sections::ends_items`), such as "Independent Auditors'
    /// Report", is a heading even when it looks like a name.
    fn of(line: &str) -> Kind {
        let continues = CONTINUED.is_match(line);
        if LEGEND.is_match(line) || !continues && PERIOD.is_match(line) {
            Kind::Legend
        } else if continues {
            Kind::Continues
        } else if qualifies(line) {
            Kind::Note
        } else if !crate::sections::ends_items(line) && names_filer(line) {
            Kind::Name
        } else {
            Kind::Heading
        }
    }
}

/// A line that tells how to read a page rather than heading it: a reference
/// to the notes or to the auditors' report under a financial statement ("See
/// accompanying notes", "The accompanying notes are an integral part of
/// these statements"), the units of a table ("(In millions, except per share
/// data)", "(Tabular dollars in thousands)", "(000's omitted)") or whose
/// statements they are ("(Parent Company Only)", "Acme Corp. (Parent
/// Only)", "(Registrant Only)"). A line that starts with the date or period
/// it covers is one too, unless it says it continues (see `PERIOD`).
///
/// The units are also read from a line that starts with a parenthesis and
/// gives them as "000", "$000" or "000’s" before that parenthesis closes:
/// "($000, except share/unit data)", "(Tabular amounts in US$ 000’s, except
/// share and per share data)". The "000" follows no digit, comma or period
/// and a space, a comma, the ")" or the end of the line follows it, so
/// "(Formed in 2000)", "(10,000,000 shares authorized)" and "(Commission
/// File No. 000-12345)" give no units.
static LEGEND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)^\(?(?:see|the accompanying)\b|\bintegral part\b|\bin (?:thousands|millions|billions)\b|\btabular dollars\b",
        r"|\b000(?:['’]?s)? omitted\b|\b(?:parent|registrant)(?: company| corporation)? only\b",
        r"|^\((?:[^)]*[^\d,.)])?000(?:['’]?s)?(?:[\s,)]|$)",
    ))
    .unwrap()
});

/// The start of a line that gives the date or period of the page it is
/// printed on, with whatever follows: "December 31, 2024 and 2023", "As of
/// June 30, 2023", "For the years ended December 31, 2012 and 2011",
/// "Years ended December 2014, 2013, and 2012", "March 31, 2010 and
/// December 31, 2009", "For the Periods Ended December 31, 2009, 2008 (Not
/// Covered by the Report Included Herein)".
static PERIOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)^(?:[\w ,]*\b(?:ended|ending|as of|at|through)\s+)?",
        r"(?:jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|june?|july?|aug(?:ust)?|sep(?:t(?:ember)?)?",
        r"|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)\.?(?:\s+\d{1,2},?)?\s+\d{4}\b",
    ))
    .unwrap()
});

/// Whether a line qualifies the name or the title over which it is printed:
/// a note in parentheses or brackets that runs to the end of the line
/// ("(Unaudited)", "(Restated)", "(A Partnership)",
/// "(Debtor-in-Possession)", "[Formerly Acme Inc.]"), or the start or the
/// end of one ("(A Wholly-Owned Subsidiary of", "Holding Corporation)"), or
/// a line that goes on from the name ("AND SUBSIDIARIES", "& Subsidiary",
/// "and Predecessor"). A heading with a letter in parentheses before it,
/// "(A) Restatement", is none.
fn qualifies(line: &str) -> bool {
    let (opened, closed) = (line.matches(['(', '[']).count(), line.matches([')', ']']).count());
    line.starts_with(['(', '[']) && (line.ends_with([')', ']']) || opened > closed)
        || line.starts_with('&')
        || line.get(..4).is_some_and(|word| word.eq_ignore_ascii_case("and "))
        || closed > opened
}

/// The end of a name with its legal form: "Inc.", "Corp.", "Corporation",
/// "& Co.", "L.P.", "LP", "LLC", "Ltd.", "plc", "N.A.", "Bancorp", or "and
/// Subsidiaries", "& Subsidiary", "and its wholly-owned subsidiary", "and
/// subsidiary companies".
static LEGAL_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:\b(?:inc|incorporated|corp|corporation|co|l\.\s?p|lp|llc|ltd|limited|plc|n\.\s?a|s\.\s?a",
        r"|bancorp|bancorporation|bancshares|bankshares)\b\.?",
        r"|(?:\band|&)\s+(?:its\s+)?(?:wholly[- ]owned\s+)?subsidiar(?:y|ies)\b(?:\s+companies\b)?)[,)\]]*$",
    ))
    .unwrap()
});

/// Whether a running header names the filer rather than heading the text:
/// a line with no word that only a heading would have ("our", "to",
/// "about", "with", "for", or "the" after the first word) that ends with a
/// legal form (see `LEGAL_FORM`), or with "Company", "Bank", "Trust",
/// "Group" or "Holdings" after at least two words of the name other than a
/// first "The" and with no "of": "THE WALT DISNEY COMPANY", "Metropolitan
/// Life Insurance Company". So "THE COMPANY", "Investment Trust",
/// "Executive Officers of the Company", "Information About Our Company",
/// "Condensed Financial Statements of Parent Company" and "Notes to
/// Financial Statements of XYZ Corp. and Subsidiaries" head the text.
fn names_filer(line: &str) -> bool {
    let words: Vec<String> =
        line.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).collect();
    if words
        .iter()
        .enumerate()
        .any(|(i, w)| matches!(w.as_str(), "our" | "to" | "about" | "with" | "for") || i > 0 && w == "the")
    {
        return false;
    }
    if LEGAL_FORM.is_match(line) {
        return true;
    }
    let first = usize::from(words.first().is_some_and(|w| w == "the"));
    words.len() >= first + 3
        && !words.iter().any(|w| w == "of")
        && words
            .last()
            .is_some_and(|w| matches!(w.as_str(), "company" | "bank" | "trust" | "group" | "holding" | "holdings"))
}

/// A running header that says it continues a heading: "(Continued)",
/// "cont'd" or "(cont.)".
static CONTINUED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bcont(?:inued\b|['’]d\b|\.)").unwrap());

/// Whether a line starts, after spaces and tabs, with "item" or "part" in
/// any case, as every Item heading and Part line that section splitting
/// reads does.
fn starts_like_heading(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t']).as_bytes();
    rest.get(..4).is_some_and(|word| word.eq_ignore_ascii_case(b"item") || word.eq_ignore_ascii_case(b"part"))
}

/// Whether a line ends a sentence: with a "." (inside any closing ")") after
/// a word of five or more letters or a word with a digit, not after an
/// abbreviation such as "Inc." or "Co.".
fn is_sentence(line: &str) -> bool {
    let Some(rest) = line.trim_end_matches([')', ' ']).strip_suffix('.') else { return false };
    let word = rest.rsplit(char::is_whitespace).next().unwrap_or("");
    word.chars().filter(|c| c.is_alphabetic()).count() >= 5 || word.contains(|c: char| c.is_ascii_digit())
}

/// A bare page number dropped just before a kept line, on that line's page.
#[derive(Clone, Copy)]
struct AfterNumber {
    /// The number, when it is one in digits ("45", "- 45 -", "Page 45").
    value: Option<u32>,
    /// The number of the page before, the last bare number before it that
    /// ended a page, when that number is in digits.
    page: Option<u32>,
    /// Whether the kept line before the number is on its page.
    joined: bool,
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
/// Statement on Form" over "S-1", "... rated by S&P:" over "A-1"). So does
/// the page of the last entry of a contents page laid out one cell per line
/// ("Signatures" over "S-1", `after_number`): the line above it names the
/// entry, the bare page number above that is the page of the entry before,
/// and the line above that number names that entry, all on one page, the
/// two names no end of a sentence. The name of the entry
/// has a word that starts with the first letter of the prefix, as the name
/// of what those pages hold does ("Signatures", "Exhibit Index"), and the
/// page number points ahead: it is higher than the number of the page before,
/// which the numbers of an exhibit list printed over the exhibits'
/// descriptions ("23" over "Consent of Ernst & Young LLP") at the end of a
/// report are not. Such a page counts among the copies numbered 1, 2, 3, or
/// the copies count so without it; either way it stays.
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
/// A page number between dashes a wide space from the text of its line
/// ("- 12 -          Docket No. 95-052"), or after "Page" in parentheses
/// alone on its line ("(Page 12)"), goes when its copies pass the same two
/// tests and most of the pages between them hold a page of text (see
/// `full_pages`). The items of a list numbered between dashes hold less,
/// and so do the entries of an index of the Items incorporated by
/// reference, each with the page it points to under it, "(Page 4)".
///
/// Lines of table cells (`is_cell`) are data and never count on their own.
/// A short row printed cell by cell (`rows`, as ranges of `lines`) counts as
/// one copy: its lines, with the page number taken out of one of them, must
/// repeat as the text of a line does, as in a footer with the company name
/// over the report name and the page number in the next cell. Its copies
/// must also mostly not be next to each other, as the rows of a table are.
fn page_furniture(
    lines: &[&str],
    is_cell: &[bool],
    rows: &[Range<usize>],
    ends_page: &[bool],
    after_number: &[Option<AfterNumber>],
) -> HashSet<usize> {
    const MIN_REPEATS: usize = 5;
    const MIN_LETTERS: usize = 10;
    const MAX_LEN: usize = 120;
    // Filings also spell the dash as a non-breaking hyphen (U+2011) or an en dash.
    static FORM_NAME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b(?:10[-‐‑‒–—][KQ]|8[-‐‑‒–—]K|20[-‐‑‒–—]F|40[-‐‑‒–—]F)").unwrap());
    static GOES_ON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?i:\(continued\)|continued)$").unwrap());
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
    // Whether a line is the page of a contents entry named on the line above.
    let refers = |i: usize, prefix: &str| {
        let initial =
            |w: &str| w.chars().next().zip(prefix.chars().next()).is_some_and(|(a, b)| a.eq_ignore_ascii_case(&b));
        i.checked_sub(2).is_some_and(|before| {
            let name = i - 1;
            after_number[name].is_some_and(|n| n.joined && n.value.zip(n.page).is_none_or(|(value, page)| value > page))
                && !lines[name].ends_with('.')
                && !lines[before].ends_with('.')
                && lines[name].split(|c: char| !c.is_alphanumeric()).any(initial)
        })
    };

    // Whether most of the pages between copies hold a page of text, counting
    // the bytes before each line when a family first needs them.
    let mut bytes = None;
    let mut full = |seen: &[(usize, PageNumber)]| {
        let before: &Vec<usize> = bytes.get_or_insert_with(|| bytes_before(lines));
        full_pages(seen.windows(2).map(|w| before[w[1].0] - before[w[0].0]).collect())
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
            let labels = ending.clone().filter(|&&(i, _)| !refers(i, before));
            if ending.clone().zip(1..).all(|((_, p), k)| p.number == k)
                || labels.clone().zip(1..).all(|((_, p), k)| p.number == k)
            {
                drop.extend(labels.map(|&(i, _)| i));
            }
        }
        if n < MIN_REPEATS {
            continue;
        }
        let apart = mostly(seen.windows(2).filter(|w| w[1].0 > w[0].0 + 1).count(), n - 1);
        let furniture = match place {
            Place::Label | Place::Chapter | Place::Alone => apart && mostly(page_steps(seen), n - 1),
            // The text beside the number may be as short as "Covenant"
            // beside the numbers of a list, and "(Page 4)" may point to a
            // page, so the pages must be full.
            Place::Dashed | Place::PageInParens => apart && mostly(page_steps(seen), n - 1) && full(seen),
            Place::Edge | Place::PageOf | Place::Middle => {
                let short = letters(before) + letters(after) < MIN_LETTERS;
                let spaced = before.contains(' ') || after.contains(' ');
                let form = FORM_NAME.is_match(before) || FORM_NAME.is_match(after);
                let piped = mostly(seen.iter().filter(|(_, p)| p.piped).count(), n);
                let first = seen.iter().filter(|(_, p)| p.first).count();
                let facing = place == Place::Edge && first >= 2 && n - first >= 2;
                // A footer that only says the text goes on, on full pages
                // one to three apart: "47 (Continued)".
                let continues =
                    place == Place::Edge && GOES_ON.is_match(before) && mostly(page_steps(seen), n - 1) && full(seen);
                let text = (spaced && (!short || form)) || place == Place::PageOf || piped || facing || continues;
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
    /// After "Page" in parentheses, alone on the line: "(Page 12)".
    PageInParens,
    /// Between dashes at the start or end of the line, a wide space from
    /// its text: "- 12 -          Docket No. 95-052".
    Dashed,
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
/// period ("(12)", "12."), or one between dashes a wide space before or
/// after the text of its line, or one at the start or end of the line, in a
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
        let Some(m) = (1..8).find_map(|g| c.get(g)) else { return none() };
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
    static PAGE_IN_PARENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\((?i:page)\s+(\d{1,3})\)$").unwrap());
    if line.starts_with('(')
        && let Some(c) = PAGE_IN_PARENS.captures(line)
    {
        return [c[1].parse().ok().map(|number| PageNumber::new(number, ("(Page", ")", Place::PageInParens))), None];
    }
    let (open, close) = if line.starts_with('(') { ("(", ")") } else { ("", ".") };
    let alone = line.strip_prefix(open).and_then(|rest| rest.strip_suffix(close));
    if let Some(digits) = alone.filter(|d| (1..=3).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit())) {
        return [digits.parse().ok().map(|number| PageNumber::new(number, (open, close, Place::Alone))), None];
    }
    // The line of a text filing that starts or ends with a page number
    // between dashes, three or more spaces from its text. Only a line with
    // a dash at one end is matched, so the other lines cost nothing.
    static DASHED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:- ?(\d{1,3}) ?- {3,}(\S.*)|(\S.*?) {3,}- ?(\d{1,3}) ?-)$").unwrap());
    if (line.starts_with('-') || line.ends_with('-'))
        && let Some(c) = DASHED.captures(line)
        && let Some((number, text)) = c.get(1).zip(c.get(2)).or(c.get(4).zip(c.get(3)))
    {
        let key = (text.as_str(), "", Place::Dashed);
        return [number.as_str().parse().ok().map(|number| PageNumber::new(number, key)), None];
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
                    <p>(table of contents)</p><p>[Table of Contents]</p>\
                    <p>F - 4</p><p>F\u{2011}5</p><p>- -8-</p><p>- ii -</p><p>-iii-</p><p>Page E-1</p><p>End</p>";
        assert_eq!(html_to_text(html), "Text\nMore\nEnd\n");
        // Only a whole line is a page number.
        let html = "<p>- iv - Text</p><p>Page E-1 lists the exhibits.</p><p>See page A-2</p>";
        assert_eq!(html_to_text(html), "- iv - Text\nPage E-1 lists the exhibits.\nSee page A-2\n");
        // A number between dashes after an "F".
        let html = "<p>Text</p><p>- F 1 -</p><p>- F-2 -</p><p>-F3-</p><p>End</p>";
        assert_eq!(html_to_text(html), "Text\nEnd\n");
        for line in ["- F 1", "F 1 -", "- FS 1 -", "- G 1 -", "- F 1 - Text", "- F 1234 -", "- F -"] {
            assert_eq!(html_to_text(&format!("<p>Text</p><p>{line}</p>")), format!("Text\n{line}\n"));
        }
        for line in [
            "(Page 4",
            "Page 4)",
            "(Pages 4)",
            "(Page F-4)",
            "(Page 4) of the report",
            "(Page 4 of 9)",
            "(Page)",
            "(Page 1234)",
        ] {
            assert_eq!(html_to_text(&format!("<p>Text</p><p>{line}</p>")), format!("Text\n{line}\n"));
        }
    }

    /// "Page" and a number in parentheses alone on its line, "(Page 4)", goes
    /// when its copies number full pages one to three apart, as the pages of
    /// an annual report attached to a 10-K are numbered, and stays elsewhere,
    /// as the page an Item incorporated by reference points to does.
    #[test]
    fn page_numbers_in_parentheses_after_page_dropped() {
        for footer in ["(Page {})", "(page {})"] {
            let (html, want) = full_paged(footer, numbered(1..7));
            assert_eq!(html_to_text(&html), want, "{footer}");
        }
        let pages = [1, 4, 5, 6, 7, 8, 9, 12];
        let raw: String =
            pages.iter().map(|n| format!("{}\n          (Page {n})\n", page_text(&n.to_string()))).collect();
        let want: String = pages.iter().map(|n| format!("{}\n", page_text(&n.to_string()))).collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")), want);
        // Four copies; numbers that rise more than three at a time; pages of
        // a line or two, as in an index of the Items incorporated by
        // reference; a page reference alone.
        let (html, _) = full_paged("(Page {})", numbered(1..5));
        assert_eq!(html_to_text(&html).matches("(Page").count(), 4);
        let (html, _) = full_paged("(Page {})", (1..9).map(|n| (4 * n).to_string()));
        assert_eq!(html_to_text(&html).matches("(Page").count(), 8);
        let (html, _) = paged("(Page {})", numbered(1..9));
        assert_eq!(html_to_text(&html).matches("(Page").count(), 8);
        let html =
            "<p>Item 7. Management's Discussion and Analysis</p><p>(Page 4)</p><p>Item 8. Financial Statements</p>";
        assert_eq!(html_to_text(html).matches("(Page 4)").count(), 1);
        // Copies on lines next to each other, two steps in five; numbers of
        // four digits.
        let html: String = (1..7)
            .map(|n| {
                let text =
                    if n == 2 || n == 4 { String::new() } else { format!("<p>{}</p>", page_text(&n.to_string())) };
                format!("{text}<p>(Page {n})</p>")
            })
            .collect();
        assert_eq!(html_to_text(&html).matches("(Page").count(), 6);
        let (html, _) = full_paged("(Page {})", numbered(1001..1007));
        assert_eq!(html_to_text(&html).matches("(Page").count(), 6);
        // A page is the text from one copy to the next, the first copy
        // included: most must hold 1,000 bytes.
        for (len, copies) in [(992, 0), (991, 6)] {
            let html: String = (1..7).map(|n| format!("<p>{}</p><p>(Page {n})</p>", "x".repeat(len))).collect();
            assert_eq!(html_to_text(&html).matches("(Page").count(), copies, "{len}");
        }
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

    /// "Text of page N." and more, over 1,000 bytes, as a page of a filing holds.
    fn page_text(page: &str) -> String {
        format!("Text of page {page}. {}", ["More text."; 100].join(" "))
    }

    /// As `paged`, with a full page of text on each page.
    fn full_paged(footer: &str, pages: impl IntoIterator<Item = String>) -> (String, String) {
        let (mut html, mut want) = (String::new(), String::new());
        for page in pages {
            let text = page_text(&page);
            html += &format!("<p>{text}</p><p>{}</p>", footer.replace("{}", &page));
            want += &format!("{text}\n");
        }
        (html, want)
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

    /// A footer of a page number and "(Continued)" goes when its copies are
    /// on full pages one to three apart, on either side of the number.
    #[test]
    fn page_numbers_beside_continued_dropped() {
        for footer in ["{}          (Continued)", "(Continued) {}", "{} (continued)", "CONTINUED {}"] {
            let (html, want) = full_paged(footer, numbered(47..53));
            assert_eq!(html_to_text(&html), want, "{footer}");
            let raw: String = (47..53)
                .map(|n| format!("{}\n{}\n", page_text(&n.to_string()), footer.replace("{}", &n.to_string())))
                .collect();
            assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")), want, "{footer}");
        }
        // Four copies; another word; more beside the word.
        let (html, _) = full_paged("{} (Continued)", numbered(47..51));
        assert_eq!(html_to_text(&html).matches("(Continued)").count(), 4);
        for footer in ["{} (Concluded)", "{} (Continued) -", "{} (Continued", "{} Discontinued"] {
            let (html, _) = full_paged(footer, numbered(47..57));
            assert_eq!(html_to_text(&html).lines().count(), 20, "{footer}");
        }
        // Numbers that rise more than three at a time, as the notes that go
        // on over a page do, or that do not rise.
        let (html, _) = full_paged("{} (Continued)", (1..9).map(|n| (4 * n).to_string()));
        assert_eq!(html_to_text(&html).matches("(Continued)").count(), 8);
        let (html, _) = full_paged("{} (Continued)", [3, 3, 4, 4, 5, 5, 6, 6].map(|n| n.to_string()));
        assert_eq!(html_to_text(&html).matches("(Continued)").count(), 8);
        // Pages of a paragraph, as of notes short enough that each goes on
        // once under its number: "3 (Continued)", then "4 (Continued)".
        let (html, _) = paged("{} (Continued)", numbered(3..9));
        assert_eq!(html_to_text(&html).matches("(Continued)").count(), 6);
        // Copies on lines next to each other, two steps in five.
        let html: String = (47..53)
            .map(|n| {
                let text =
                    if n == 48 || n == 50 { String::new() } else { format!("<p>{}</p>", page_text(&n.to_string())) };
                format!("{text}<p>{n} (Continued)</p>")
            })
            .collect();
        assert_eq!(html_to_text(&html).matches("(Continued)").count(), 6);
        // The middle cell of a row of three.
        let raw: String = (47..57).map(|n| format!("{}\n(Continued)\t{n}\tA\n", page_text(&n.to_string()))).collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")).lines().count(), 20);
    }

    /// The running header of a text filing that starts or ends with its
    /// page number between dashes, a wide space from the text, goes when its
    /// copies are on full pages one to three apart.
    #[test]
    fn page_numbers_between_dashes_beside_text_dropped() {
        let text = |header: &str, pages: &[u32]| {
            let raw: String = pages
                .iter()
                .map(|n| format!("{}\n{}\n", header.replace("{}", &n.to_string()), page_text(&n.to_string())))
                .collect();
            document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>"))
        };
        let pages: Vec<u32> = (8..14).collect();
        let want: String = pages.iter().map(|n| format!("{}\n", page_text(&n.to_string()))).collect();
        for header in [
            "- {} -          Docket No. 95-052",
            "-{}-   Docket No. 95-052",
            "Docket Nos. ER95-836-000 and ER95-851-000           - {} -",
            "A   -{}-",
        ] {
            assert_eq!(text(header, &pages), want, "{header}");
        }
        // Two spaces from the text; no dash before or after the number, as
        // a negative number has none; four digits; a word between the
        // dashes; text on both sides of the number.
        for header in [
            "ORDER  - {} -",
            "- {} -  Docket No. 94-475",
            "ORDER     - {}",
            "ORDER     {} -",
            "{} -     Docket No. 94-475",
            "{} -     Docket No. 94-475 -",
            "- {}     Docket No. 94-475",
            "ORDER     - No. {} -",
            "ORDER     - {} -     Docket No. 94-475",
            "- ORDER     - {} -     Docket No. 94-475",
            "- Net loss     - {}",
            "ORDER     -  {}  -",
            "-  {}  -     Docket No. 94-475",
        ] {
            assert_eq!(text(header, &pages).lines().count(), 12, "{header}");
        }
        // Three copies each of two texts: neither has five.
        let raw: String = pages
            .iter()
            .map(|n| {
                let docket = if n % 2 == 0 { "94-475" } else { "95-052" };
                format!("- {n} -     Docket No. {docket}\n{}\n", page_text(&n.to_string()))
            })
            .collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")).lines().count(), 12);
        assert_eq!(text("ORDER     - {} -", &[1008, 1009, 1010, 1011, 1012, 1013]).lines().count(), 12);
        // Four copies; numbers that rise more than three at a time or do
        // not rise; copies on lines next to each other.
        let header = "- {} -     Docket No. 94-475";
        assert_eq!(text(header, &[8, 9, 10, 11]).lines().count(), 8);
        assert_eq!(text(header, &[8, 12, 16, 20, 24, 28]).lines().count(), 12);
        assert_eq!(text(header, &[8, 8, 8, 9, 9, 9]).lines().count(), 12);
        let raw: String = pages
            .iter()
            .map(|&n| {
                let text = if n == 9 || n == 11 { String::new() } else { format!("{}\n", page_text(&n.to_string())) };
                format!("{text}- {n} -     Docket No. 94-475\n")
            })
            .collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")).lines().count(), 10);
        // Pages of a paragraph, as of a list numbered between dashes.
        let raw: String = pages.iter().map(|n| format!("- {n} -     Covenant\nText of covenant {n}.\n")).collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")).lines().count(), 12);
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

    /// A contents page laid out one cell per line ends with the page of its
    /// last entry, "S-1" under "Signatures". The entry above has its page on
    /// the line after its name too, so the line is a reference and stays,
    /// while the labels at the foot of the signature pages go.
    #[test]
    fn page_references_of_contents_entries_that_end_a_page_stay() {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        let contents = "<p>Item 14. Principal Accountant Fees and Services</p><p>44</p>\
                        <p>Item 15. Exhibits and Financial Statement Schedules</p><p>45</p><p>Signatures</p><p>S-1</p>";
        let kept = "Item 14. Principal Accountant Fees and Services\n\
                    Item 15. Exhibits and Financial Statement Schedules\nSignatures\nS-1\n";
        // The only line of its kind, as when the signature page prints no label.
        let html = format!("{contents}{page_break}<p>Text.</p>{page_break}<p>Signed by the registrant.</p>");
        assert_eq!(tables_of(&html).0, format!("{kept}Text.\nSigned by the registrant.\n"));
        // With labels on two signature pages and on the exhibit index.
        let html = format!(
            "{contents}{page_break}<p>Text.</p>{page_break}<p>Signed by the registrant.</p><p>S-1</p>{page_break}\
             <p>Signed by the directors.</p><p>S-2</p>{page_break}<p>Exhibit index.</p><p>E-1</p>"
        );
        assert_eq!(
            tables_of(&html).0,
            format!("{kept}Text.\nSigned by the registrant.\nSigned by the directors.\nExhibit index.\n")
        );
        // A "Table of Contents" link between the page above and the entry.
        let html = format!(
            "<p>Exhibit Index</p><p>45</p><p><a href=\"#toc\">Table of Contents</a></p><p>Signatures</p><p>S-1</p>\
             {page_break}<p>Text.</p>"
        );
        assert_eq!(tables_of(&html).0, "Exhibit Index\nSignatures\nS-1\nText.\n");
        // Other prefixes, and an entry that names its pages after other words.
        for (entry, page) in
            [("Exhibit Index", "E-1"), ("Index to Exhibits", "E - 1"), ("Financial Statement Schedules", "FS-1")]
        {
            let html = format!("<p>Signatures</p><p>45</p><p>{entry}</p><p>{page}</p>{page_break}<p>Text.</p>");
            assert_eq!(tables_of(&html).0, format!("Signatures\n{entry}\n{page}\nText.\n"));
        }
        // A text filing.
        let raw = "<DOCUMENT>\n<TEXT>\nItem 14. Exhibits\n     45\nSignatures\n     S-1\n<PAGE>\nText.\n<PAGE>\n\
                   Signed by the registrant.\n     S-1\n</TEXT>";
        assert_eq!(document_to_text(raw), "Item 14. Exhibits\nSignatures\nS-1\nText.\nSigned by the registrant.\n");
    }

    /// A label at the foot of a page still goes when the line above it does
    /// not follow a page number printed on the same page: when that number
    /// ends the page before, is a cell of a table, comes above another line,
    /// or when the line above the label ends a sentence or has no word that
    /// the prefix abbreviates, as an exhibit's description below its number.
    #[test]
    fn page_labels_below_lines_that_are_no_contents_entries_dropped() {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        for (page, kept) in [
            (format!("<p>Text.</p><p>44</p>{page_break}<p>Signatures</p><p>S-1</p>"), "Text.\nSignatures\n"),
            (
                format!(
                    "<p>Text.</p>{page_break}<table><tr><td>Total</td><td>45</td></tr></table><p>Schedule II</p><p>S-1</p>"
                ),
                "Text.\nTotal\t45\nSchedule II\n",
            ),
            (
                format!("<p>Text.</p>{page_break}<p>Exhibits</p><p>45</p><p>Signatures</p><p>John Smith</p><p>S-1</p>"),
                "Text.\nExhibits\nSignatures\nJohn Smith\n",
            ),
            (
                format!(
                    "<p>Text.</p>{page_break}<p>Note</p><p>12</p><p>The directors signed this report.</p><p>S-1</p>"
                ),
                "Text.\nNote\nThe directors signed this report.\n",
            ),
            (
                format!(
                    "<p>Text.</p>{page_break}<p>24</p><p>Power of attorney</p><p>32</p><p>Certification</p><p>E-1</p>"
                ),
                "Text.\nPower of attorney\nCertification\n",
            ),
            // No line above the number, a sentence above it, or a page break
            // between that line and the number.
            (format!("<p>45</p><p>Signatures</p><p>S-1</p>{page_break}<p>Text.</p>"), "Signatures\nText.\n"),
            // The number of the entry above is the number of the page before.
            (
                format!("<p>Text.</p><p>45</p>{page_break}<p>Exhibits</p><p>45</p><p>Signatures</p><p>S-1</p>"),
                "Text.\nExhibits\nSignatures\n",
            ),
            (
                format!("<p>The report ends here.</p><p>45</p><p>Signatures</p><p>S-1</p>{page_break}<p>Text.</p>"),
                "The report ends here.\nSignatures\nText.\n",
            ),
            (
                format!(
                    "<p>Item 14. Exhibits</p>{page_break}<p>45</p><p>Signatures</p><p>S-1</p>{page_break}<p>Text.</p>"
                ),
                "Item 14. Exhibits\nSignatures\nText.\n",
            ),
        ] {
            assert_eq!(tables_of(&page).0, kept, "{page}");
        }
    }

    /// An exhibit list at the end of a report prints each exhibit's number
    /// over its description, and its pages end with labels E-1, E-2, E-3.
    /// The number above the first page's last description ("23" over
    /// "Consent of Ernst & Young LLP") is lower than the number of the page
    /// before, so it is no page of a contents entry, and every label goes.
    #[test]
    fn exhibit_numbers_over_descriptions_are_no_contents_pages() {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        let html = format!(
            "<p>Text.</p><p>45</p>{page_break}<p>EXHIBIT INDEX</p><p>21</p><p>Subsidiaries of the Registrant</p>\
             <p>23</p><p>Consent of Ernst &amp; Young LLP</p><p>E-1</p>{page_break}<p>31</p>\
             <p>Certification of the Chief Executive Officer</p><p>E-2</p>{page_break}<p>32</p>\
             <p>Section 1350 Certifications</p><p>E-3</p>{page_break}<p>End.</p>"
        );
        assert_eq!(
            tables_of(&html).0,
            "Text.\nEXHIBIT INDEX\nSubsidiaries of the Registrant\nConsent of Ernst & Young LLP\n\
             Certification of the Chief Executive Officer\nSection 1350 Certifications\nEnd.\n"
        );
        // Pages that point ahead stay a contents page's.
        let html = format!(
            "<p>Cover.</p><p>1</p>{page_break}<p>Exhibits</p><p>45</p><p>Signatures</p><p>S-1</p>{page_break}<p>Text.</p>"
        );
        assert_eq!(tables_of(&html).0, "Cover.\nExhibits\nSignatures\nS-1\nText.\n");
    }

    /// The signature pages are labeled from S-2 on, the first one unlabeled.
    /// With the contents entry's page they count 1, 2, 3: the labels go and
    /// the contents entry's page stays.
    #[test]
    fn page_labels_that_go_on_from_a_contents_page_dropped() {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        let html = format!(
            "<p>Item 14. Principal Accountant Fees and Services</p><p>44</p>\
             <p>Item 15. Exhibits and Financial Statement Schedules</p><p>45</p><p>Signatures</p><p>S-1</p>{page_break}\
             <p>Text.</p>{page_break}<p>Signed by the registrant.</p>{page_break}<p>Signed by the directors.</p>\
             <p>S-2</p>{page_break}<p>Signed by more directors.</p><p>S-3</p>{page_break}<p>End.</p>"
        );
        assert_eq!(
            tables_of(&html).0,
            "Item 14. Principal Accountant Fees and Services\nItem 15. Exhibits and Financial Statement Schedules\n\
             Signatures\nS-1\nText.\nSigned by the registrant.\nSigned by the directors.\nSigned by more directors.\nEnd.\n"
        );
    }

    /// HTML of printed pages, each given as its paragraphs, with a page break
    /// after each page.
    fn printed(pages: &[Vec<String>]) -> String {
        let page_break = "<div style=\"page-break-after: always\"></div>";
        pages.iter().map(|page| page.iter().map(|p| format!("<p>{p}</p>")).collect::<String>() + page_break).collect()
    }

    /// `pages` pages, page `n` made of `top`, "Text of page n." and `bottom`,
    /// each with "{}" replaced by `n`.
    fn framed(top: &[&str], bottom: &[&str], pages: std::ops::Range<u32>) -> Vec<Vec<String>> {
        pages
            .map(|n| {
                let fill = |l: &&str| l.replace("{}", &n.to_string());
                let text = format!("Text of page {n}.");
                top.iter().map(fill).chain([text]).chain(bottom.iter().map(fill)).collect()
            })
            .collect()
    }

    /// Lines printed at the top or the foot of most pages without a page
    /// number go where the filing marks its page breaks, but for the first
    /// copy of a name, also when they sit below or above another such line,
    /// while a caption printed as often in the middle of the pages stays.
    #[test]
    fn running_headers_without_page_numbers_dropped() {
        let mut pages = vec![vec!["Annual Report".to_string(), "MetLife, Inc.".to_string(), "Cover.".to_string()]];
        for n in 1..11 {
            pages.push(vec![
                "MetLife, Inc.".into(),
                "Notes to the Consolidated Financial Statements — (Continued)".into(),
                format!("Text of page {n}."),
                "(In millions)".into(),
                format!("More text of page {n}."),
                "MetLife, Inc. and Subsidiaries".into(),
                format!("Acme Corporation Annual Report {}", n + 40),
            ]);
        }
        let mut want = "Annual Report\nMetLife, Inc.\nCover.\nMetLife, Inc.\n".to_string();
        for n in 1..11 {
            want += &format!("Text of page {n}.\n(In millions)\nMore text of page {n}.\n");
            if n == 1 {
                want += "MetLife, Inc. and Subsidiaries\n";
            }
        }
        let html = printed(&pages);
        assert_eq!(tables_of(&html).0, want);
        assert_eq!(html_to_text(&html), want);
        // A text filing's pages.
        let raw: String = pages.iter().map(|p| p.join("\n") + "\n<PAGE>\n").collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")), want);
        // Nine copies in ten at the edges of pages: those go but the first,
        // the other stays.
        let mut pages = framed(&["Acme Corporation"], &["End of page {}."], 1..10);
        pages[4].insert(2, "Acme Corporation".into());
        let text = html_to_text(&printed(&pages));
        assert_eq!(text.matches("Acme Corporation").count(), 2, "{text}");
        // Six lines deep at the top of each page; the seventh line is no
        // header.
        let top = [
            "Acme Corp. (Continued)",
            "Notes (Continued)",
            "Balance Sheets (Continued)",
            "Schedule I (Continued)",
            "Segment Data (Continued)",
            "Exhibits (continued)",
            "Seventh line",
        ];
        let text = html_to_text(&printed(&framed(&top, &[], 1..7)));
        assert_eq!(text.lines().filter(|l| top.contains(l)).collect::<Vec<_>>(), ["Seventh line"; 6]);
        // Pages with no text of their own count as one page: four of seven
        // pages are enough.
        let mut pages = framed(&["Acme Corporation"], &[], 1..4);
        pages.extend(vec![vec!["Acme Corporation".to_string(), "Blank page".to_string()]; 4]);
        let text = html_to_text(&printed(&pages));
        assert_eq!(text.matches("Acme").count(), 1, "{text}");
        // A table of one cell per row only lays out its text.
        let page_break = "<div style=\"page-break-after: always\"></div>";
        let html: String = (1..7)
            .map(|n| {
                format!(
                    "<table><tr><td>Acme Inc.</td></tr><tr><td></td><td>Notes (continued)</td></tr></table><p>Text {n}.</p>{page_break}"
                )
            })
            .collect();
        assert_eq!(
            html_to_text(&html),
            "Acme Inc.\n".to_string() + &(1..7).map(|n| format!("Text {n}.\n")).collect::<String>()
        );
    }

    /// In a document without page breaks, running headers go when its bare
    /// page numbers count the pages: five or more numbers written in the
    /// same way, each one more than the one before, with 500 bytes of text
    /// between them and, on most pages, 1,000, and five of them followed by
    /// a line printed five times or more.
    #[test]
    fn running_headers_dropped_where_page_numbers_show_the_pages() {
        // Pages of two header lines and `filler` + 49 bytes of text, each
        // ending with its number, or starting with it.
        let pages = |numbers: &[&str], filler: usize, top: bool| -> Vec<Vec<String>> {
            let page = |(i, number): (usize, &&str)| {
                let mut page = vec![
                    "Acme Corporation".to_string(),
                    "Notes (Continued)".to_string(),
                    format!("Text of page {i}. {}", "x".repeat(filler)),
                ];
                if !number.is_empty() {
                    page.insert(if top { 0 } else { 3 }, number.to_string());
                }
                page
            };
            numbers.iter().enumerate().map(page).collect()
        };
        let html = |pages: &[Vec<String>]| pages.concat().iter().map(|l| format!("<p>{l}</p>")).collect::<String>();
        let headers =
            |text: &str| text.lines().filter(|l| ["Acme Corporation", "Notes (Continued)"].contains(l)).count();
        let all = [
            ["3", "4", "5", "6", "7", "8"],
            ["- 3 -", "- 4 -", "- 5 -", "- 6 -", "- 7 -", "- 8 -"],
            ["Page 3", "Page 4", "Page 5", "PAGE 6", "- 7 -", "8"],
            ["F-3", "F-4", "F-5", "- F 6 -", "F-7", "F-8"],
            ["i", "ii", "iii", "iv", "v", "- vi -"],
            ["Page A-1", "Page A-2", "Page A-3", "Page A-4", "Page A-5", "Page A-6"],
            ["i", "ii", "iii", "1", "2", "3"],
        ];
        for numbers in all {
            for top in [false, true] {
                let pages = pages(&numbers, 951, top);
                let text = html_to_text(&html(&pages));
                assert_eq!((headers(&text), text.lines().count()), (1, 7), "{numbers:?} {top}");
                let raw = pages.concat().join("\n");
                let text = document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}\n</TEXT>"));
                assert_eq!((headers(&text), text.lines().count()), (1, 7), "{numbers:?} {top}");
            }
        }
        // Five numbers end five pages, but not when the first comes before
        // any text.
        let five = ["3", "4", "5", "6", "7", ""];
        assert_eq!(headers(&html_to_text(&html(&pages(&five, 951, false)))), 1);
        assert_eq!(headers(&html_to_text(&html(&pages(&five, 951, true)))), 12);
        // Four numbers end four pages, with a fifth page after them, also
        // when a fifth number that does not count the pages ends it.
        for last in ["", "9"] {
            assert_eq!(headers(&html_to_text(&html(&pages(&["3", "4", "5", "6", last], 951, false)))), 10);
        }
        // Page breaks show the pages where there are any: numbers in the
        // middle of those pages change nothing, even with full pages between.
        let broken: String = (3..9)
            .map(|n| {
                let filler = "x".repeat(1000);
                let page =
                    ["Acme Corporation", "Notes (Continued)", &format!("Text of page {n}. {filler}"), &n.to_string()];
                let page: String = page.iter().map(|l| format!("<p>{l}</p>")).collect();
                format!("{page}<p>More text of page {n}.</p><div style=\"page-break-after: always\"></div>")
            })
            .collect();
        assert_eq!((headers(&html_to_text(&broken)), html_to_text(&broken).lines().count()), (1, 13));
        // Most pages one byte short of 1,000 bytes of text.
        assert_eq!(headers(&html_to_text(&html(&pages(&all[0], 950, false)))), 12);
        // Four of the six pages between seven page ends, and three.
        let mut short = pages(&["3", "4", "5", "6", "7", "8", "9"], 951, false);
        for page in &mut short[1..5] {
            page[2].pop();
        }
        assert_eq!(headers(&html_to_text(&html(&short))), 14);
        short[4][2].push('x');
        assert_eq!(headers(&html_to_text(&html(&short))), 1);
        // The text before the first page end is no page between two of them:
        // two short pages of the four between five page ends are not most,
        // also after a short first page.
        let mut cover = pages(&["3", "4", "5", "6", "7", ""], 951, false);
        for page in &mut cover[..3] {
            let len = page[2].len();
            page[2].truncate(len - 400);
        }
        assert_eq!(headers(&html_to_text(&html(&cover))), 1);
        // Between two pages of 500 bytes a number counts the pages; between
        // two of 499 bytes it does not, and the headers of the second page
        // are in the middle of a page.
        for (cut, want) in [(500, 1), (501, 16)] {
            let mut pages = pages(&["3", "4", "5", "6", "7", "8", "9", "10"], 951, false);
            for page in &mut pages[5..7] {
                let len = page[2].len();
                page[2].truncate(len - cut);
            }
            assert_eq!(headers(&html_to_text(&html(&pages))), want, "{cut}");
        }
        // A page ends at the line before its number: with the number at the
        // top of each page over a running header, the footers at the bottom
        // of the page before go. Without that header, the numbers show no
        // pages.
        let footed: Vec<Vec<String>> = pages(&all[0], 951, true)
            .into_iter()
            .map(|mut page| {
                let text = page.remove(3);
                page.insert(1, text);
                page
            })
            .collect();
        assert_eq!(headers(&html_to_text(&html(&footed))), 12);
        let headed: Vec<Vec<String>> = footed
            .into_iter()
            .map(|mut page| {
                page.insert(1, "Acme Holdings, Inc.".into());
                page
            })
            .collect();
        assert_eq!(headers(&html_to_text(&html(&headed))), 1);
        // Numbers that do not count the pages: not one more each, the same
        // number, falling, two ways of writing in turn, and only four pages.
        for numbers in [
            ["2", "4", "6", "8", "10", "12"],
            ["7", "7", "7", "7", "7", "7"],
            ["8", "7", "6", "5", "4", "3"],
            ["3", "F-4", "5", "F-6", "7", "F-8"],
            ["i", "2", "iii", "4", "v", "6"],
            ["3", "4", "5", "6", "", ""],
            ["", "", "", "", "", ""],
        ] {
            for top in [false, true] {
                assert_eq!(headers(&html_to_text(&html(&pages(&numbers, 951, top)))), 12, "{numbers:?} {top}");
            }
        }
        // The numbers of a list are no page numbers: "Director" stays, also
        // with paragraphs of 500 bytes in the items.
        for words in [0, 100] {
            let list: String = (1..9)
                .map(|n| format!("<p>{n}</p><p>Director</p><p>Name {n}</p><p>{}</p>", "Text ".repeat(words)))
                .collect();
            assert_eq!(html_to_text(&list).matches("Director").count(), 8, "{words}");
        }
        // A line printed five times or more, the running header at the top
        // of the next page, follows five of the numbers; following four, the
        // numbers show no pages and the footers stay, also when a line
        // printed more often follows a fifth.
        let doc = |tops: usize, fifth: &str| -> String {
            (0..8)
                .map(|i| {
                    let top = match i {
                        _ if (1..=tops).contains(&i) => "<p>Notes (Continued)</p>".to_string(),
                        _ if i == tops + 1 && !fifth.is_empty() => format!("<p>{fifth}</p>"),
                        _ => String::new(),
                    };
                    let text = format!("Text of page {i}. {}", "x".repeat(1000));
                    format!("{top}<p>{text}</p><p>Acme Corporation</p><p>{}</p>", i + 3)
                })
                .collect()
        };
        for (tops, fifth, want) in [(5, "", (1, 0)), (4, "", (8, 4)), (4, "Acme Corporation", (9, 4))] {
            let text = html_to_text(&doc(tops, fifth));
            let found = (text.matches("Acme Corporation").count(), text.matches("Notes").count());
            assert_eq!(found, want, "{tops} {fifth}");
        }
        // So the items of a longer list, each a page of text under its own
        // title that ends with the same line, are no pages: "Vote Required"
        // stays.
        let list: String = (1..9)
            .map(|n| format!("<p>{n}</p><p>Proposal {n}</p><p>Text. {}</p><p>Vote Required</p>", "x".repeat(1000)))
            .collect();
        assert_eq!(html_to_text(&list).matches("Vote Required").count(), 8);
        // Four numbers that a name follows are not enough, and five are: the
        // name over the text before the first number stays either way.
        for (tops, want) in [(4, 5), (5, 1)] {
            let html: String = (0..8)
                .map(|i| {
                    let top = if i <= tops { "<p>Acme Corporation</p>" } else { "" };
                    format!("{top}<p>Text of page {i}. {}</p><p>{}</p>", "x".repeat(1000), i + 3)
                })
                .collect();
            assert_eq!(html_to_text(&html).matches("Acme Corporation").count(), want, "{tops}");
        }
        // A caption that starts each item of a list after its number is no
        // running header, though a page of text follows it: each item ends
        // with a sentence, or with a line printed in every item.
        let item = |n: usize, caption: &str, last: &str| {
            [n.to_string(), caption.into(), format!("Name {n}"), format!("Text. {}.", "x".repeat(1000)), last.into()]
        };
        for (caption, last) in
            [("Nominee for Election as Director", ""), ("Property Description", "Encumbrances: None")]
        {
            let items: Vec<[String; 5]> = (1..13).map(|n| item(n, caption, last)).collect();
            let html: String = items.concat().iter().filter(|l| !l.is_empty()).map(|l| format!("<p>{l}</p>")).collect();
            let text = html_to_text(&html);
            assert_eq!(
                (text.matches(caption).count(), text.matches("Encumbrances").count()),
                (12, 12 * last.len().min(1))
            );
            let raw: String = items.concat().iter().filter(|l| !l.is_empty()).map(|l| format!("{l:^60}\n")).collect();
            let text = document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>"));
            assert_eq!(text.matches(caption).count(), 12, "{caption}");
        }
        // A note that follows the numbers counts as a name does: the name at
        // the foot of each page goes.
        let html: String = (0..8)
            .map(|i| {
                let text = format!("Text of page {i}. {}", "x".repeat(1000));
                format!("<p>(Unaudited)</p><p>{text}</p><p>Acme Corporation</p><p>{}</p>", i + 3)
            })
            .collect();
        let text = html_to_text(&html);
        assert_eq!((text.matches("Acme Corporation").count(), text.matches("(Unaudited)").count()), (1, 8));
        // A heading over the pages after the numbers, on the first page and
        // the next `tops`, is their running header where two pages break
        // inside a sentence: the last line of the page ends with `end`, and
        // the next page goes on in lowercase. One such page is not enough,
        // nor are pages that go on in lowercase after a stop, nor four
        // numbers that the heading follows.
        let notes = |tops: usize, inside: usize, end: &str| -> String {
            (0..8)
                .map(|i| {
                    let top = if i <= tops { "<p>Notes to Financial Statements</p>" } else { "" };
                    let start =
                        if i > 0 && i <= inside { format!("rest of sentence {i}.") } else { format!("Text {i}.") };
                    let end = if i < inside { end } else { "end." };
                    format!("{top}<p>{start} {} {end}</p><p>{}</p>", "x".repeat(1000), i + 3)
                })
                .collect()
        };
        for (tops, inside, end, want) in [
            (7, 2, "and the", 1),
            (7, 1, "and the", 8),
            (7, 0, "and the", 8),
            (7, 2, "the end.", 8),
            (7, 2, "(the end.)", 8),
            (7, 2, "as follows:", 8),
            (7, 2, "follows;", 8),
            (5, 2, "and the", 1),
            (4, 2, "and the", 5),
        ] {
            let found = html_to_text(&notes(tops, inside, end)).matches("Notes to").count();
            assert_eq!(found, want, "{tops} {inside} {end}");
        }
    }

    /// Copies in table data do not count against a name, a note that goes
    /// with it or a header that says it continues, while they count against
    /// a heading.
    #[test]
    fn names_also_printed_in_tables_dropped_from_page_tops() {
        // Ten pages under each header of `tops`; the first header of `tops`
        // is also a row of a table of data on `tabled` of its pages, or a
        // line of text in the middle of them.
        let doc = |tops: &[&[&str]], tabled: usize, in_table: bool| -> String {
            let mut html = String::new();
            for (t, top) in tops.iter().enumerate() {
                for n in 0..10 {
                    html += &top.iter().map(|l| format!("<p>{l}</p>")).collect::<String>();
                    html += &format!("<p>Text of page {t}.{n}.</p>");
                    if t == 0 && n < tabled {
                        let copy = top[0];
                        html += &if in_table {
                            format!("<table><tr><td>{copy}</td></tr><tr><td>Sales {n}</td><td>12</td></tr></table>")
                        } else {
                            format!("<p>{copy}</p><p>Sales {n}</p>")
                        };
                    }
                    html += "<div style=\"page-break-after: always\"></div>";
                }
            }
            html
        };
        let count = |html: &str, line: &str| html_to_text(html).lines().filter(|l| *l == line).count();
        let acme = "Acme Corporation";
        // The three copies in tables stay, and so does the first of the ten
        // at the top of pages.
        assert_eq!(count(&doc(&[&[acme]], 3, true), acme), 4);
        // Copies in the middle of pages count: three in thirteen keep all.
        assert_eq!(count(&doc(&[&[acme]], 3, false), acme), 13);
        // One in eleven does not, in a table or not.
        assert_eq!(count(&doc(&[&[acme]], 1, false), acme), 2);
        let html = doc(&[&[acme, "Notes (Continued)"]], 3, true);
        assert_eq!(count(&html, acme), 4);
        // Five copies outside tables are enough, and four are not.
        let html = doc(&[&[acme]], 3, true);
        let eight: String = html.split_inclusive("</div>").take(5).collect();
        assert_eq!(count(&eight, acme), 4);
        let six: String = html.split_inclusive("</div>").take(4).collect();
        assert_eq!(count(&six, acme), 7);
        // The same with another name over the pages after them, which stays
        // over the first of its pages, whatever filer it names.
        for other in [
            "Beta Power LLC",
            "Acme Power Corporation",
            "The Beta Company of Ohio, Inc.",
            "Acme Corporation and Subsidiaries",
            "ACME CORPORATION",
            "Acme Corp.",
        ] {
            let html = doc(&[&[acme], &[other]], 3, true);
            assert_eq!((count(&html, acme), count(&html, other)), (4, 1), "{other}");
        }
        // A note under the name on each page goes with it, and stays over
        // the first page.
        for note in [
            "(A Development Stage Company)",
            "[Formerly Beta Inc.]",
            "and Subsidiaries",
            "AND SUBSIDIARIES",
            "& Subsidiary",
            "(A Wholly-Owned Subsidiary of",
            "Holding Corporation)",
        ] {
            let html = doc(&[&[acme, note]], 3, true);
            assert_eq!((count(&html, acme), count(&html, note)), (4, 1), "{note}");
        }
        // A note's copies in tables do not count either, so a header that
        // says it continues stays in the block below it.
        let html = doc(&[&["(Unaudited)", "Notes (Continued)"]], 3, true);
        assert_eq!((count(&html, "(Unaudited)"), count(&html, "Notes (Continued)")), (13, 0));
        // A heading keeps its count, whoever the filer is.
        let heading = "Summary of Results";
        assert_eq!(count(&doc(&[&[heading]], 3, true), heading), 13);
        // A header that says it continues does not.
        let notes = "Notes (Continued)";
        let html = doc(&[&[notes, acme], &["Beta Power LLC"]], 3, true);
        assert_eq!((count(&html, notes), count(&html, acme)), (3, 1));
    }

    /// A legend printed with the running header of each page stays, and a
    /// name or a header that says it continues under it or, at the foot of
    /// the page, over it, is in the block all the same: the name goes but
    /// for its first copy, the header from every page. A heading there ends
    /// the block and stays.
    #[test]
    fn running_headers_dropped_past_a_legend() {
        let count =
            |pages: &[Vec<String>], line: &str| html_to_text(&printed(pages)).lines().filter(|l| *l == line).count();
        let (date, units, notes) = ("December 31, 2024 and 2023", "(In thousands)", "Note 2 - Policies (Continued)");
        for legends in [
            &[date][..],
            &[units],
            &[date, units],
            &["(000's omitted)", "As of June 30, 2023", date],
            &["For the years ended December 31, 2012 and 2011", "Years ended December 2014, 2013, and 2012"],
            &["(Parent Company Only)"],
            &["Acme Corporation (Parent Only)", "(PARENT CORPORATION ONLY)", "(Registrant Only)"],
        ] {
            let top = [&["Acme Corporation"], legends, &[notes, "ACME CORPORATION"]].concat();
            let pages = framed(&top, &[], 1..7);
            for (line, want) in [("Acme Corporation", 1), (notes, 0), ("ACME CORPORATION", 1)] {
                assert_eq!(count(&pages, line), want, "{line} under {legends:?}");
            }
            for legend in legends {
                assert_eq!(count(&pages, legend), 6, "{legend}");
            }
            // The same at the foot of the page, in the other order.
            let bottom: Vec<&str> = top.iter().rev().copied().collect();
            let pages = framed(&[], &bottom, 1..7);
            assert_eq!((count(&pages, notes), count(&pages, "ACME CORPORATION"), count(&pages, legends[0])), (0, 1, 6));
        }
        // A heading past a legend stays, and so does what is past the
        // heading.
        for heading in ["Notes to Financial Statements", "Segment Information", "Item 8. Financial Statements"] {
            for legend in [date, units] {
                let pages = framed(&["Acme Corporation", legend, heading, notes], &[], 1..7);
                assert_eq!(
                    (
                        count(&pages, "Acme Corporation"),
                        count(&pages, legend),
                        count(&pages, heading),
                        count(&pages, notes)
                    ),
                    (1, 6, 6, 6),
                    "{heading} {legend}"
                );
            }
        }
        // A legend at the top of the page ends no block at its foot.
        let pages = framed(&[date], &["Segment Information"], 1..7);
        assert_eq!((count(&pages, date), count(&pages, "Segment Information")), (6, 1));
        // A legend that is mostly in the middle of pages is in no block.
        let mut pages = framed(&[units, notes], &[], 1..7);
        for page in &mut pages {
            page.extend([units.to_string(), "More text.".to_string(), units.to_string(), "End.".to_string()]);
        }
        assert_eq!((count(&pages, units), count(&pages, notes)), (18, 6));
        // Nor is one with fewer than five copies.
        let mut pages = framed(&[notes], &[], 1..7);
        for page in &mut pages[..4] {
            page.insert(0, date.to_string());
        }
        assert_eq!((count(&pages, date), count(&pages, notes)), (4, 6));
        // A legend that ends like a name is a legend all the same, and the
        // copies of the name over it in tables are left out of its count.
        for legend in [date, "In thousands - Beta Corp."] {
            let html: String = (1..11)
                .map(|n| {
                    let table = "<table><tr><td>Acme Corporation</td></tr><tr><td>Sales</td><td>12</td></tr></table>";
                    let table = if n < 4 { table.replace("Sales", &format!("Sales {n}")) } else { String::new() };
                    let page_break = "<div style=\"page-break-after: always\"></div>";
                    format!("<p>Acme Corporation</p><p>{legend}</p><p>Text of page {n}.</p>{table}{page_break}")
                })
                .collect();
            let text = html_to_text(&html);
            assert_eq!((text.matches("Acme Corporation").count(), text.matches(legend).count()), (4, 10), "{legend}");
        }
        // The legend counts among the six lines of a block.
        let top = ["Acme Corp.", date, units, "As of June 30, 2023", "Tabular dollars", "Acme Corp. LLC", notes];
        let pages = framed(&top, &[], 1..7);
        assert_eq!((count(&pages, "Acme Corp. LLC"), count(&pages, notes)), (1, 6));
        // A legend that says it continues is a legend all the same, and past
        // a legend a note stays in the block: the header under it goes.
        let (continued, stated) = ("(In thousands) (Continued)", "(Stated in U.S. Dollars)");
        let pages = framed(&["Acme Corporation", continued, stated, notes], &[], 1..7);
        let counts = (count(&pages, "Acme Corporation"), count(&pages, continued), count(&pages, stated));
        assert_eq!((counts, count(&pages, notes)), ((1, 6, 6), 0));
    }

    /// A name stays over the first page of each run of pages it heads, as a
    /// heading does, and goes from the others, whether the headers name one
    /// filer or several; a note under it on each page stays and goes with it.
    /// A page without the name ends the run, and so does a page with another
    /// name in its place. A copy at the foot of a page with the name at its
    /// top goes on with the run.
    #[test]
    fn names_stay_over_the_first_page_of_each_run() {
        let filing = |pages: &[Vec<String>]| {
            let raw: String = pages.iter().map(|p| p.join("\n") + "\n<PAGE>\n").collect();
            document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>"))
        };
        let names =
            |text: &str| -> Vec<String> { text.lines().filter(|l| !l.starts_with("Text")).map(String::from).collect() };
        let (acme, beta, note) =
            ("Acme Corporation", "Beta Power LLC", "(A Wholly-Owned Subsidiary of Acme Corporation)");
        // Five pages of each filer, then five more of the first.
        let mut pages = framed(&[acme], &[], 1..6);
        pages.extend(framed(&[beta, note], &[], 6..11));
        pages.extend(framed(&[acme], &[], 11..16));
        for text in [html_to_text(&printed(&pages)), filing(&pages)] {
            assert_eq!(names(&text), [acme, beta, note, acme], "{text}");
            assert!(text.contains(&format!("Text of page 5.\n{beta}\n{note}\nText of page 6.\n")), "{text}");
            assert!(text.contains(&format!("Text of page 10.\n{acme}\nText of page 11.\n")), "{text}");
        }
        // Two names of one filer and a third name of another.
        let mut pages = framed(&[acme], &[], 1..6);
        pages.extend(framed(&["Acme Corporation and Subsidiaries"], &[], 6..11));
        pages.extend(framed(&[beta], &[], 11..16));
        assert_eq!(names(&filing(&pages)), [acme, "Acme Corporation and Subsidiaries", beta]);
        // Two names over each page, the second changing: the first stays
        // once, the second over the first page of each of its runs.
        let mut pages = framed(&[acme, beta], &[], 1..6);
        pages.extend(framed(&[acme, "Gamma Gas Company"], &[], 6..11));
        pages.extend(framed(&[acme, beta], &[], 11..16));
        assert_eq!(names(&filing(&pages)), [acme, beta, "Gamma Gas Company", beta]);
        // A run of seven pages, and one more copy one page after its last
        // or before its first, which goes on with the run, or two pages,
        // which starts a run of its own. A header that says it continues
        // goes from every page.
        let doc = |header: &str, gap: u32, after: bool| -> Vec<Vec<String>> {
            let (run, alone) = if after { (1..8, 7 + gap) } else { (gap + 1..gap + 8, 1) };
            (1..16)
                .map(|n| {
                    let top = if run.contains(&n) || n == alone { vec![header.to_string()] } else { Vec::new() };
                    top.into_iter().chain([format!("Text of page {n}.")]).collect()
                })
                .collect()
        };
        for after in [true, false] {
            let count = |gap: u32| html_to_text(&printed(&doc(acme, gap, after))).matches(acme).count();
            assert_eq!((count(1), count(2), count(3)), (1, 2, 2), "{after}");
            let continued = "Notes (Continued)";
            let text = html_to_text(&printed(&doc(continued, 3, after)));
            assert!(!text.contains(continued), "{text}");
        }
        // A name at the top and at the foot of each page stays once: its
        // second copy on a page goes on with the run.
        let pages = framed(&[acme], &[acme], 1..7);
        for text in [html_to_text(&printed(&pages)), filing(&pages)] {
            assert_eq!(text.matches(acme).count(), 1, "{text}");
        }
        // "(Continued)" alone says it continues, though it is written as a
        // note is, and goes from every page.
        let text = html_to_text(&printed(&framed(&[acme, "(Continued)"], &[], 1..7)));
        assert_eq!((text.matches(acme).count(), text.matches("(Continued)").count()), (1, 0), "{text}");
    }

    /// A heading that is also the running header of the pages after it stays
    /// at the top of the first page of each run of pages it heads, and once
    /// when it is printed on every other page of six, while a header
    /// that says it continues the heading goes. Under a legend a heading
    /// stays on every page and a name does not, which tells the two apart; a
    /// heading at which an Item ends is a heading even when it looks like a
    /// name. A note with no line over it stays on every page.
    #[test]
    fn headings_printed_as_running_headers_stay_once() {
        let notes = "Notes to Consolidated Financial Statements";
        // Two runs of pages two pages apart, and a run of continued pages.
        let mut pages = framed(&["Acme Corporation", notes], &[], 1..8);
        pages.extend(framed(&["Acme Corporation"], &[], 8..10));
        pages.extend(framed(&["Acme Corporation", notes], &[], 10..13));
        pages.extend(framed(
            &["Acme Corporation", "Notes to Consolidated Financial Statements (Continued)"],
            &[],
            13..18,
        ));
        let text = html_to_text(&printed(&pages));
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with("Text")).collect();
        assert_eq!(lines, ["Acme Corporation", notes, notes], "{text}");
        assert!(text.starts_with(&format!("Acme Corporation\n{notes}\nText of page 1.\n")), "{text}");
        assert!(text.contains(&format!("Text of page 9.\n{notes}\nText of page 10.\n")), "{text}");
        // On facing pages, six pages of each are one run.
        let pages: Vec<Vec<String>> = (1..13)
            .map(|n| {
                let top = if n % 2 == 1 { vec![notes.to_string()] } else { vec!["Acme Corporation".to_string()] };
                top.into_iter().chain([format!("Text of page {n}.")]).collect()
            })
            .collect();
        let text = html_to_text(&printed(&pages));
        assert_eq!(text.lines().filter(|l| !l.starts_with("Text")).count(), 2, "{text}");
        // Other headings and Item titles.
        for title in [
            "Management's Discussion and Analysis",
            "Glossary of Terms",
            "Schedule II",
            "EXHIBIT INDEX",
            "Exhibits",
            "Index to Financial Statements",
            "Supplemental Financial Information",
            "Supplementary Data",
            "Report of Independent Registered Public Accounting Firm",
            "Selected Financial Data",
            "The Consolidated Balance Sheets",
            "Condensed Statements of Operations",
            "BUSINESS",
            "Risk Factors",
            "Properties",
            "Legal Proceedings",
            "Signatures",
            "Power of Attorney",
            "Quantitative and Qualitative Disclosures About Market Risk",
            "Financial Statements and Supplementary Data",
            "Controls and Procedures",
            "Executive Compensation",
            "Segment Information",
            "Business Overview",
            "Independent Auditors' Report",
            "Consent of Independent Registered Public Accounting Firm",
            "Schedule II of Acme Holdings, Inc.",
            "Bank Regulation",
            // Headings that end with a word of a name.
            "THE COMPANY",
            "Description of the Company",
            "Information About Our Company",
            "Executive Officers of the Company",
            "Bank Holdings",
            "THE BANK",
            "Regulation of the Bank",
            "Supervision and Regulation of Holding Companies",
            "Condensed Consolidating Financial Statements of Parent Company",
            "Notes to Financial Statements of XYZ Corp. and Subsidiaries",
            "Investment Trust",
            "Limited Partners",
            "Our Business Group",
            "Transactions with Affiliated Companies",
            "Guarantor Subsidiaries",
            "Investments in Subsidiaries",
            "Acme Companies",
            "Report to the Board of Acme Inc.",
            "Letter about Acme Inc.",
            "Notes for Acme Corp.",
            "Business with Acme Corp.",
            "Our Acme Corp.",
            "(A) Restatement",
            "Restated Results (a Summary)",
            "Loans Made by the Bank",
            "THE PARENT COMPANY",
        ] {
            let pages = framed(&[title], &[], 1..8);
            let text = html_to_text(&printed(&pages));
            assert_eq!(text.lines().filter(|&l| l == title).count(), 1, "{text}");
            let raw: String = pages.iter().map(|p| p.join("\n") + "\n<PAGE>\n").collect();
            let text = document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>"));
            assert_eq!(text.lines().filter(|&l| l == title).count(), 1, "{text}");
            let text = html_to_text(&printed(&framed(&["December 31, 2024", title], &[], 1..8)));
            assert_eq!(text.lines().filter(|&l| l == title).count(), 7, "{text}");
        }
        // Headers that continue a heading go from every page, also one that
        // starts with a date.
        for header in [
            "Notes to Financial Statements, cont'd",
            "Notes to Financial Statements (cont.)",
            "December 31, 2024 and 2023 (Continued)",
        ] {
            let text = html_to_text(&printed(&framed(&[header], &[], 1..8)));
            assert!(!text.contains(header), "{text}");
        }
        // Notes with no line over them stay on every page.
        for note in ["(A Delaware Limited Partnership)", "[Formerly Acme Mining]", "(Formerly Acme Holdings)"] {
            let text = html_to_text(&printed(&framed(&[note], &[], 1..8)));
            assert_eq!(text.matches(note).count(), 7, "{text}");
        }
        // Names.
        for header in [
            "Acme Corporation",
            "ACME INC.",
            "THE FIRST SIGNATURE BANK",
            "Pfizer Inc. and Subsidiary Companies",
            "Acme Bancorporation",
            "Acme Bankshares",
            "Cellular Communications of Puerto Rico, Inc.",
            "Acme Bancorp, Inc. and Subsidiaries",
            "Acme Holdings & Its Wholly-Owned Subsidiary",
            "Acme Bancorp",
            "THE ACME TOOL COMPANY",
            "Acme & Co.",
            "J.P. Morgan Chase & Co.",
            "Acme Energy Partners, L.P.",
            "Acme Operating Partnership LP",
            "Acme Ltd.",
            "Acme Limited",
            "Acme PLC",
            "Acme Bank, N.A.",
            "Acme S.A.",
            "Acme Bancshares",
            "Acme Royalty Trust",
            "First Citizens Bancshares, Inc.,",
            "Acme Incorporated",
            "Acme Tool Holding",
            "Acme LLC",
            "Acme Tool Group",
        ] {
            for top in [&[header][..], &["December 31, 2024", header]] {
                let text = html_to_text(&printed(&framed(top, &[], 1..8)));
                assert_eq!(text.lines().filter(|&l| l == header).count(), 1, "{text}");
            }
        }
    }

    /// Lines at the edges of pages stay when they are no running header:
    /// too few copies, or too many in the middle of pages; Item headings and
    /// Part lines, and the lines below them; table rows and cells, as a
    /// table's header row repeated on its continued pages; long lines and
    /// lines of fewer than two letters; sentences and footnotes under the
    /// statements; pages printed again word for word, with no text of their
    /// own; and any line of a document without page breaks.
    #[test]
    fn page_edge_lines_that_are_no_running_headers_stay() {
        let cases: Vec<(Vec<Vec<String>>, &str, usize)> = vec![
            (framed(&["Acme Corporation"], &[], 1..5), "Acme Corporation", 4),
            (
                {
                    let mut pages = framed(&["Acme Corporation"], &["End of page {}."], 1..9);
                    pages[2].insert(2, "Acme Corporation".into());
                    pages[5].insert(2, "Acme Corporation".into());
                    pages
                },
                "Acme Corporation",
                10,
            ),
            (
                {
                    let mut pages = framed(&["Acme Corporation", "Notes (Continued)"], &[], 1..7);
                    for (n, page) in pages.iter_mut().enumerate() {
                        page.push("Acme Corporation".into());
                        page.push(format!("End of page {n}."));
                    }
                    pages
                },
                "Notes (Continued)",
                6,
            ),
            (
                {
                    // Once the header above it goes, a footer on other pages
                    // has too few copies at the edges.
                    let mut pages = framed(&["Acme Corporation", "Notes (Continued)"], &[], 1..7);
                    for page in &mut pages {
                        page.push("Acme Corporation".into());
                        page.push(format!("End of {}", page[2]));
                    }
                    pages.extend(framed(&[], &["Notes (Continued)"], 7..13));
                    pages
                },
                "Notes (Continued)",
                12,
            ),
            (
                {
                    // Pages of the header alone count once.
                    let mut pages = framed(&["Acme Corporation"], &[], 1..7);
                    pages.extend(vec![vec!["Acme Corporation".to_string()]; 2]);
                    for page in &mut pages[..2] {
                        page.push("Acme Corporation".into());
                        page.push(format!("End of {}", page[1]));
                    }
                    pages
                },
                "Acme Corporation",
                10,
            ),
            (framed(&["PART II", "Notes (Continued)"], &[], 1..7), "PART II", 6),
            (framed(&["PART II", "Notes (Continued)"], &[], 1..7), "Notes (Continued)", 6),
            (framed(&["Item 8. Financial Statements"], &[], 1..7), "Item 8.", 6),
            (framed(&[&"Acme Corporation Annual Report ".repeat(5)], &[], 1..7), "Acme Corporation Annual", 6),
            (framed(&["-- A --"], &[], 1..7), "-- A --", 6),
            (framed(&[], &["See accompanying notes to the financial statements."], 1..7), "See accompanying", 6),
            (framed(&[], &["Omitted pursuant to Instruction I to Form 10-K."], 1..7), "Omitted", 6),
            (framed(&[], &["1 Includes results of Acme Ltd"], 1..7), "1 Includes", 6),
            (framed(&[], &["(2) Excludes Acme Ltd"], 1..7), "(2) Excludes", 6),
            (framed(&[], &["* Restated"], 1..7), "* Restated", 6),
            (framed(&[], &["See Notes to Consolidated Financial Statements"], 1..7), "See Notes", 6),
            (framed(&[], &["(see accompanying notes)"], 1..7), "(see", 6),
            (framed(&[], &["The accompanying notes are part of these statements"], 1..7), "The accompanying", 6),
            (framed(&[], &["Notes are an integral part of these statements"], 1..7), "Notes are", 6),
            (framed(&["(In thousands, except per share data)"], &[], 1..7), "(In thousands", 6),
            (framed(&["(Dollars in millions)"], &[], 1..7), "(Dollars", 6),
            (framed(&["(in billions)"], &[], 1..7), "(in billions", 6),
            (framed(&["Tabular dollars"], &[], 1..7), "Tabular", 6),
            (framed(&["(000’s omitted in all tables)"], &[], 1..7), "(000’s omitted", 6),
            (framed(&["($000 OMITTED)"], &[], 1..7), "($000 OMITTED", 6),
            (framed(&["(000's omitted)"], &[], 1..7), "(000's omitted", 6),
            (framed(&["December 31, 2024 and 2023"], &[], 1..7), "December 31", 6),
            (framed(&["Sept. 30, 2024, 2023 & 2022"], &[], 1..7), "Sept. 30", 6),
            (framed(&["As of June 30, 2023"], &[], 1..7), "As of June", 6),
            (framed(&["For the years ended December 31, 2024 and 2023"], &[], 1..7), "For the years", 6),
            (
                framed(&["For the period from inception on May 6, 2003 through December 31, 2007"], &[], 1..7),
                "For the",
                6,
            ),
            (framed(&["At December 31, 2024 and 2023"], &[], 1..7), "At December", 6),
            (vec![vec!["[THIS PAGE INTENTIONALLY LEFT BLANK]".to_string()]; 6], "[THIS PAGE", 6),
            (framed(&["• Investment"], &[], 1..7), "• Investment", 6),
            (framed(&[], &["● Audit Committee"], 1..7), "● Audit", 6),
        ];
        for (pages, start, copies) in cases {
            let html = printed(&pages);
            let text = html_to_text(&html);
            assert_eq!(text.lines().filter(|l| l.starts_with(start)).count(), copies, "{start}: {text}");
            // Without page breaks nothing goes.
            let html = html.replace("page-break-after: always", "");
            assert_eq!(html_to_text(&html).lines().filter(|l| l.starts_with(start)).count(), copies, "{start}");
        }
        // Rows of a table at the top of each page, printed as a line of cells
        // or cell by cell, or a row of one cell in a table of data.
        let page_break = "<div style=\"page-break-after: always\"></div>";
        for (row, lines) in [
            ("<td>Year</td><td>2024</td>", 1),
            ("<td>Acme<br>Corporation</td><td>Annual Report</td>", 3),
            ("<td colspan=\"2\">Incorporated by Reference</td></tr><tr><td>Exhibit</td><td>Form</td>", 2),
        ] {
            let html: String =
                (1..7).map(|n| format!("<table><tr>{row}</tr></table><p>Text of page {n}.</p>{page_break}")).collect();
            let text = html_to_text(&html);
            assert_eq!(text.lines().filter(|l| !l.starts_with("Text")).count(), 6 * lines, "{text}");
        }
        // A cell of one line beside a long cell in a layout table.
        let long = "Text of the page. ".repeat(60);
        let html: String = (1..7)
            .map(|n| format!("<table><tr><td>Acme Corporation</td><td>{long}{n}</td></tr></table>{page_break}"))
            .collect();
        assert_eq!(html_to_text(&html).matches("Acme Corporation").count(), 6);
        // A row of cells in a text filing.
        let raw: String = (1..7).map(|n| format!("Acme\tCorporation\nText of page {n}.\n<PAGE>\n")).collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")).matches("Acme\t").count(), 6);
    }

    /// The lines of parts of a filing signed one by one stay, at the top or
    /// the foot of pages next to each other all the same: the signature page
    /// of each registrant, in HTML or in a text filing, and each auditors'
    /// report. A name over more pages than are signed still goes from the
    /// later pages of its run.
    #[test]
    fn lines_of_signed_pages_stay() {
        let pages = |lines: &[&str]| -> Vec<Vec<String>> {
            (1..7).map(|n| lines.iter().map(|l| l.replace("{}", &n.to_string())).collect()).collect()
        };
        let want = |pages: &[Vec<String>]| pages.concat().join("\n") + "\n";
        for signed in [
            pages(&[
                "SIGNATURES",
                "Acme Subsidiary {} has caused this report to be signed.",
                "/s/ Jane Doe",
                "Jane Doe",
                "President",
            ]),
            pages(&[
                "SIGNATURES",
                "Acme Subsidiary {} has caused this report to be signed.",
                "By /s/ Jane Doe",
                "President",
            ]),
            pages(&[
                "Report of Independent Registered Public Accounting Firm",
                "We have audited the balance sheets of Acme Subsidiary {}.",
                "/s/ Ernst & Young LLP",
                "Philadelphia, Pennsylvania",
            ]),
        ] {
            assert_eq!(html_to_text(&printed(&signed)), want(&signed));
        }
        let signed = pages(&[
            "SIGNATURES",
            "Acme Subsidiary {} has caused this report to be signed.",
            "*By:  R. Russell Davis",
            "Attorney-in-Fact",
        ]);
        let raw: String = signed.iter().map(|page| page.join("\n") + "\n<PAGE>\n").collect();
        assert_eq!(document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>")), want(&signed));
        // The name over eight pages, the last four or three of them signed.
        for (signed, copies) in [(4, 8), (3, 1)] {
            let mut pages = framed(&["Acme Corporation"], &[], 1..9);
            for page in &mut pages[8 - signed..] {
                page.push("/s/ Jane Doe".into());
            }
            let text = html_to_text(&printed(&pages));
            assert_eq!(text.matches("Acme Corporation").count(), copies, "{text}");
        }
    }

    /// A text filing that prints a page's number at the end of its last line,
    /// after a wide space, loses the number when it is the one after the page
    /// number printed before it, or the number printed at the top of its
    /// page, and the one before the page number printed after it.
    #[test]
    fn page_numbers_joined_to_a_last_line_dropped() {
        let filing = |pages: &[&str]| {
            let raw: String = pages.iter().map(|p| format!("{p}\n<PAGE>\n")).collect();
            document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{raw}</TEXT>"))
        };
        let gap = " ".repeat(30);
        let joined = format!("Pilot training systems and\nsupplies.{gap}2");
        for pages in [
            [format!("Text.\n{gap}1"), joined.clone(), format!("Text.\n{gap}3")],
            ["Text.".to_string(), format!("{gap}2\n{joined}"), format!("{gap}3\nText.")],
        ] {
            let pages = pages.each_ref().map(String::as_str);
            assert_eq!(filing(&pages), "Text.\nPilot training systems and\nsupplies.\nText.\n", "{pages:?}");
        }
        // With a page number on one side only, it stays.
        for pages in [
            [format!("Text.\n{gap}1"), joined.clone(), "Text.".to_string()],
            ["Text.".to_string(), joined.clone(), format!("Text.\n{gap}3")],
            [format!("Text.\n{gap}3"), joined.clone(), format!("Text.\n{gap}3")],
        ] {
            let pages = pages.each_ref().map(String::as_str);
            assert!(filing(&pages).contains(&format!("supplies.{gap}2\n")), "{pages:?}");
        }
        // A narrower space, a number that is no next page, a table row and a
        // line that does not end its page keep their numbers.
        for (page, line) in [
            (format!("Text.\nsupplies.{}2", " ".repeat(9)), "supplies.         2"),
            (format!("Text.\nsupplies.{gap}7"), "supplies.                              7"),
            (format!("Text.\nTokyo,   Japan{gap}2"), "Tokyo,   Japan                              2"),
            (format!("Text.\nTokyo,\tJapan{gap}2"), "Tokyo,\tJapan                              2"),
            (format!("supplies.{gap}2\nText."), "supplies.                              2"),
        ] {
            let text = filing(&[&format!("Text.\n{gap}1"), &page, &format!("Text.\n{gap}3")]);
            assert!(text.contains(&format!("{line}\n")), "{text}");
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
            "<td>(Page {})</td><td>Acme Corporation<br>2021 Annual Report</td><td>Part II</td>",
            "<td>- F {} -</td><td>Acme Corporation<br>2021 Annual Report</td><td>Part II</td>",
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

    /// Text inside `<pre>` keeps its line breaks and the spaces inside its
    /// lines, through the elements inside it. Outside `<pre>` a line break
    /// is a space as before, and so it is again after the `<pre>` ends.
    #[test]
    fn pre_keeps_its_lines_and_spaces() {
        let html = "<html><body><pre>\n\n       ITEM 1.    BUSINESS\n\n  We make   things.  \n\
                    <b>Sales</b>     <u>2003</u>   &amp;   <font size=2>2002\nNet    1</font>\n</pre>\
                    <p>After   the\nblock.</p></body></html>";
        let want = "ITEM 1.    BUSINESS\nWe make   things.\nSales     2003   &   2002\nNet    1\nAfter the block.\n";
        assert_eq!(html_to_text(html), want);
        // The same lines outside `<pre>` are one line with single spaces.
        let flowed = html.replace("<pre>", "<div>").replace("</pre>", "</div>");
        assert_eq!(
            html_to_text(&flowed),
            "ITEM 1. BUSINESS We make things. Sales 2003 & 2002 Net 1\nAfter the block.\n"
        );
        // A `<pre>` inside another one: the outer one goes on after it.
        let nested = "<pre>a  b\n<pre>c  d\ne</pre>f  g\nh</pre><p>i  j\nk</p>";
        assert_eq!(html_to_text(nested), "a  b\nc  d\ne\nf  g\nh\ni j k\n");
        // An unclosed `<pre>` holds the rest of the document.
        assert_eq!(html_to_text("<p>a\nb</p><pre>c  d\ne<p>f\ng"), "a b\nc  d\ne\nf\ng\n");
    }

    /// Inside `<pre>`, a tab stays, any other space is one space, as a
    /// non-breaking space is, a `<br>` and a block break the line as
    /// elsewhere, and characters that print nothing are left out.
    #[test]
    fn pre_spaces_and_breaks() {
        let html = "<pre>\tName\tAge\u{a0}\u{a0}Held\r\nA\u{200b}B<br>C  D<div>E  F</div>G&#13;H\u{c}I</pre>";
        assert_eq!(html_to_text(html), "Name\tAge  Held\nAB\nC  D\nE  F\nG H I\n");
        // Lines of spaces only, and the spaces at the two ends of a line, go.
        assert_eq!(html_to_text("<pre>   \n \u{a0} \n  a  \n\t\n  b</pre>"), "a\nb\n");
    }

    /// A text filing inside `<html><body><pre>` comes out as the same text
    /// filed as plain text does: its page tags end the pages, so the page
    /// numbers go, also one joined to the last line of a page and the "S-1"
    /// and "E-1" of its last pages, and so do the running headers but for
    /// the name on the cover.
    #[test]
    fn pre_text_is_cleaned_as_a_text_filing_is() {
        let mut body = String::from("                 ACME CORP\n\n                 FORM 10-K\n<PAGE>\n");
        for n in 2..9 {
            body += &format!("ACME CORP\nNOTES TO FINANCIAL STATEMENTS (Continued)\n\nItem {n}.    Title {n}\n\n");
            body += &format!("     Text of page {n}, which no other page has, to fill the page with\nwords.");
            body += &if n == 4 { format!("{n:>40}\n<PAGE>\n") } else { format!("\n\n{n:>40}\n<PAGE>\n") };
        }
        body += "SIGNATURES\n\n      S-1\n<PAGE>\nEXHIBIT INDEX\n      E-1\n";
        let plain = document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{body}</TEXT>\n</DOCUMENT>"));
        assert_eq!(plain.lines().count(), 2 + 7 * 3 + 2, "{plain}");
        assert!(plain.starts_with("ACME CORP\nFORM 10-K\nItem 2.") && !plain.contains("NOTES"), "{plain}");
        assert_eq!(plain.matches("ACME").count(), 1, "{plain}");
        assert!(
            plain.contains(
                "Item 4.    Title 4\nText of page 4, which no other page has, to fill the page with\nwords.\n"
            )
        );
        let wrapped = format!("<DOCUMENT>\n<TEXT>\n<HTML>\n<BODY>\n<PRE>\n{body}</PRE>\n</BODY>\n</HTML>\n</TEXT>");
        assert!(document_is_html(&wrapped));
        assert_eq!(document_to_text(&wrapped), plain);
        // The SGML table tags of a text filing inside `<pre>` leave its lines
        // in place, with or without a caption.
        let table = "ITEM 6.    SELECTED DATA\n<TABLE>\n<CAPTION>\n                Year ended\n<S>      <C>\n\
                     Net sales   $ 6,134\n<FN>\n(1) A   note.\n</FN>\n</TABLE>\n<TABLE>\n<S>      <C>\n\
                     Net income  $   134\n</TABLE>\nITEM 7.    DISCUSSION\n";
        let plain = document_to_text(&format!("<DOCUMENT>\n<TEXT>\n{table}</TEXT>"));
        assert_eq!(plain.lines().count(), 6);
        assert_eq!(
            document_to_text(&format!("<DOCUMENT>\n<TEXT>\n<HTML><BODY><PRE>\n{table}</PRE></BODY></HTML></TEXT>")),
            plain
        );
    }

    /// A page tag ends a page only inside `<pre>`: elsewhere in HTML the
    /// pages end where a style says so, as before. Without a page end, the
    /// page numbers that count only at the end of a page stay.
    #[test]
    fn page_tags_end_pages_only_inside_pre() {
        let pages = "Text.\n<PAGE>\nSignatures.\n\n                S-1\n<PAGE>\nIndex.\n   E-1\n";
        assert_eq!(
            html_to_text(&format!("<html><body><pre>{pages}</pre></body></html>")),
            "Text.\nSignatures.\nIndex.\n"
        );
        let unmarked = pages.replace("<PAGE>\n", "");
        assert_eq!(
            html_to_text(&format!("<html><body><pre>{unmarked}</pre></body></html>")),
            "Text.\nSignatures.\nS-1\nIndex.\nE-1\n"
        );
        let flowed =
            "<html><body><p>Text.</p><PAGE><p>Signatures.</p><p>S-1</p><PAGE><p>Index.</p><p>E-1</p></body></html>";
        assert_eq!(html_to_text(flowed), "Text.\nSignatures.\nS-1\nIndex.\nE-1\n");
        // A page tag after the `<pre>` has ended is outside it.
        let after =
            "<html><body><pre>Text.</pre><PAGE><p>Signatures.</p><p>S-1</p><PAGE><p>Index.</p><p>E-1</p></body></html>";
        assert_eq!(html_to_text(after), "Text.\nSignatures.\nS-1\nIndex.\nE-1\n");
    }

    /// A short `<pre>` in ordinary HTML: a table typed in columns keeps its
    /// rows, and one in a table cell makes the cell one of several lines.
    /// The cells of a table row inside `<pre>` are ordinary cells.
    #[test]
    fn pre_blocks_in_ordinary_html() {
        let html = "<p>Sales by   year\nfollow.</p><pre>\n                 2003      2002\n\
                    Net sales       6,134     5,921\nNet income        134       (21)\n</pre><p>More   text.</p>";
        assert_eq!(
            html_to_text(html),
            "Sales by year follow.\n2003      2002\nNet sales       6,134     5,921\nNet income        134       (21)\nMore text.\n"
        );
        let html = "<table><tr><td>Office</td><td><pre>1 Main  Street\nSpringfield</pre></td></tr>\
                    <tr><td>Phone</td><td><pre>555  0100</pre></td></tr></table>";
        let (text, tables) = tables_of(html);
        assert_eq!(text, "Office\n1 Main  Street\nSpringfield\nPhone\t555  0100\n");
        assert_eq!(rows(&tables[0]), [vec!["Office", "1 Main  Street\nSpringfield"], vec!["Phone", "555  0100"]]);
        let html = "<pre>a  b\n<table><tr><td>Net   sales\nof goods</td><td>6,134</td></tr></table>c  d\ne</pre>";
        let (text, tables) = tables_of(html);
        assert_eq!(text, "a  b\nNet sales of goods\t6,134\nc  d\ne\n");
        assert_eq!(rows(&tables[0]), [vec!["Net sales of goods", "6,134"]]);
    }

    /// The lines of a `<pre>` element in which no page ends are kept as the
    /// lines of table cells are: a number typed alone on its line in a table
    /// stays, and so do the lines of a heading typed at the top of each page,
    /// while in a `<pre>` with a page tag they are a page number and a
    /// running header. A `<pre>` of one line is an ordinary line. The lines
    /// around the `<pre>` are cleaned as before.
    #[test]
    fn pre_without_page_ends_is_table_data() {
        let table = "Reserve at start of year      1,234\nCharged to expense\n                                 45\n\
                     Reserve at end of year        1,279\n";
        let lines =
            "Reserve at start of year      1,234\nCharged to expense\n45\nReserve at end of year        1,279\n";
        let html = format!("<p>12</p><p>The reserve:</p><pre>\n{table}</pre><p>13</p><p>More.</p>");
        assert_eq!(html_to_text(&html), format!("The reserve:\n{lines}More.\n"));
        // A `<pre>` ending in a line break, and one inside a cell.
        assert_eq!(html_to_text(&format!("<pre>{table}</pre><p>45</p>")), lines);
        let (text, _) = tables_of(&format!("<table><tr><td><pre>{table}</pre></td></tr></table><p>45</p>"));
        assert_eq!(text, lines);
        // With a page tag in it, the `<pre>` holds pages.
        let paged = format!("<pre>{table}<PAGE>\nNext page.</pre>");
        assert_eq!(html_to_text(&paged), lines.replace("45\n", "") + "Next page.\n");
        // A `<pre>` inside one that holds pages is part of those pages.
        let nested = "<pre>Charged to expense\n<pre>        45\n</pre>Next\n<PAGE>\nNext page.</pre>";
        assert_eq!(html_to_text(nested), "Charged to expense\nNext\nNext page.\n");
        // The line of the next `<pre>` starts after the spaces that end one.
        assert_eq!(html_to_text("<pre>Text.\n<PAGE>\n   </pre><pre>45</pre>"), "Text.\n45\n");
        // With a row printed cell by cell inside.
        let html = "<pre>Reserve\n<table><tr><td>Charged<br>to expense</td><td>45</td></tr></table>12\n</pre><p>13</p>";
        assert_eq!(html_to_text(html), "Reserve\nCharged\nto expense\n45\n12\n");
        let html = html.replace("12\n", "Total\n");
        assert_eq!(html_to_text(&html), "Reserve\nCharged\nto expense\n45\nTotal\n");

        let page = |n: u32, head: &str| {
            format!(
                "<div style=\"page-break-before: always\">{head}<pre>Text of page {n}, which no other page has.\n</pre></div>"
            )
        };
        let typed: String = (1..8).map(|n| page(n, "<pre>ACME CORP\nNOTES</pre>")).collect();
        assert_eq!(html_to_text(&typed).matches("ACME CORP\nNOTES\n").count(), 7);
        for head in ["<p>ACME CORP</p>", "<pre>ACME CORP</pre>", "<pre>\n  ACME CORP\n\n</pre>"] {
            let one_line: String = (1..8).map(|n| page(n, head)).collect();
            assert_eq!(html_to_text(&one_line).matches("ACME").count(), 1, "{head}");
        }
    }

    /// A page tag inside a small `<pre>` ends a page, and in a document that
    /// marks no page breaks the page numbers that count the pages end them
    /// as before: the running headers they show still go, all but the first
    /// copy of a name.
    #[test]
    fn page_tag_in_a_small_pre() {
        let words = "Words of this page alone, which no other page has. ".repeat(25);
        let page = |n: u32| format!("<p>ACME CORP</p><p>NOTES (Continued)</p><p>Page {n}: {words}</p><p>{n}</p>");
        let pages: String = (1..9).map(page).collect();
        let text = html_to_text(&pages);
        assert!(text.matches("ACME CORP").count() == 1 && !text.contains("NOTES"), "{text}");
        let stray = pages.replacen("<p>4</p>", "<pre>\n<PAGE>\n</pre><p>4</p>", 1);
        assert_eq!(html_to_text(&stray), text);
        // In a table cell too.
        let in_cell =
            pages.replacen("<p>4</p>", "<table><tr><td><pre>Note.\nMore.\n<PAGE>\n</pre></td></tr></table><p>4</p>", 1);
        assert_eq!(html_to_text(&in_cell).matches("ACME CORP").count(), 1, "{in_cell}");
        // A page tag also ends a page that no page number ends, as a cover.
        let cover = format!("<p>Cover. {words}</p><pre>\n<PAGE>\n</pre>{pages}");
        assert_eq!(html_to_text(&cover), format!("Cover. {}\n{text}", words.trim_end()));
        // Without page numbers, the page tag ends one page, and no header
        // repeats at the top of five pages.
        let unnumbered: String = (1..9)
            .map(|n| {
                let page = page(n).replace(&format!("<p>{n}</p>"), "");
                if n == 4 { page + "<pre>\n<PAGE>\n</pre>" } else { page }
            })
            .collect();
        assert_eq!(html_to_text(&unnumbered).matches("ACME CORP").count(), 8);
    }

    /// A bare number that ends a `<pre>` element, alone in it or on its last
    /// line, is a page number when the one before or after it among such
    /// numbers counts the pages with it, as in a deck of slides each in
    /// `<pre>` elements, or when a page break follows it. The running
    /// headers of those pages then go too. An amount typed in a `<pre>` of
    /// its own between them is table data.
    #[test]
    fn page_numbers_that_end_a_pre() {
        let words = "and words that fill the slide. ".repeat(40);
        // A name over each slide: a heading there would go only where the
        // slides break inside sentences (see `numbered_pages`).
        let slide = |n: u32, extra: &str| {
            format!("<pre>Acme Corporation</pre><pre>Slide {n} has text of its own, {words}</pre>{extra}<pre>{n}</pre>")
        };
        // The numbers skip 5: "4" and "6" still count the pages.
        let deck: String =
            [1, 2, 3, 4, 6, 7, 8].map(|n| slide(n, if n == 4 { "<pre>$</pre><pre>790</pre>" } else { "" })).concat();
        let text = tables_of(&deck).0;
        // As the same slides in paragraphs without the amount.
        let flowed =
            html_to_text(&deck.replace("<pre>790</pre>", "").replace("<pre>", "<p>").replace("</pre>", "</p>"));
        assert_eq!(text, flowed.replace("$\n", "$\n790\n"));
        assert_eq!(text.matches("Acme Corporation").count(), 1, "{text}");
        assert!(text.lines().all(|l| l.parse::<u32>().is_err() || l == "790"), "{text}");
        // The last line of a longer `<pre>`, with blank lines around it, in
        // roman numerals too.
        let pages: String = ["i", "ii", "iii"]
            .iter()
            .map(|n| format!("<pre>Text of page {n}.\nMore text.\n\n   {n}\n\n</pre><hr>"))
            .collect();
        let text = "Text of page i.\nMore text.\nText of page ii.\nMore text.\nText of page iii.\nMore text.\n";
        assert_eq!(html_to_text(&pages), text);
        // A number alone that counts with no other, as the end of a typed
        // table, stays, as do numbers written two ways or the same number
        // twice.
        let html = "<p>Units:</p><pre>12</pre><p>Text.</p><pre>Total\n   40</pre>";
        assert_eq!(html_to_text(html), "Units:\n12\nText.\nTotal\n40\n");
        let html = "<pre>Text.\n\n   ii\n</pre><p>Text.</p><pre>3</pre><p>Text.</p><pre>3</pre>";
        assert_eq!(html_to_text(html), "Text.\nii\nText.\n3\nText.\n3\n");
        // One that a page break follows goes, on its own, but not one with
        // text after it on the page.
        let broken = "<pre>Text.\n\n   17\n</pre><div style=\"page-break-before: always\"><p>Next.</p></div>";
        assert_eq!(html_to_text(broken), "Text.\nNext.\n");
        let later = broken.replace("<div", "<p>More.</p><div");
        assert_eq!(html_to_text(&later), "Text.\n17\nMore.\nNext.\n");
        // In table cells it is table data.
        let cells: String = (1..4).map(|n| format!("<tr><td><pre>Text {n}.\n  {n}</pre></td></tr>")).collect();
        let html = format!("<table>{cells}</table>");
        assert_eq!(html_to_text(&html), "Text 1.\n1\nText 2.\n2\nText 3.\n3\n");
        // A number in a `<pre>` of several lines that is not the last stays.
        assert_eq!(
            html_to_text("<pre>7\nText.</pre><pre>8\nText.</pre><pre>9\nText.</pre>"),
            "7\nText.\n8\nText.\n9\nText.\n"
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

    /// The second line of a header of two lines goes where the first goes.
    /// A note under a name or a heading on each of its pages stays with it
    /// over the first page of each run and goes with it from the others, and
    /// so does a third line under the second.
    #[test]
    fn second_lines_of_running_headers_go_with_the_first() {
        let count =
            |pages: &[Vec<String>], line: &str| html_to_text(&printed(pages)).lines().filter(|l| *l == line).count();
        let (acme, notes) = ("Acme Corporation", "Notes to Financial Statements");
        for (first, second) in [
            (acme, "(A Wholly-Owned Subsidiary of Beta, Inc.)"),
            ("Acme Insurance Company of Ohio", "(A Wholly-Owned Subsidiary of Beta, Inc.)"),
            ("ACME BANCSHARES, INC.,", "AND SUBSIDIARY"),
            (acme, "[FORMERLY BETA FINISHERS, INC.]"),
            (notes, "(Unaudited)"),
        ] {
            let pages = framed(&[first, second], &[], 1..13);
            assert_eq!((count(&pages, first), count(&pages, second)), (1, 1), "{second}");
            let text = html_to_text(&printed(&pages));
            assert!(text.starts_with(&format!("{first}\n{second}\nText of page 1.\nText of page 2.\n")), "{text}");
            // At the foot of the pages too.
            let pages = framed(&[], &[first, second], 1..13);
            assert_eq!((count(&pages, first), count(&pages, second)), (1, 1), "{second}");
            // Each run of pages keeps its first copy of both.
            let mut pages = framed(&[first, second], &[], 1..7);
            pages.extend(framed(&[], &[], 7..8));
            pages.extend(framed(&[first, second], &[], 8..14));
            assert_eq!((count(&pages, first), count(&pages, second)), (2, 2), "{second}");
        }
        // A third line goes with the second, and a header that says it
        // continues goes from every page under them.
        let (second, third) = ("AND SUBSIDIARY", "(A Development Stage Company)");
        let pages = framed(&[acme, second, third, "Notes (Continued)"], &[], 1..13);
        let text = html_to_text(&printed(&pages));
        assert!(text.starts_with(&format!("{acme}\n{second}\n{third}\nText of page 1.\nText of page 2.\n")), "{text}");
        assert_eq!((count(&pages, acme), count(&pages, second), count(&pages, third)), (1, 1, 1));
        // The same note under two names goes with each.
        let mut pages = framed(&[acme, third], &[], 1..7);
        pages.extend(framed(&["Beta Power LLC", third], &[], 7..13));
        assert_eq!((count(&pages, acme), count(&pages, "Beta Power LLC"), count(&pages, third)), (1, 1, 2));
        // The first page of the name's run has no second line: the first
        // copy of the second line stays.
        let mut pages = framed(&[acme], &[], 1..2);
        pages.extend(framed(&[acme, third], &[], 2..13));
        assert_eq!((count(&pages, acme), count(&pages, third)), (1, 1));
        assert!(html_to_text(&printed(&pages)).contains(&format!("Text of page 1.\n{third}\nText of page 2.\n")));
    }

    /// A note under a name on only some of the name's pages qualifies those
    /// pages and stays on each: fewer than nine in ten. So does a note under
    /// a header that says it continues, which goes from every page, a note
    /// under a legend, and a legend under the name on every page.
    #[test]
    fn notes_over_some_pages_of_a_running_header_stay() {
        let count =
            |pages: &[Vec<String>], line: &str| html_to_text(&printed(pages)).lines().filter(|l| *l == line).count();
        let acme = "Acme Corporation";
        for note in ["(Unaudited)", "(Debtor-in-Possession)", "(A Development Stage Company)", "AND SUBSIDIARIES"] {
            // Five of twelve pages, in the middle of the run of the name.
            let mut pages = framed(&[acme], &[], 1..5);
            pages.extend(framed(&[acme, note], &[], 5..10));
            pages.extend(framed(&[acme], &[], 10..13));
            assert_eq!((count(&pages, acme), count(&pages, note)), (1, 5), "{note}");
            // Seventeen of twenty pages stay, eighteen go with the name.
            for (with, left) in [(17, 17), (18, 1)] {
                let mut pages = framed(&[acme, note], &[], 1..with + 1);
                pages.extend(framed(&[acme], &[], with + 1..21));
                assert_eq!((count(&pages, acme), count(&pages, note)), (1, left), "{note} {with}");
            }
            // Under another line on most of the name's pages.
            let mut pages = framed(&[acme, "Schedule II", note], &[], 1..7);
            pages.extend(framed(&[acme, note], &[], 7..13));
            assert_eq!((count(&pages, acme), count(&pages, "Schedule II"), count(&pages, note)), (1, 1, 7), "{note}");
        }
        // Under a header that says it continues, and under a legend.
        let (continued, date) = ("Notes to Financial Statements (Continued)", "December 31, 2024 and 2023");
        let pages = framed(&[acme, continued, "(Unaudited)"], &[], 1..13);
        assert_eq!((count(&pages, acme), count(&pages, continued), count(&pages, "(Unaudited)")), (1, 0, 12));
        let pages = framed(&[acme, date, "(Unaudited)"], &[], 1..13);
        assert_eq!((count(&pages, acme), count(&pages, date), count(&pages, "(Unaudited)")), (1, 12, 12));
        // A legend under the name on every page.
        for legend in ["(Parent Company Only)", "(In millions)", "(Registrant Only)"] {
            let pages = framed(&[acme, legend], &[], 1..13);
            assert_eq!((count(&pages, acme), count(&pages, legend)), (1, 12), "{legend}");
        }
    }

    /// The pages of a line and the pages where a note is under it are
    /// counted as pages, not as copies: a name at the top and at the foot of
    /// a page is on that page once, and so is a note under both.
    #[test]
    fn second_lines_counted_by_page_not_by_copy() {
        let count =
            |pages: &[Vec<String>], line: &str| html_to_text(&printed(pages)).lines().filter(|l| *l == line).count();
        let (acme, note) = ("Acme Corporation", "(A Development Stage Company)");
        // The note under the name at the top only, or at the foot only: under
        // half of the name's copies, and under the name on each of its pages.
        for pages in [framed(&[acme, note], &[acme], 1..13), framed(&[acme], &[acme, note], 1..13)] {
            assert_eq!((count(&pages, acme), count(&pages, note)), (1, 1));
        }
        // The note under the name at the top and at the foot of six of its
        // twelve pages: as many copies as the name has pages, on half of them.
        let mut pages = framed(&[acme, note], &[acme, note], 1..7);
        pages.extend(framed(&[acme], &[acme], 7..13));
        assert_eq!((count(&pages, acme), count(&pages, note)), (1, 12));
    }

    /// The text of `pages` pages, page `n` made of "Text of page n." under
    /// the lines of `top` on the pages of `held`.
    fn headed(top: &[&str], held: &[u32], pages: u32) -> String {
        let pages: Vec<Vec<String>> = (1..=pages)
            .map(|n| {
                let top = if held.contains(&n) { top } else { &[] };
                top.iter().map(|l| l.to_string()).chain([format!("Text of page {n}.")]).collect()
            })
            .collect();
        html_to_text(&printed(&pages))
    }

    /// A heading or a name printed on every other page, as on facing pages,
    /// is one run from five pages on: it stays over the first and goes from
    /// the others, and the second line of the header with it.
    #[test]
    fn headers_on_every_other_page_of_six_stay_once() {
        let count = |text: &str, line: &str| text.lines().filter(|l| *l == line).count();
        for header in ["Notes to Consolidated Financial Statements", "Acme Corporation"] {
            let text = headed(&[header], &[1, 3, 5, 7, 9, 11], 12);
            assert_eq!(count(&text, header), 1, "{text}");
            let start = format!("{header}\nText of page 1.\nText of page 2.\nText of page 3.\n");
            assert!(text.starts_with(&start), "{text}");
            // Five pages are a chain too, and it need not start the filing.
            let text = headed(&[header], &[4, 6, 8, 10, 12], 14);
            assert_eq!(count(&text, header), 1, "{text}");
            assert!(text.contains(&format!("Text of page 3.\n{header}\nText of page 4.\nText of page 5.\n")), "{text}");
            // Two chains, each with its first copy.
            let text = headed(&[header], &[1, 3, 5, 7, 9, 11, 16, 18, 20, 22, 24, 26], 27);
            assert_eq!(count(&text, header), 2, "{text}");
            assert!(text.contains(&format!("Text of page 15.\n{header}\nText of page 16.\n")), "{text}");
        }
        let (notes, note) = ("Notes to Consolidated Financial Statements", "(Unaudited)");
        let text = headed(&[notes, note], &[1, 3, 5, 7, 9, 11], 12);
        assert_eq!((count(&text, notes), count(&text, note)), (1, 1), "{text}");
    }

    /// Three copies two pages apart are first copies, as a name over three
    /// statements is: each stays.
    #[test]
    fn headers_on_every_other_page_of_three_stay() {
        let count = |text: &str, line: &str| text.lines().filter(|l| *l == line).count();
        for header in ["Notes to Consolidated Financial Statements", "Acme Corporation"] {
            // Two chains of three, four pages apart.
            let text = headed(&[header], &[1, 3, 5, 9, 11, 13], 14);
            assert_eq!(count(&text, header), 6, "{text}");
            // A chain of three after a run of four pages.
            let text = headed(&[header], &[1, 2, 3, 4, 7, 9, 11], 12);
            assert_eq!(count(&text, header), 4, "{text}");
        }
    }

    /// Nor are four copies two pages apart one run, also when a fifth page
    /// two pages before or after them holds the text next to a page that
    /// holds it: a page with the text on the page next to it is not one of
    /// a chain.
    #[test]
    fn headers_on_every_other_page_of_four_stay() {
        let count = |text: &str, line: &str| text.lines().filter(|l| *l == line).count();
        for header in ["Notes to Consolidated Financial Statements", "Acme Corporation"] {
            // Two chains of four, five pages apart, and one of four with a
            // fifth copy far from it.
            let text = headed(&[header], &[1, 3, 5, 7, 12, 14, 16, 18], 19);
            assert_eq!(count(&text, header), 8, "{text}");
            let text = headed(&[header], &[1, 3, 5, 7, 12], 13);
            assert_eq!(count(&text, header), 5, "{text}");
            // Pages 1 and 2, then every other page to 10: page 2 goes on
            // with the run of page 1, and 4 to 10 are a chain of four.
            let text = headed(&[header], &[1, 2, 4, 6, 8, 10], 11);
            assert_eq!(count(&text, header), 5, "{text}");
            // Every other page from 1 to 9, then page 10, which goes on
            // with the run of page 9: 1 to 7 are a chain of four.
            let text = headed(&[header], &[1, 3, 5, 7, 9, 10], 11);
            assert_eq!(count(&text, header), 5, "{text}");
        }
    }

    /// With five pages left beside a page that has the text on the page
    /// next to it, the five are a chain: its first copy stays two pages
    /// after a run of two pages, and the page two after its last starts the
    /// run of the page next to it.
    #[test]
    fn chains_of_every_other_page_end_before_a_page_next_to_one_with_the_text() {
        let count = |text: &str, line: &str| text.lines().filter(|l| *l == line).count();
        for header in ["Notes to Consolidated Financial Statements", "Acme Corporation"] {
            // Pages 1 and 2, then every other page to 12: 4 to 12 are a
            // chain of five, and the copies of pages 1 and 4 stay.
            let text = headed(&[header], &[1, 2, 4, 6, 8, 10, 12], 13);
            assert_eq!(count(&text, header), 2, "{text}");
            assert!(text.contains(&format!("Text of page 3.\n{header}\nText of page 4.\nText of page 5.\n")), "{text}");
            // Every other page from 1 to 11, then page 12: 1 to 9 are a
            // chain of five, and the copies of pages 1 and 11 stay.
            let text = headed(&[header], &[1, 3, 5, 7, 9, 11, 12], 13);
            assert_eq!(count(&text, header), 2, "{text}");
            let run = format!("Text of page 10.\n{header}\nText of page 11.\nText of page 12.\n");
            assert!(text.contains(&run), "{text}");
        }
    }

    /// Pages three apart are no chain, alone or after pages two apart.
    #[test]
    fn headers_on_every_third_page_stay() {
        let count = |text: &str, line: &str| text.lines().filter(|l| *l == line).count();
        for header in ["Notes to Consolidated Financial Statements", "Acme Corporation"] {
            let text = headed(&[header], &[1, 4, 7, 10, 13, 16], 17);
            assert_eq!(count(&text, header), 6, "{text}");
            // Two chains of three, three pages apart.
            let text = headed(&[header], &[1, 3, 5, 8, 10, 12], 13);
            assert_eq!(count(&text, header), 6, "{text}");
        }
    }

    /// A page that holds the text at its top and at its foot counts once
    /// in a chain, and a chain is the chain of one text: a heading under
    /// the name on every other page of the name's chain stays on each.
    #[test]
    fn chains_of_every_other_page_count_pages_of_one_text() {
        let count = |text: &str, line: &str| text.lines().filter(|l| *l == line).count();
        let (acme, notes) = ("Acme Corporation", "Notes to Consolidated Financial Statements");
        for header in [notes, acme] {
            let pages: Vec<Vec<String>> = (1..13)
                .map(|n| {
                    let text = format!("Text of page {n}.");
                    if n % 2 == 1 { vec![header.to_string(), text, header.to_string()] } else { vec![text] }
                })
                .collect();
            let text = html_to_text(&printed(&pages));
            assert_eq!(count(&text, header), 1, "{text}");
            assert!(text.starts_with(&format!("{header}\nText of page 1.\nText of page 2.\n")), "{text}");
        }
        let pages: Vec<Vec<String>> = (1..21)
            .map(|n| {
                let top: &[&str] = match n % 4 {
                    1 => &[acme],
                    3 => &[acme, notes],
                    _ => &[],
                };
                top.iter().map(|l| l.to_string()).chain([format!("Text of page {n}.")]).collect()
            })
            .collect();
        let text = html_to_text(&printed(&pages));
        assert_eq!((count(&text, acme), count(&text, notes)), (1, 5), "{text}");
    }

    /// A note wrapped over two lines stays whole. Its first line opens a
    /// parenthesis or a bracket and does not close it, and stays where the
    /// line under it in the block stays. A note closed on its line goes with
    /// the line above it all the same, and the rule reads notes only.
    #[test]
    fn first_lines_of_wrapped_notes_stay_with_the_rest() {
        let count =
            |pages: &[Vec<String>], line: &str| html_to_text(&printed(pages)).lines().filter(|l| *l == line).count();
        let (acme, notes) = ("Acme Corporation", "Notes to Combined Financial Statements");
        let date = "December 31, 2024 and 2023";
        // The rest of the note starts with the period it covers, so it is a
        // legend and stays on every page.
        for (first, rest) in [
            ("(Information as of and for the Nine Months", "Ended September 30, 1997 is unaudited)"),
            ("[Information as of and for the Nine Months", "Ended September 30, 1997 is unaudited]"),
        ] {
            let pages = framed(&[notes, first, rest], &[], 1..13);
            assert_eq!((count(&pages, notes), count(&pages, first), count(&pages, rest)), (1, 12, 12), "{first}");
            // A first line that stays leaves the lines above it as they are.
            let pages = framed(&[acme, date, first, rest], &[], 1..13);
            let copies = [acme, date, first, rest].map(|line| count(&pages, line));
            assert_eq!(copies, [1, 12, 12, 12], "{first}");
        }
        // The rest goes with the first line, and both stay over the first
        // page; with nothing under it in the block the first line goes.
        let (first, rest) = ("(A Wholly-Owned Subsidiary of", "Beta Holding Corporation)");
        let pages = framed(&[acme, first, rest], &[], 1..13);
        assert_eq!((count(&pages, acme), count(&pages, first), count(&pages, rest)), (1, 1, 1));
        let pages = framed(&[acme, first], &[], 1..13);
        assert_eq!((count(&pages, acme), count(&pages, first)), (1, 1));
        // A note closed on its line goes over a legend.
        for note in ["(Unaudited)", "(A Development Stage Company)", "[Formerly Beta Inc.]", "AND SUBSIDIARIES"] {
            let pages = framed(&[acme, note, date], &[], 1..13);
            assert_eq!((count(&pages, acme), count(&pages, note), count(&pages, date)), (1, 1, 12), "{note}");
        }
        // A heading that opens a parenthesis is no note: it goes as a
        // heading does.
        let (heading, rest) = ("Notes to Financial Statements (Information for", "September 30, 1997 is unaudited)");
        let pages = framed(&[heading, rest], &[], 1..13);
        assert_eq!((count(&pages, heading), count(&pages, rest)), (1, 12));
    }

    /// Units given as "000", "$000" or "000’s" in a parenthetical are a
    /// legend, which stays on every page. A parenthetical that gives no
    /// units is a note and goes with the heading above it, and a "000"
    /// outside the parenthesis that starts the line makes no legend.
    #[test]
    fn units_given_as_000_in_parentheses_are_a_legend() {
        let count =
            |pages: &[Vec<String>], line: &str| html_to_text(&printed(pages)).lines().filter(|l| *l == line).count();
        let notes = "Notes to Consolidated Financial Statements";
        for legend in [
            "(Tabular amounts in US$ 000’s, except share and per share data)",
            "($000, except share/unit data)",
            "(000’s, except per share data)",
            "(Amounts in $000's)",
            "(in 000s)",
            "(US$000)",
        ] {
            let pages = framed(&[notes, legend], &[], 1..13);
            assert_eq!((count(&pages, notes), count(&pages, legend)), (1, 12), "{legend}");
        }
        for note in [
            "(Unaudited)",
            "(A Development Stage Company)",
            "(Formed in 2000)",
            "(10,000,000 shares authorized)",
            "(Exchange ratio of 1.000 to 1)",
            "(Commission File No. 000-12345)",
        ] {
            let pages = framed(&[notes, note], &[], 1..13);
            assert_eq!((count(&pages, notes), count(&pages, note)), (1, 1), "{note}");
        }
        for header in ["Acme Portfolio 000 LLC", "(Restated) Schedule 000"] {
            let pages = framed(&[header], &[], 1..13);
            assert_eq!(count(&pages, header), 1, "{header}");
        }
    }
}
