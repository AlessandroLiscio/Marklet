//! Bytes to text.
//!
//! The order is fixed and deliberate: **BOM sniff → UTF-8 validate →
//! Windows-1252 fallback**. It covers virtually every real-world `.md` on a
//! Western machine at zero binary cost. Universal (CJK) detection with
//! `encoding_rs` + `chardetng` is roughly +500 KB and lives behind the
//! `full-encodings` cargo feature; it is deliberately *not* part of this module.
//!
//! Nothing here may panic on file content. A file that lies about its encoding
//! produces a [`DecodeError`], never an `unwrap`.

use std::borrow::Cow;
use std::fmt;

use super::Encoding;

/// A file whose bytes contradict the encoding it declared.
///
/// Only a *declared* encoding can fail: a BOM is a promise, and a file that
/// breaks it is malformed rather than merely unusual. Everything without a BOM
/// falls through to Windows-1252, which is total and therefore infallible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A UTF-8 BOM was present but the remaining bytes are not valid UTF-8.
    InvalidUtf8 { offset: usize },
    /// A UTF-16 BOM was present but the byte length is odd.
    TruncatedUtf16,
    /// A UTF-16 BOM was present but the code units contain an unpaired surrogate.
    InvalidUtf16,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::InvalidUtf8 { offset } => {
                write!(f, "UTF-8 BOM present but byte {offset} is not valid UTF-8")
            }
            DecodeError::TruncatedUtf16 => {
                f.write_str("UTF-16 BOM present but the file has an odd number of bytes")
            }
            DecodeError::InvalidUtf16 => {
                f.write_str("UTF-16 BOM present but the text contains an unpaired surrogate")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

const BOM_UTF8: &[u8] = &[0xEF, 0xBB, 0xBF];
const BOM_UTF16LE: &[u8] = &[0xFF, 0xFE];
const BOM_UTF16BE: &[u8] = &[0xFE, 0xFF];

/// Decodes `bytes` into text, reporting which path was taken.
///
/// Borrows when the input is already UTF-8, which is the overwhelmingly common
/// case — a 5 MB document costs no copy at all on that path.
pub fn decode(bytes: &[u8]) -> Result<(Cow<'_, str>, Encoding), DecodeError> {
    if let Some(rest) = bytes.strip_prefix(BOM_UTF8) {
        return match std::str::from_utf8(rest) {
            Ok(s) => Ok((Cow::Borrowed(s), Encoding::Utf8Bom)),
            Err(e) => Err(DecodeError::InvalidUtf8 {
                offset: BOM_UTF8.len() + e.valid_up_to(),
            }),
        };
    }
    if let Some(rest) = bytes.strip_prefix(BOM_UTF16LE) {
        return utf16(rest, u16::from_le_bytes).map(|s| (Cow::Owned(s), Encoding::Utf16Le));
    }
    if let Some(rest) = bytes.strip_prefix(BOM_UTF16BE) {
        return utf16(rest, u16::from_be_bytes).map(|s| (Cow::Owned(s), Encoding::Utf16Be));
    }

    match std::str::from_utf8(bytes) {
        Ok(s) => Ok((Cow::Borrowed(s), Encoding::Utf8)),
        // No BOM and not UTF-8: Windows-1252 is a total mapping, so this branch
        // always succeeds. It is a guess, which is exactly why the caller is
        // told about it through `RenderedDoc::encoding`.
        Err(_) => Ok((Cow::Owned(windows_1252(bytes)), Encoding::Windows1252)),
    }
}

/// Decodes as UTF-8 as far as possible, replacing what it cannot.
///
/// Used only as the last resort behind [`super::render`], which must not fail.
pub fn decode_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn utf16(rest: &[u8], to_u16: fn([u8; 2]) -> u16) -> Result<String, DecodeError> {
    if rest.len() % 2 != 0 {
        return Err(DecodeError::TruncatedUtf16);
    }
    let units: Vec<u16> = rest.chunks_exact(2).map(|c| to_u16([c[0], c[1]])).collect();
    String::from_utf16(&units).map_err(|_| DecodeError::InvalidUtf16)
}

/// The 32 code points where Windows-1252 differs from Latin-1, `0x80..=0x9F`.
///
/// The five bytes Microsoft left undefined (`0x81`, `0x8D`, `0x8F`, `0x90`,
/// `0x9D`) map to the C1 control of the same value, matching the WHATWG index
/// and therefore matching what `encoding_rs` would do if the `full-encodings`
/// feature were on. Two builds of Marklet must not disagree about a file.
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

fn windows_1252(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            0x00..=0x7F => out.push(b as char),
            0x80..=0x9F => out.push(CP1252_HIGH[(b - 0x80) as usize]),
            // 0xA0..=0xFF is Latin-1, which is code-point-identical.
            _ => out.push(b as char),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_utf8_is_borrowed() {
        let (text, enc) = decode(b"# hi").expect("valid utf-8");
        assert!(matches!(text, Cow::Borrowed(_)));
        assert_eq!(enc, Encoding::Utf8);
        assert_eq!(text, "# hi");
    }

    #[test]
    fn utf8_bom_is_stripped() {
        let mut v = BOM_UTF8.to_vec();
        v.extend_from_slice(b"# hi");
        let (text, enc) = decode(&v).expect("valid utf-8");
        assert_eq!(enc, Encoding::Utf8Bom);
        assert_eq!(text, "# hi");
    }

    #[test]
    fn utf16le_bom_round_trips() {
        let mut v = BOM_UTF16LE.to_vec();
        for u in "# hé".encode_utf16() {
            v.extend_from_slice(&u.to_le_bytes());
        }
        let (text, enc) = decode(&v).expect("valid utf-16");
        assert_eq!(enc, Encoding::Utf16Le);
        assert_eq!(text, "# hé");
    }

    #[test]
    fn utf16be_bom_round_trips() {
        let mut v = BOM_UTF16BE.to_vec();
        for u in "# hé".encode_utf16() {
            v.extend_from_slice(&u.to_be_bytes());
        }
        let (text, enc) = decode(&v).expect("valid utf-16");
        assert_eq!(enc, Encoding::Utf16Be);
        assert_eq!(text, "# hé");
    }

    #[test]
    fn cp1252_smart_quotes() {
        // 0x93 / 0x94 are curly double quotes in cp1252 and invalid UTF-8.
        let (text, enc) = decode(b"\x93quoted\x94 \x97 dash").expect("cp1252 is total");
        assert_eq!(enc, Encoding::Windows1252);
        assert_eq!(text, "\u{201C}quoted\u{201D} \u{2014} dash");
    }

    #[test]
    fn odd_length_utf16_is_an_error_not_a_panic() {
        let mut v = BOM_UTF16LE.to_vec();
        v.push(0x41);
        assert_eq!(decode(&v), Err(DecodeError::TruncatedUtf16));
    }

    #[test]
    fn lying_utf8_bom_is_an_error_not_a_panic() {
        let mut v = BOM_UTF8.to_vec();
        v.push(0xFF);
        assert!(matches!(decode(&v), Err(DecodeError::InvalidUtf8 { .. })));
    }

    #[test]
    fn unpaired_surrogate_is_an_error() {
        let mut v = BOM_UTF16LE.to_vec();
        v.extend_from_slice(&0xD800u16.to_le_bytes());
        v.extend_from_slice(&0x0041u16.to_le_bytes());
        assert_eq!(decode(&v), Err(DecodeError::InvalidUtf16));
    }
}
