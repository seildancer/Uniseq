use super::SpanError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceSpan {
    start: usize,
    end: usize,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize) -> Result<Self, SpanError> {
        if end < start {
            return Err(SpanError::EndBeforeStart { start, end });
        }

        Ok(Self { start, end })
    }

    pub fn unchecked(start: usize, end: usize) -> Self {
        debug_assert!(start <= end);
        Self { start, end }
    }

    pub fn start(self) -> usize {
        self.start
    }

    pub fn end(self) -> usize {
        self.end
    }

    pub fn len(self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub fn contains_offset(self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    pub fn contains_span(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    pub fn slice<'a>(self, text: &'a str) -> Result<&'a str, SpanError> {
        self.validate_for_text(text)?;
        Ok(&text[self.start..self.end])
    }

    pub fn validate_for_text(self, text: &str) -> Result<(), SpanError> {
        if self.end > text.len() {
            return Err(SpanError::OutOfBounds {
                span_end: self.end,
                text_len: text.len(),
            });
        }

        if !text.is_char_boundary(self.start) {
            return Err(SpanError::NotUtf8Boundary { offset: self.start });
        }

        if !text.is_char_boundary(self.end) {
            return Err(SpanError::NotUtf8Boundary { offset: self.end });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_span_uses_utf8_byte_offsets() {
        let text = "aéz";
        let span = SourceSpan::new(1, 3).unwrap();

        assert_eq!(span.slice(text).unwrap(), "é");
        assert_eq!(span.len(), 2);
    }

    #[test]
    fn rejects_invalid_utf8_boundaries() {
        let text = "aéz";
        let span = SourceSpan::new(1, 2).unwrap();

        assert_eq!(
            span.slice(text).unwrap_err(),
            SpanError::NotUtf8Boundary { offset: 2 }
        );
    }

    #[test]
    fn rejects_end_before_start() {
        assert_eq!(
            SourceSpan::new(5, 4).unwrap_err(),
            SpanError::EndBeforeStart { start: 5, end: 4 }
        );
    }
}
