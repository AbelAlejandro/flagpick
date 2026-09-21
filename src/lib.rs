pub mod adapters;
pub mod buffer;
pub mod cache;
pub mod discovery;
pub mod parser;
pub mod picker;
pub mod probe;
pub mod schema;

use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorError {
    cursor: usize,
    character_count: usize,
}

impl fmt::Display for CursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cursor {} is beyond the buffer's {} characters",
            self.cursor, self.character_count
        )
    }
}

impl Error for CursorError {}

pub fn character_cursor_to_byte(buffer: &str, cursor: usize) -> Result<usize, CursorError> {
    if cursor == 0 {
        return Ok(0);
    }

    if let Some((byte, _)) = buffer.char_indices().nth(cursor) {
        return Ok(byte);
    }

    let character_count = buffer.chars().count();
    if cursor == character_count {
        Ok(buffer.len())
    } else {
        Err(CursorError {
            cursor,
            character_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::character_cursor_to_byte;

    #[test]
    fn converts_character_positions_to_byte_offsets() {
        let input = "aé🙂z";
        assert_eq!(character_cursor_to_byte(input, 0), Ok(0));
        assert_eq!(character_cursor_to_byte(input, 1), Ok(1));
        assert_eq!(character_cursor_to_byte(input, 2), Ok(3));
        assert_eq!(character_cursor_to_byte(input, 3), Ok(7));
        assert_eq!(character_cursor_to_byte(input, 4), Ok(8));
        assert!(character_cursor_to_byte(input, 5).is_err());
    }
}
