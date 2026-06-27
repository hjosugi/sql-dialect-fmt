//! Encoding boundary for formatter inputs.
//!
//! The lexer/parser operate on UTF-8 `&str`, but the CLI sees arbitrary bytes.
//! This crate keeps that boundary explicit: known Unicode encodings are decoded
//! losslessly, while unknown or malformed byte streams remain opaque so tools do
//! not accidentally rewrite data they do not understand.
//!
//! The entry point is [`DecodedText::decode`], which sniffs a BOM, decodes when it can, and
//! otherwise keeps the original bytes intact. Edit the recovered text with [`DecodedText::map_text`]
//! and round-trip back to bytes with [`DecodedText::encode`]; the original encoding (and any BOM) is
//! preserved end to end.
//!
//! ```
//! use sql_dialect_fmt_encoding::{DecodedText, TextEncoding};
//! let decoded = DecodedText::decode("select 1\n".as_bytes());
//! assert_eq!(decoded.encoding(), TextEncoding::Utf8);
//! let upper = decoded.map_text(|t| t.to_uppercase());
//! assert_eq!(upper.encode(), b"SELECT 1\n");
//! ```

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

/// A text encoding that [`DecodedText`] can recognize and round-trip.
///
/// `#[non_exhaustive]`: more encodings may be added in future releases, so match with a wildcard
/// arm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TextEncoding {
    /// UTF-8 with no byte-order mark.
    Utf8,
    /// UTF-8 prefixed with a byte-order mark, which is preserved on re-encoding.
    Utf8Bom,
    /// Little-endian UTF-16 (identified by its byte-order mark).
    Utf16Le,
    /// Big-endian UTF-16 (identified by its byte-order mark).
    Utf16Be,
    /// Bytes that could not be decoded as text and are passed through verbatim.
    OpaqueBytes,
}

/// The result of decoding a byte stream: either recovered text in a known [`TextEncoding`], or the
/// original bytes preserved verbatim because they could not be decoded.
///
/// Construct one with [`DecodedText::decode`]; it never loses data and [`DecodedText::encode`]
/// reproduces the input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedText {
    kind: DecodedKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedKind {
    Text {
        encoding: TextEncoding,
        text: String,
    },
    Opaque {
        bytes: Vec<u8>,
        reason: OpaqueReason,
    },
}

/// Why a byte stream was kept opaque instead of being decoded as text.
///
/// `#[non_exhaustive]`: more failure reasons may be added in future releases, so match with a
/// wildcard arm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OpaqueReason {
    /// The bytes were not valid UTF-8.
    InvalidUtf8,
    /// A UTF-16 stream had an odd number of bytes, so it could not be split into 16-bit units.
    OddLengthUtf16,
    /// The 16-bit units did not form valid UTF-16 (for example an unpaired surrogate).
    InvalidUtf16,
}

impl DecodedText {
    /// Decode `bytes`, sniffing a leading byte-order mark to pick the encoding. Valid UTF-8 or
    /// UTF-16 is recovered as text; anything that fails to decode is preserved verbatim with an
    /// [`OpaqueReason`]. Never fails and never loses data.
    pub fn decode(bytes: &[u8]) -> Self {
        if bytes.starts_with(UTF8_BOM) {
            return decode_utf8(&bytes[UTF8_BOM.len()..], TextEncoding::Utf8Bom, bytes);
        }
        if bytes.starts_with(UTF16_LE_BOM) {
            return decode_utf16(&bytes[UTF16_LE_BOM.len()..], TextEncoding::Utf16Le, bytes);
        }
        if bytes.starts_with(UTF16_BE_BOM) {
            return decode_utf16(&bytes[UTF16_BE_BOM.len()..], TextEncoding::Utf16Be, bytes);
        }
        decode_utf8(bytes, TextEncoding::Utf8, bytes)
    }

    /// The encoding this text was decoded as, or [`TextEncoding::OpaqueBytes`] when the input could
    /// not be decoded.
    pub fn encoding(&self) -> TextEncoding {
        match &self.kind {
            DecodedKind::Text { encoding, .. } => *encoding,
            DecodedKind::Opaque { .. } => TextEncoding::OpaqueBytes,
        }
    }

    /// The decoded text, or `None` when the input was kept opaque (see [`Self::opaque_reason`]).
    pub fn as_str(&self) -> Option<&str> {
        match &self.kind {
            DecodedKind::Text { text, .. } => Some(text),
            DecodedKind::Opaque { .. } => None,
        }
    }

    /// Why the input was kept opaque, or `None` when it decoded as text.
    pub fn opaque_reason(&self) -> Option<OpaqueReason> {
        match &self.kind {
            DecodedKind::Text { .. } => None,
            DecodedKind::Opaque { reason, .. } => Some(*reason),
        }
    }

    /// Re-encode back to bytes in the original encoding (re-adding any BOM). For opaque input this
    /// returns the preserved original bytes, so decode-then-encode is always a faithful round-trip.
    pub fn encode(&self) -> Vec<u8> {
        match &self.kind {
            DecodedKind::Text { encoding, text } => encode_text(*encoding, text),
            DecodedKind::Opaque { bytes, .. } => bytes.clone(),
        }
    }

    /// Apply `edit` to the decoded text, keeping the original encoding. Opaque input is returned
    /// unchanged, so transformations never run on bytes that could not be understood as text.
    pub fn map_text(&self, edit: impl FnOnce(&str) -> String) -> Self {
        match &self.kind {
            DecodedKind::Text { encoding, text } => DecodedText {
                kind: DecodedKind::Text {
                    encoding: *encoding,
                    text: edit(text),
                },
            },
            DecodedKind::Opaque { .. } => self.clone(),
        }
    }
}

