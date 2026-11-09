//! Place another tool's section text in our filing text.
//!
//! Both texts are reduced to lowercase ASCII letters and digits, which removes
//! differences in spacing, line breaks and table layout. A section starts at
//! the line of our text where its opening characters appear and the text after
//! them agrees best with the section; a heading that also appears in the table
//! of contents is told apart that way. It ends where its last characters
//! appear. Page headers and footers that the tool keeps and our text drops
//! are stepped over, at the section's start too.

use std::sync::LazyLock;

use regex::Regex;

/// How many normalized characters to match at each end.
const WINDOW: usize = 120;
/// Opening characters looked up at line starts.
const HEAD: usize = 24;
/// How many pieces of the section, and of what size, are compared after a
/// possible start.
const CHUNKS: usize = 50;
const CHUNK: usize = 16;
/// Text found in only one of the two versions (a page footer) that the
/// comparison steps over.
const SLACK: usize = 120;

/// A line that only names a Part ("PART II", "Part II - Other Information").
static PART_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^part\s+(?:i{1,3}|iv|[1-4])\b").unwrap());
static ITEM_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^item\s*\d").unwrap());
/// How many leading lines of a section can be skipped, and how long they can be.
const SKIP_LINES: usize = 4;
const SKIP_LEN: usize = 80;

pub struct Aligner {
    norm: String,
    /// Byte offset in the original text of each normalized character.
    map: Vec<u32>,
    /// Line number of each normalized character.
    line: Vec<u32>,
    len: usize,
}

fn norm_chars(s: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    s.char_indices().filter(|(_, c)| c.is_ascii_alphanumeric()).map(|(i, c)| (i, c.to_ascii_lowercase()))
}

fn normalize(s: &str) -> String {
    norm_chars(s).map(|(_, c)| c).collect()
}

impl Aligner {
    pub fn new(text: &str) -> Self {
        let mut norm = String::with_capacity(text.len());
        let mut map = Vec::with_capacity(text.len());
        let mut line = Vec::with_capacity(text.len());
        let mut n = 0u32;
        for (i, c) in text.char_indices() {
            if c == '\n' {
                n += 1;
            } else if c.is_ascii_alphanumeric() {
                norm.push(c.to_ascii_lowercase());
                map.push(i as u32);
                line.push(n);
            }
        }
        Self { norm, map, line, len: text.len() }
    }

