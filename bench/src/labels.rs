//! Label files and the line index used to resolve them.
//!
//! A label names a line by its text and its occurrence among lines with the
//! same text, plus the start of the line after it as a check. Lines are
//! compared after dropping whitespace and case, so labels survive spacing
//! changes in html_to_text. No byte offsets are stored.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parsec_edgar::FormKind;
use serde::{Deserialize, Serialize};

use crate::Res;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LabelFile {
    pub accession: String,
    pub cik: u64,
    pub company: String,
    pub form: String,
    pub filing_date: String,
    /// Document the benchmark reads (the primary document, or the full submission for old filings).
    pub url: String,
    /// Filing agent: self, DFIN, Workiva, Toppan Merrill, ...
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// False until the Items are labeled.
    #[serde(default)]
    pub labeled: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// Items with a body heading, in document order.
    #[serde(default)]
    pub items: Vec<ItemLabel>,
    /// Items with no body heading (left out, or only listed in a cross-reference index).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub absent: Vec<String>,
    /// Where the last Item ends, when that is not the end of the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<LineRef>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ItemLabel {
    pub id: String,
    /// The heading line.
    #[serde(flatten)]
    pub heading: LineRef,
    /// The body only says the answer is incorporated by reference, not applicable, or none.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub incorporated: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub uncertain: bool,
    /// Where this Item ends, when that is not the next Item's heading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<LineRef>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LineRef {
    /// The line as printed.
    pub text: String,
    /// Index among lines with the same normalized text, from 0.
    #[serde(default)]
    pub occurrence: usize,
    /// Start of the following line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// Lowercase, no whitespace, plain quotes and dashes.
pub fn normalize(s: &str) -> String {
    norm_chars(s).map(|(_, c)| c).collect()
}

/// Normalized chars with the byte offset of the source char.
fn norm_chars(s: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    s.char_indices()
        .filter(|(_, c)| !c.is_whitespace() && !matches!(c, '\u{200b}' | '\u{ad}' | '\u{feff}'))
        .map(|(i, c)| {
            let c = match c {
                '‘' | '’' | '`' => '\'',
                '“' | '”' => '"',
                '–' | '—' | '‑' => '-',
                c => c,
            };
            (i, c)
        })
        .flat_map(|(i, c)| c.to_lowercase().map(move |l| (i, l)))
}

/// Filing text split into lines, with an index from normalized line text to line numbers.
pub struct Doc {
    pub text: String,
    starts: Vec<usize>,
    chars: Vec<usize>,
    occ: Vec<u32>,
    index: HashMap<String, Vec<usize>>,
    /// Whole text normalized, with the byte offset of each normalized char; built on demand.
    flat: OnceLock<(String, Vec<usize>)>,
}

impl Doc {
    pub fn new(text: String) -> Self {
        let mut starts = Vec::new();
        let mut chars = Vec::new();
        let mut occ = Vec::new();
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut pos = 0;
        let mut cpos = 0;
        for (i, line) in text.split_inclusive('\n').enumerate() {
            starts.push(pos);
            chars.push(cpos);
            let v = index.entry(normalize(line)).or_default();
            occ.push(v.len() as u32);
            v.push(i);
            pos += line.len();
            cpos += line.chars().count();
        }
        Self { text, starts, chars, occ, index, flat: OnceLock::new() }
    }

    pub fn lines(&self) -> usize {
        self.starts.len()
    }

    pub fn start(&self, i: usize) -> usize {
        self.starts[i]
    }

    pub fn line(&self, i: usize) -> &str {
        let end = self.starts.get(i + 1).copied().unwrap_or(self.text.len());
        self.text[self.starts[i]..end].trim_end_matches('\n')
    }

    pub fn occurrence(&self, i: usize) -> usize {
        self.occ[i] as usize
    }

    /// Line containing a byte offset.
    pub fn line_of(&self, byte: usize) -> usize {
        self.starts.partition_point(|&s| s <= byte).saturating_sub(1)
    }

    pub fn line_start_of(&self, byte: usize) -> usize {
        if self.starts.is_empty() { 0 } else { self.starts[self.line_of(byte)] }
    }

    /// Character offset of a byte offset.
    pub fn char_of(&self, byte: usize) -> usize {
        if byte >= self.text.len() {
            return self.chars.last().copied().unwrap_or(0) + self.text[self.starts.last().copied().unwrap_or(0)..].chars().count();
        }
        let l = self.line_of(byte);
        let mut b = byte;
        while !self.text.is_char_boundary(b) {
            b -= 1;
        }
        self.chars[l] + self.text[self.starts[l]..b].chars().count()
    }

    pub fn line_ref(&self, i: usize) -> Res<LineRef> {
        if i >= self.lines() {
            return Err(format!("line {i} is past the end ({} lines)", self.lines()));
        }
        let next = (i + 1 < self.lines()).then(|| self.line(i + 1).chars().take(40).collect::<String>());
        Ok(LineRef { text: self.line(i).to_owned(), occurrence: self.occurrence(i), next })
    }

    /// Line number of a label.
    pub fn find(&self, r: &LineRef) -> Res<usize> {
        let empty = Vec::new();
        let cands = self.index.get(&normalize(&r.text)).unwrap_or(&empty);
        let want_next = r.next.as_deref().map(normalize);
        let next_ok = |i: usize| match &want_next {
            None => true,
            Some(n) => i + 1 < self.lines() && normalize(self.line(i + 1)).starts_with(n.as_str()),
        };
        if let Some(&i) = cands.get(r.occurrence)
            && next_ok(i)
        {
            return Ok(i);
        }
        let good: Vec<usize> = cands.iter().copied().filter(|&i| next_ok(i)).collect();
        match (good.len(), cands.get(r.occurrence)) {
            (1, _) => Ok(good[0]),
            // The following line changed: fall back to the occurrence.
            (0, Some(&i)) => Ok(i),
            _ => Err(format!("{:?} #{} ({} matching lines)", r.text, r.occurrence, cands.len())),
        }
    }

    /// Byte offset of a label. When no whole line matches (a text conversion
    /// that joins lines), the heading followed by the start of its next line
    /// is searched inside lines instead.
    pub fn find_pos(&self, r: &LineRef) -> Res<usize> {
        match self.find(r) {
            Ok(line) => Ok(self.start(line)),
            Err(e) => {
                let (flat, map) = self.flat.get_or_init(|| {
                    let mut flat = String::new();
                    let mut map = Vec::new();
                    for (i, c) in norm_chars(&self.text) {
                        flat.push(c);
                        map.extend(std::iter::repeat_n(i, c.len_utf8()));
                    }
                    (flat, map)
                });
                let head = normalize(&r.text);
                let with_next = head.clone() + &r.next.as_deref().map(normalize).unwrap_or_default();
                let hits: Vec<usize> = flat.match_indices(with_next.as_str()).map(|(i, _)| i).collect();
                let hits = if hits.is_empty() {
                    flat.match_indices(head.as_str()).map(|(i, _)| i).collect()
                } else {
                    hits
                };
                let i = match hits.len() {
                    0 => return Err(e),
                    1 => hits[0],
                    _ => *hits.get(r.occurrence).unwrap_or(&hits[0]),
                };
                Ok(map[i])
            }
        }
    }
}

pub struct TrueItem {
    pub id: String,
    /// Byte offset of the heading (a line start unless lines were joined).
    pub pos: usize,
    /// Line holding the heading.
    pub line: usize,
    /// Byte offset where the Item ends.
    pub end: usize,
}

pub struct Truth {
    pub items: Vec<TrueItem>,
    /// Byte offset where the last Item ends, when labeled.
    pub end_pos: Option<usize>,
}

/// Resolve labels to positions and spans. An Item runs to its own end label,
/// else to the next labeled heading, else to the filing's end label or the end of the text.
pub fn resolve(doc: &Doc, label: &LabelFile) -> Res<Truth> {
    let end_pos = label.end.as_ref().map(|e| doc.find_pos(e)).transpose().map_err(|e| format!("end: {e}"))?;
    let mut items: Vec<(usize, &ItemLabel)> = Vec::new();
    for it in &label.items {
        let pos = doc.find_pos(&it.heading).map_err(|e| format!("{}: {e}", it.id))?;
        items.push((pos, it));
    }
    items.sort_by_key(|(p, _)| *p);
    let doc_end = end_pos.unwrap_or(doc.text.len());
    let mut out = Vec::new();
    for (n, (pos, it)) in items.iter().enumerate() {
        let end = match &it.end {
            Some(e) => doc.find_pos(e).map_err(|e| format!("{} end: {e}", it.id))?,
            None => items.get(n + 1).map_or(doc_end, |(p, _)| *p),
        };
        if end <= *pos {
            return Err(format!("{} ends before it starts", it.id));
        }
        out.push(TrueItem { id: it.id.clone(), pos: *pos, line: doc.line_of(*pos), end });
    }
    Ok(Truth { items: out, end_pos })
}

/// Section id from a short form: `1A` (10-K), `II1A` (10-Q), `2.02` (8-K), or a full id.
pub fn item_id(kind: FormKind, s: &str) -> Res<String> {
    let s = s.trim().to_ascii_lowercase();
    if s.starts_with("item_") || s.starts_with("part_") {
        return Ok(s);
    }
    let bad = || format!("bad item {s:?} for {kind:?}");
    match kind {
        FormKind::TenK => {
            let ok = s.len() <= 3 && s.chars().next().is_some_and(|c| c.is_ascii_digit());
            if ok { Ok(format!("item_{s}")) } else { Err(bad()) }
        }
        FormKind::TenQ => {
            let (part, item) = if let Some(r) = s.strip_prefix("ii") {
                (2, r)
            } else if let Some(r) = s.strip_prefix('i') {
                (1, r)
            } else {
                return Err(bad());
            };
            if item.is_empty() { Err(bad()) } else { Ok(format!("part_{part}_item_{item}")) }
        }
        FormKind::EightK => {
            if s.contains('.') { Ok(format!("item_{s}")) } else { Err(bad()) }
        }
    }
}

pub fn save(path: &Path, label: &LabelFile) -> Res<()> {
    let s = serde_json::to_string_pretty(label).map_err(|e| e.to_string())?;
    std::fs::write(path, s + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

fn load(path: &Path) -> Res<LabelFile> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every labeled filing, oldest first, optionally filtered by accession, company or form.
pub fn load_all(dir: &Path, filter: Option<&str>) -> Res<Vec<(PathBuf, LabelFile)>> {
    let mut out = Vec::new();
    let mut unlabeled = 0;
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in entries {
        let path = e.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let l = load(&path)?;
        if let Some(f) = filter {
            let f = f.to_ascii_lowercase();
            let hay = format!("{} {} {} {}", l.accession, l.company, l.form, l.filing_date).to_ascii_lowercase();
            if !hay.contains(&f) {
                continue;
            }
        }
        if l.labeled {
            out.push((path, l));
        } else {
            unlabeled += 1;
        }
    }
    if unlabeled > 0 {
        eprintln!("skipping {unlabeled} filings without labels");
    }
    out.sort_by(|a, b| (&a.1.filing_date, &a.1.accession).cmp(&(&b.1.filing_date, &b.1.accession)));
    Ok(out)
}

/// The label file whose accession contains `acc`.
pub fn find(dir: &Path, acc: &str) -> Res<(PathBuf, LabelFile)> {
    let mut hits = Vec::new();
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = e.map_err(|e| e.to_string())?.path();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if path.extension().is_some_and(|x| x == "json") && stem.contains(acc) {
            hits.push(path);
        }
    }
    match hits.len() {
        1 => {
            let p = hits.remove(0);
            let l = load(&p)?;
            Ok((p, l))
        }
        0 => Err(format!("no label file matches {acc}")),
        n => Err(format!("{n} label files match {acc}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_by_occurrence_and_next_line() {
        let doc = Doc::new("Item 1. Business\nItem 1A. Risk\nItem 1. Business\nWe make things.\n".into());
        let r = doc.line_ref(2).unwrap();
        assert_eq!(r.occurrence, 1);
        assert_eq!(doc.find(&r).unwrap(), 2);
        // Spacing and case changes still resolve.
        let r2 = LineRef { text: "ITEM 1.  BUSINESS".into(), occurrence: 1, next: Some("We make".into()) };
        assert_eq!(doc.find(&r2).unwrap(), 2);
        // A wrong occurrence is repaired by the next line when that is unique.
        let r3 = LineRef { text: "Item 1. Business".into(), occurrence: 0, next: Some("We make".into()) };
        assert_eq!(doc.find(&r3).unwrap(), 2);
    }

    #[test]
    fn spans_end_at_next_heading_or_end_label() {
        let doc = Doc::new("A\nItem 1. Business\nx\nItem 2. Properties\ny\nSIGNATURES\nz\n".into());
        let mut l = LabelFile::default();
        l.items.push(ItemLabel { id: "item_1".into(), heading: doc.line_ref(1).unwrap(), ..ItemLabel::default() });
        l.items.push(ItemLabel { id: "item_2".into(), heading: doc.line_ref(3).unwrap(), ..ItemLabel::default() });
        l.end = Some(doc.line_ref(5).unwrap());
        let t = resolve(&doc, &l).unwrap();
        assert_eq!(t.items[0].end, doc.start(3));
        assert_eq!(t.items[1].end, doc.start(5));
        assert_eq!(doc.char_of(doc.text.len()), doc.text.chars().count());
    }

    #[test]
    fn find_pos_inside_joined_lines() {
        // Another conversion joined the heading onto the previous line.
        let doc = Doc::new("Intro text. ITEM 7. MD&A\nResults improved.\n".into());
        let r = LineRef { text: "Item 7. MD&A".into(), occurrence: 0, next: Some("Results".into()) };
        assert!(doc.find(&r).is_err());
        assert_eq!(doc.find_pos(&r).unwrap(), "Intro text. ".len());
    }

    #[test]
    fn short_ids() {
        assert_eq!(item_id(FormKind::TenK, "1A").unwrap(), "item_1a");
        assert_eq!(item_id(FormKind::TenQ, "II1A").unwrap(), "part_2_item_1a");
        assert_eq!(item_id(FormKind::TenQ, "I2").unwrap(), "part_1_item_2");
        assert_eq!(item_id(FormKind::EightK, "2.02").unwrap(), "item_2.02");
    }
}
