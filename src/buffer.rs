use std::error::Error;
use std::fmt;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Insert { at: usize, text: String },
    Replace { range: Range<usize>, text: String },
    Delete { range: Range<usize> },
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditError {
    OutOfBounds,
    NotCharBoundary,
    ReversedRange,
    EmptyRange,
    Overlap,
    DuplicateInsert,
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::OutOfBounds => "edit offset is out of bounds",
            Self::NotCharBoundary => "edit offset is not a UTF-8 character boundary",
            Self::ReversedRange => "edit range is reversed",
            Self::EmptyRange => "edit range is empty",
            Self::Overlap => "edits overlap",
            Self::DuplicateInsert => "multiple insertions share an offset",
        };
        formatter.write_str(message)
    }
}

impl Error for EditError {}

pub struct ShellBuffer {
    original: String,
}

impl ShellBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            original: text.into(),
        }
    }

    pub fn text(&self) -> &str {
        &self.original
    }

    pub fn apply(&self, edits: &[Edit]) -> Result<String, EditError> {
        let mut ranges = Vec::new();
        let mut inserts = Vec::new();

        for edit in edits {
            match edit {
                Edit::Insert { at, text } => {
                    self.validate_offset(*at)?;
                    inserts.push((*at, text.as_str()));
                }
                Edit::Replace { range, text } => {
                    self.validate_range(range)?;
                    ranges.push((range, text.as_str()));
                }
                Edit::Delete { range } => {
                    self.validate_range(range)?;
                    ranges.push((range, ""));
                }
            }
        }

        ranges.sort_by_key(|(range, _)| range.start);
        inserts.sort_by_key(|(at, _)| *at);

        for pair in ranges.windows(2) {
            if pair[0].0.end > pair[1].0.start {
                return Err(EditError::Overlap);
            }
        }

        for pair in inserts.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(EditError::DuplicateInsert);
            }
        }

        for (at, _) in &inserts {
            if ranges
                .iter()
                .any(|(range, _)| range.start < *at && *at < range.end)
            {
                return Err(EditError::Overlap);
            }
        }

        let mut result = String::with_capacity(self.original.len());
        let mut cursor = 0;
        let mut next_insert = 0;

        for (range, replacement) in ranges {
            while next_insert < inserts.len() && inserts[next_insert].0 <= range.start {
                let (at, text) = inserts[next_insert];
                result.push_str(&self.original[cursor..at]);
                result.push_str(text);
                cursor = at;
                next_insert += 1;
            }

            result.push_str(&self.original[cursor..range.start]);
            result.push_str(replacement);
            cursor = range.end;
        }

        while next_insert < inserts.len() {
            let (at, text) = inserts[next_insert];
            result.push_str(&self.original[cursor..at]);
            result.push_str(text);
            cursor = at;
            next_insert += 1;
        }

        result.push_str(&self.original[cursor..]);
        Ok(result)
    }

    fn validate_offset(&self, offset: usize) -> Result<(), EditError> {
        if offset > self.original.len() {
            return Err(EditError::OutOfBounds);
        }
        if !self.original.is_char_boundary(offset) {
            return Err(EditError::NotCharBoundary);
        }
        Ok(())
    }

    fn validate_range(&self, range: &Range<usize>) -> Result<(), EditError> {
        self.validate_offset(range.start)?;
        self.validate_offset(range.end)?;
        if range.start > range.end {
            return Err(EditError::ReversedRange);
        }
        if range.start == range.end {
            return Err(EditError::EmptyRange);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Edit, EditError, ShellBuffer};
    use std::ops::Range;

    #[test]
    fn appends_at_the_original_end() {
        let buffer = ShellBuffer::new("echo hi");

        assert_eq!(
            buffer.apply(&[Edit::Insert {
                at: buffer.text().len(),
                text: " && exit".into(),
            }]),
            Ok("echo hi && exit".into())
        );
    }

    #[test]
    fn inserts_at_a_unicode_boundary() {
        let buffer = ShellBuffer::new("aé🙂z");

        assert_eq!(
            buffer.apply(&[Edit::Insert {
                at: 1,
                text: "✓".into(),
            }]),
            Ok("a✓é🙂z".into())
        );
    }

    #[test]
    fn applies_sorted_and_unsorted_non_overlapping_edits() {
        let buffer = ShellBuffer::new("abcdef");
        let sorted = vec![
            Edit::Insert {
                at: 1,
                text: "!".into(),
            },
            Edit::Delete { range: 2..3 },
            Edit::Replace {
                range: 4..6,
                text: "YZ".into(),
            },
        ];
        let unsorted = vec![sorted[2].clone(), sorted[0].clone(), sorted[1].clone()];

        assert_eq!(buffer.apply(&sorted), Ok("a!bdYZ".into()));
        assert_eq!(buffer.apply(&unsorted), Ok("a!bdYZ".into()));
    }

    #[test]
    fn preserves_untouched_injection_shaped_text_exactly() {
        let buffer = ShellBuffer::new("echo \"$(whoami)\" `date`\n  value='${HOME}'\n");

        assert_eq!(
            buffer.apply(&[Edit::Replace {
                range: 0..4,
                text: "printf".into(),
            }]),
            Ok("printf \"$(whoami)\" `date`\n  value='${HOME}'\n".into())
        );
    }

    #[test]
    fn rejects_invalid_edits() {
        let ascii = ShellBuffer::new("abcd");
        let unicode = ShellBuffer::new("é");

        assert_eq!(
            ascii.apply(&[Edit::Insert {
                at: 5,
                text: "x".into(),
            }]),
            Err(EditError::OutOfBounds)
        );
        assert_eq!(
            unicode.apply(&[Edit::Insert {
                at: 1,
                text: "x".into(),
            }]),
            Err(EditError::NotCharBoundary)
        );
        assert_eq!(
            ascii.apply(&[Edit::Delete {
                range: Range { start: 3, end: 1 },
            }]),
            Err(EditError::ReversedRange)
        );
        assert_eq!(
            ascii.apply(&[Edit::Replace {
                range: 1..1,
                text: "x".into(),
            }]),
            Err(EditError::EmptyRange)
        );
        assert_eq!(
            ascii.apply(&[Edit::Delete { range: 1..1 }]),
            Err(EditError::EmptyRange)
        );
        assert_eq!(
            ascii.apply(&[
                Edit::Delete { range: 1..3 },
                Edit::Replace {
                    range: 2..4,
                    text: "x".into(),
                },
            ]),
            Err(EditError::Overlap)
        );
        assert_eq!(
            ascii.apply(&[
                Edit::Insert {
                    at: 1,
                    text: "x".into(),
                },
                Edit::Insert {
                    at: 1,
                    text: "y".into(),
                },
            ]),
            Err(EditError::DuplicateInsert)
        );
    }
}