fn decode_utf8(bytes: &[u8], encoding: TextEncoding, original: &[u8]) -> DecodedText {
    match std::str::from_utf8(bytes) {
        Ok(text) => DecodedText {
            kind: DecodedKind::Text {
                encoding,
                text: text.to_owned(),
            },
        },
        Err(_) => opaque(original, OpaqueReason::InvalidUtf8),
    }
}

fn decode_utf16(bytes: &[u8], encoding: TextEncoding, original: &[u8]) -> DecodedText {
    if !bytes.len().is_multiple_of(2) {
        return opaque(original, OpaqueReason::OddLengthUtf16);
    }

    let words = bytes.chunks_exact(2).map(|chunk| match encoding {
        TextEncoding::Utf16Le => u16::from_le_bytes([chunk[0], chunk[1]]),
        TextEncoding::Utf16Be => u16::from_be_bytes([chunk[0], chunk[1]]),
        _ => unreachable!("decode_utf16 is only called for UTF-16 encodings"),
    });

    match String::from_utf16(&words.collect::<Vec<_>>()) {
        Ok(text) => DecodedText {
            kind: DecodedKind::Text { encoding, text },
        },
        Err(_) => opaque(original, OpaqueReason::InvalidUtf16),
    }
}

fn encode_text(encoding: TextEncoding, text: &str) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut bytes = Vec::with_capacity(UTF8_BOM.len() + text.len());
            bytes.extend_from_slice(UTF8_BOM);
            bytes.extend_from_slice(text.as_bytes());
            bytes
        }
        TextEncoding::Utf16Le => {
            let mut bytes = Vec::with_capacity(UTF16_LE_BOM.len() + text.len() * 2);
            bytes.extend_from_slice(UTF16_LE_BOM);
            for word in text.encode_utf16() {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes
        }
        TextEncoding::Utf16Be => {
            let mut bytes = Vec::with_capacity(UTF16_BE_BOM.len() + text.len() * 2);
            bytes.extend_from_slice(UTF16_BE_BOM);
            for word in text.encode_utf16() {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            bytes
        }
        TextEncoding::OpaqueBytes => unreachable!("opaque values are encoded from original bytes"),
    }
}

fn opaque(bytes: &[u8], reason: OpaqueReason) -> DecodedText {
    DecodedText {
        kind: DecodedKind::Opaque {
            bytes: bytes.to_vec(),
            reason,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_without_bom_round_trips() {
        let bytes = "SELECT '長芋';\n".as_bytes();
        let decoded = DecodedText::decode(bytes);

        assert_eq!(decoded.encoding(), TextEncoding::Utf8);
        assert_eq!(decoded.as_str(), Some("SELECT '長芋';\n"));
        assert_eq!(decoded.encode(), bytes);
    }

    #[test]
    fn utf8_bom_round_trips_and_preserves_bom() {
        let mut bytes = UTF8_BOM.to_vec();
        bytes.extend_from_slice("SELECT 1;\n".as_bytes());

        let decoded = DecodedText::decode(&bytes);

        assert_eq!(decoded.encoding(), TextEncoding::Utf8Bom);
        assert_eq!(decoded.as_str(), Some("SELECT 1;\n"));
        assert_eq!(decoded.encode(), bytes);
    }

    #[test]
    fn utf16_le_round_trips_with_unicode() {
        let text = "SELECT '長芋';\n";
        let bytes = encode_text(TextEncoding::Utf16Le, text);

        let decoded = DecodedText::decode(&bytes);

        assert_eq!(decoded.encoding(), TextEncoding::Utf16Le);
        assert_eq!(decoded.as_str(), Some(text));
        assert_eq!(decoded.encode(), bytes);
    }

    #[test]
    fn utf16_be_round_trips_with_unicode() {
        let text = "SELECT '長芋';\n";
        let bytes = encode_text(TextEncoding::Utf16Be, text);

        let decoded = DecodedText::decode(&bytes);

        assert_eq!(decoded.encoding(), TextEncoding::Utf16Be);
        assert_eq!(decoded.as_str(), Some(text));
        assert_eq!(decoded.encode(), bytes);
    }

    #[test]
    fn opaque_invalid_utf8_preserves_original_bytes() {
        let bytes = [0x53, 0x45, 0xFF, 0x4C];
        let decoded = DecodedText::decode(&bytes);

        assert_eq!(decoded.encoding(), TextEncoding::OpaqueBytes);
        assert_eq!(decoded.as_str(), None);
        assert_eq!(decoded.opaque_reason(), Some(OpaqueReason::InvalidUtf8));
        assert_eq!(decoded.encode(), bytes);
    }

    #[test]
    fn opaque_invalid_utf16_preserves_original_bytes() {
        let bytes = [0xFF, 0xFE, 0x00];
        let decoded = DecodedText::decode(&bytes);

        assert_eq!(decoded.encoding(), TextEncoding::OpaqueBytes);
        assert_eq!(decoded.opaque_reason(), Some(OpaqueReason::OddLengthUtf16));
        assert_eq!(decoded.encode(), bytes);
    }

    #[test]
    fn map_text_preserves_original_encoding() {
        let source = encode_text(TextEncoding::Utf16Le, "select 1\n");
        let decoded = DecodedText::decode(&source);

        let edited = decoded.map_text(|text| text.to_uppercase());

        assert_eq!(edited.encoding(), TextEncoding::Utf16Le);
        assert_eq!(
            DecodedText::decode(&edited.encode()).as_str(),
            Some("SELECT 1\n")
        );
    }
}
