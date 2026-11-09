//! Byte decoding for EDGAR documents.
//!
//! Most modern filings are UTF-8 or ASCII with HTML entities, but many older
//! filings are Windows-1252 (curly quotes and dashes in the 0x80-0x9F range).

/// Windows-1252 code points for bytes 0x80..=0x9F. Undefined bytes map to the
/// C1 control with the same value, as browsers do.
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{02C6}',
    '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}', '\u{0090}', '\u{2018}',
    '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}',
    '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/// Decode bytes as UTF-8 when valid, otherwise as Windows-1252.
pub fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes
            .iter()
            .map(|&b| match b {
                0x80..=0x9F => CP1252_HIGH[(b - 0x80) as usize],
                _ => b as char,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_passthrough() {
        assert_eq!(decode("Company’s".as_bytes()), "Company’s");
    }

    #[test]
    fn cp1252_fallback() {
        assert_eq!(decode(b"Company\x92s \x93risk\x94 \x97 caf\xe9"), "Company’s “risk” — café");
    }
}
