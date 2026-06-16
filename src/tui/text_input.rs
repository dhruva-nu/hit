//! A tiny single-line text accumulator shared by inline editors and the
//! credential-prompt modal. Holds only the raw value — masking, layout, and
//! key dispatch live at the call sites.

/// A one-line editable string buffer.
pub struct TextInput {
    value: String,
}

impl TextInput {
    /// Start from a seed string (the value currently being edited).
    pub fn new(seed: impl Into<String>) -> Self {
        Self { value: seed.into() }
    }

    /// Append a typed character.
    pub fn insert_char(&mut self, c: char) {
        self.value.push(c);
    }

    /// Delete the last character (no-op when empty).
    pub fn backspace(&mut self) {
        self.value.pop();
    }

    /// Borrow the current value.
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Consume the buffer, returning the accumulated string.
    pub fn into_string(self) -> String {
        self.value
    }
}