    /// The section without leading Part lines and page headers. Some tools put
    /// a Part line in the first Item of the Part, or start an Item at the page
    /// header above its heading; our labels start at the Item heading. A page
    /// header is a short line that our text drops, so a section that starts
    /// with one has opening characters found nowhere in our text. Item
    /// headings are never skipped.
    fn strip_leading_lines<'a>(&self, section: &'a str) -> &'a str {
        let mut rest = section.trim_start();
        for _ in 0..SKIP_LINES {
            let (first, after) = rest.split_once('\n').unwrap_or((rest, ""));
            let first = first.trim();
            if first.len() > SKIP_LEN || ITEM_LINE.is_match(first) {
                break;
            }
            let part = PART_LINE.is_match(first) && !first.to_ascii_lowercase().contains("item");
            if !part {
                let head: String = norm_chars(rest).map(|(_, c)| c).take(HEAD).collect();
                if head.is_empty() || self.norm.contains(&head) {
                    break;
                }
            }
            rest = after.trim_start();
        }
        rest
    }

    /// Byte span of a section's text, or None when its start cannot be found.
    pub fn place(&self, section: &str) -> Option<(usize, usize)> {
        let section = self.strip_leading_lines(section);
        let n = normalize(section);
        if n.len() < 5 {
            return None;
        }
        let first_line = section.lines().map(normalize).find(|l| !l.is_empty()).unwrap_or_default();
        let start = self.start_at_line(&n, &first_line).or_else(|| self.start_by_window(&n))?;
        // End: the last window found after the start.
        let mut end = None;
        for back in [0, 20, 60, 150, 400, 1000] {
            if back + 8 > n.len() {
                break;
            }
            let hi = n.len() - back;
            let w = &n[hi.saturating_sub(WINDOW)..hi];
            if let Some(p) = self.norm[start..].find(w) {
                end = Some(start + p + w.len() + back);
                break;
            }
        }
        let end = end.unwrap_or(start + n.len()).min(self.norm.len());
        let byte_start = self.map[start] as usize;
        let byte_end = if end >= self.map.len() { self.len } else { self.map[end] as usize };
        Some((byte_start, byte_end.max(byte_start)))
    }

    /// The line start where the section's opening characters appear and the
    /// following text agrees best; on a tie, the later one (the body comes
    /// after the table of contents).
    fn start_at_line(&self, n: &str, first_line: &str) -> Option<usize> {
        let head = &n[..n.len().min(HEAD)];
        let mut best: Option<(usize, usize)> = None;
        let mut from = 0;
        while let Some(p) = self.norm[from..].find(head) {
            let o = from + p;
            from = o + 1;
            if o > 0 && self.line[o - 1] == self.line[o] {
                continue;
            }
            let score = 2 * usize::from(self.whole_line(o, first_line)) + self.agreement(n, o);
            if best.is_none_or(|(s, _)| score >= s) {
                best = Some((score, o));
            }
        }
        best.map(|(_, o)| o)
    }

    /// Whether our line starting at `o` is exactly the section's first line.
    fn whole_line(&self, o: usize, first_line: &str) -> bool {
        let end = o + first_line.len();
        self.norm.get(o..end) == Some(first_line) && self.line.get(end).is_none_or(|&l| l != self.line[o])
    }

    /// How many of the section's opening pieces appear in order after `o`.
    fn agreement(&self, n: &str, o: usize) -> usize {
        let mut cur = o;
        let mut hits = 0;
        for chunk in n.as_bytes().chunks(CHUNK).take(CHUNKS) {
            // Both strings are ASCII.
            let chunk = std::str::from_utf8(chunk).unwrap_or_default();
            let hi = (cur + chunk.len() + SLACK).min(self.norm.len());
            if let Some(p) = self.norm[cur..hi].find(chunk) {
                hits += 1;
                cur += p + chunk.len();
            }
        }
        hits
    }

    /// Fallback when no line starts with the section's opening characters:
    /// the first window found, trying later windows when the tool's heading
    /// differs from the filing's.
    fn start_by_window(&self, n: &str) -> Option<usize> {
        for skip in [0, 20, 60, 150, 400, 1000] {
            if skip >= n.len() {
                break;
            }
            let w = &n[skip..(skip + WINDOW).min(n.len())];
            if w.len() < 8 {
                break;
            }
            if let Some(p) = self.norm.find(w) {
                return Some(p.saturating_sub(skip));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_reformatted_text() {
        let text = "TOC\nItem 1. Business 3\nItem 1. Business\nWe make widgets in many places around the world.\nItem 2. Properties\nWe own a plant.\n";
        let a = Aligner::new(text);
        let (s, e) = a.place("ITEM 1.  BUSINESS We make widgets in many places\naround the world.").unwrap();
        assert_eq!(&text[s..e], "Item 1. Business\nWe make widgets in many places around the world.\n");
        assert!(a.place("Nothing like this appears here").is_none());
    }

    #[test]
    fn short_item_placed_in_body_not_contents() {
        let text = "Contents\nItem 5. Market\nItem 6. [Reserved]\nItem 7. Discussion\n\
                    Item 5. Market\nOur shares trade on an exchange.\nItem 6. [Reserved]\nItem 7. Discussion\nSales grew.\n";
        let a = Aligner::new(text);
        let (s, _) = a.place("Item 6. [Reserved]").unwrap();
        assert!(text[s..].starts_with("Item 6. [Reserved]\nItem 7. Discussion\nSales"));
    }

    #[test]
    fn page_header_before_heading_skipped() {
        let text = "Item 8. Statements\nNumbers.\nItem 9A. Controls and Procedures.\nOur controls work well enough.\n";
        let a = Aligner::new(text);
        let tool = "Acme Corp   65\n\nTable of Contents\nItem 9A. Controls and Procedures.\nOur controls work";
        let (s, _) = a.place(tool).unwrap();
        assert!(text[s..].starts_with("Item 9A."));
        // A line that is in our text belongs to the section.
        let (s, _) = a.place("Numbers.\nItem 9A. Controls and Procedures.\nOur controls work").unwrap();
        assert!(text[s..].starts_with("Numbers."));
        // So does a heading cut short: the start is in the heading's line.
        let (s, _) = a.place("ontrols and\nProcedures.\nOur controls work well").unwrap();
        let line_start = text[..s].rfind('\n').map_or(0, |i| i + 1);
        assert!(text[line_start..].starts_with("Item 9A."));
    }

    #[test]
    fn part_line_and_page_footer_in_tool_text() {
        let contents: String = (1..=12).map(|i| format!("Item {i}. Heading number {i}\n")).collect();
        let text = format!(
            "{contents}Item 4. Mine Safety Disclosures\nNot applicable.\nItem 5. Market for Common Equity\n\
             Our common stock trades on the New York Stock Exchange under the symbol XYZ.\n"
        );
        let a = Aligner::new(&text);
        let tool = "PART II\n\nItem 5. Market for Common Equity\n\n34 Acme Corp 2023 Form 10-K\n\n\
                    Our common stock trades on the New York Stock Exchange under the symbol XYZ.";
        let (s, e) = a.place(tool).unwrap();
        assert!(text[s..e].starts_with("Item 5. Market for Common Equity\nOur common stock"));
        assert!(text[s..e].trim_end().ends_with("symbol XYZ."));
    }
}
