//! Place another tool's section text in our filing text.
//!
//! Both texts are reduced to lowercase ASCII letters and digits, which removes
//! differences in spacing, line breaks and table layout. The section's first
//! characters give its start and its last characters give its end.

/// How many normalized characters to match at each end.
const WINDOW: usize = 120;

pub struct Aligner {
    norm: String,
    /// Byte offset in the original text of each normalized character.
    map: Vec<u32>,
    len: usize,
}

fn norm_chars(s: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    s.char_indices().filter(|(_, c)| c.is_ascii_alphanumeric()).map(|(i, c)| (i, c.to_ascii_lowercase()))
}

impl Aligner {
    pub fn new(text: &str) -> Self {
        let mut norm = String::with_capacity(text.len());
        let mut map = Vec::with_capacity(text.len());
        for (i, c) in norm_chars(text) {
            norm.push(c);
            map.push(i as u32);
        }
        Self { norm, map, len: text.len() }
    }

    /// Byte span of a section's text, or None when its start cannot be found.
    pub fn place(&self, section: &str) -> Option<(usize, usize)> {
        let n: String = norm_chars(section).map(|(_, c)| c).collect();
        if n.len() < 8 {
            return None;
        }
        // Start: the first window found, trying later windows when the
        // tool's heading differs from the filing's.
        let mut start = None;
        for skip in [0, 20, 60, 150, 400, 1000] {
            if skip >= n.len() {
                break;
            }
            let w = &n[skip..(skip + WINDOW).min(n.len())];
            if w.len() < 8 {
                break;
            }
            if let Some(p) = self.norm.find(w) {
                start = Some(p.saturating_sub(skip));
                break;
            }
        }
        let start = start?;
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
}
