//! Shared source text position helpers.
//!
//! The parser and lexer report diagnostics in byte offsets, while humans and editor protocols need
//! line/column coordinates. This crate keeps that mapping in one place without depending on LSP
//! types or parser internals.

/// A one-based human-readable source position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

impl LineColumn {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A zero-based LSP-style position whose character offset is measured in UTF-16 code units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Utf16Position {
    pub line: u32,
    pub character: u32,
}

impl Utf16Position {
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// Maps byte offsets into a source string to line/column coordinates.
pub struct LineIndex<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self { text, line_starts }
    }

    /// Return the one-based human-readable line and column for `offset`.
    ///
    /// Columns are counted in Unicode scalar values, matching Rust `char`s. Out-of-range offsets
    /// clamp to the document end; offsets in the middle of a UTF-8 sequence clamp back to the
    /// previous character boundary.
    pub fn line_column(&self, offset: usize) -> LineColumn {
        let offset = self.clamp_offset(offset);
        let line = self.line_for_offset(offset);
        let line_start = self.line_starts[line];
        let column = self.text[line_start..offset].chars().count() + 1;
        LineColumn::new(line + 1, column)
    }

    /// Return the zero-based line and UTF-16 column for `offset`.
    ///
    /// This is the coordinate system used by LSP. Out-of-range offsets clamp to the document end;
    /// offsets in the middle of a UTF-8 sequence clamp back to the previous character boundary.
    pub fn utf16_position(&self, offset: usize) -> Utf16Position {
        let offset = self.clamp_offset(offset);
        let line = self.line_for_offset(offset);
        let line_start = self.line_starts[line];
        let character = utf16_len(&self.text[line_start..offset]);
        Utf16Position::new(line as u32, character)
    }

    /// The UTF-16 position one past the last character.
    pub fn end_utf16_position(&self) -> Utf16Position {
        self.utf16_position(self.text.len())
    }

    /// The byte offset for a zero-based line and UTF-16 column.
    ///
    /// Out-of-range lines and columns clamp to the line or document end. A UTF-16 column landing in
    /// the middle of a surrogate pair maps to the start of that character.
    pub fn offset_for_utf16_position(&self, position: Utf16Position) -> usize {
        let line = position.line as usize;
        let Some(&line_start) = self.line_starts.get(line) else {
            return self.text.len();
        };
        let mut remaining = position.character as usize;
        let mut offset = line_start;
        for ch in self.text[line_start..].chars() {
            let width = ch.len_utf16();
            if remaining < width || ch == '\n' {
                break;
            }
            remaining -= width;
            offset += ch.len_utf8();
        }
        offset
    }

    fn line_for_offset(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        }
    }

    fn clamp_offset(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

/// Count UTF-16 code units in `text`.
pub fn utf16_len(text: &str) -> u32 {
    text.chars().map(|ch| ch.len_utf16() as u32).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_offsets_to_one_based_line_columns() {
        let text = "abc\ndefg\nhi";
        let index = LineIndex::new(text);
        assert_eq!(index.line_column(0), LineColumn::new(1, 1));
        assert_eq!(index.line_column(4), LineColumn::new(2, 1));
        assert_eq!(index.line_column(6), LineColumn::new(2, 3));
        assert_eq!(index.line_column(999), LineColumn::new(3, 3));
    }

    #[test]
    fn maps_offsets_to_utf16_positions() {
        let text = "SELECT a\nFROM 芋;\n";
        let index = LineIndex::new(text);
        assert_eq!(index.utf16_position(0), Utf16Position::new(0, 0));
        assert_eq!(index.utf16_position(7), Utf16Position::new(0, 7));
        assert_eq!(
            index.utf16_position(text.find("FROM").unwrap()),
            Utf16Position::new(1, 0)
        );
        assert_eq!(
            index.utf16_position(text.find(';').unwrap()),
            Utf16Position::new(1, 6)
        );
    }

    #[test]
    fn maps_utf16_positions_back_to_offsets() {
        let text = "SELECT a\nFROM 芋;\nSELECT 😀;\n";
        let index = LineIndex::new(text);
        for offset in [
            0usize,
            7,
            text.find("FROM").unwrap(),
            text.find(';').unwrap(),
            text.find("😀").unwrap(),
        ] {
            assert_eq!(
                index.offset_for_utf16_position(index.utf16_position(offset)),
                offset
            );
        }
    }

    #[test]
    fn clamps_mid_character_offsets() {
        let text = "a😀b";
        let index = LineIndex::new(text);
        assert_eq!(index.line_column(2), LineColumn::new(1, 2));
        assert_eq!(index.utf16_position(2), Utf16Position::new(0, 1));
    }
}
