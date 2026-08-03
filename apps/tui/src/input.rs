use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default)]
pub struct Composer {
    text: String,
    cursor: usize,
}

impl Composer {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn insert_paste(&mut self, value: &str) {
        let value = sanitize_input(value);
        self.text.insert_str(self.cursor, &value);
        self.cursor += value.len();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ComposerAction {
        match key.code {
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.insert_char('\n');
                ComposerAction::Changed
            }
            KeyCode::Enter if !self.is_empty() => ComposerAction::Submit,
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER) =>
            {
                self.insert_char(character);
                ComposerAction::Changed
            }
            KeyCode::Backspace => {
                if let Some(previous) = self.text[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map(|(index, _)| index)
                {
                    self.text.drain(previous..self.cursor);
                    self.cursor = previous;
                    ComposerAction::Changed
                } else {
                    ComposerAction::None
                }
            }
            KeyCode::Delete => {
                if let Some(character) = self.text[self.cursor..].chars().next() {
                    let end = self.cursor + character.len_utf8();
                    self.text.drain(self.cursor..end);
                    ComposerAction::Changed
                } else {
                    ComposerAction::None
                }
            }
            KeyCode::Left => {
                if let Some(previous) = self.text[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map(|(index, _)| index)
                {
                    self.cursor = previous;
                }
                ComposerAction::None
            }
            KeyCode::Right => {
                if let Some(character) = self.text[self.cursor..].chars().next() {
                    self.cursor += character.len_utf8();
                }
                ComposerAction::None
            }
            KeyCode::Home => {
                self.cursor = self.text[..self.cursor]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
                ComposerAction::None
            }
            KeyCode::End => {
                self.cursor += self.text[self.cursor..]
                    .find('\n')
                    .unwrap_or(self.text.len() - self.cursor);
                ComposerAction::None
            }
            _ => ComposerAction::None,
        }
    }

    fn insert_char(&mut self, character: char) {
        if !character.is_control() || character == '\n' {
            self.text.insert(self.cursor, character);
            self.cursor += character.len_utf8();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerAction {
    None,
    Changed,
    Submit,
}

#[must_use]
pub fn sanitize_terminal_text(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| match character {
            '\n' => Some('\n'),
            '\t' => Some(' '),
            character if character.is_control() => None,
            character => Some(character),
        })
        .collect()
}

fn sanitize_input(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| match character {
            '\n' => Some('\n'),
            '\t' => Some(' '),
            character if character.is_control() => None,
            character => Some(character),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_unicode_without_splitting_characters() {
        let mut composer = Composer::default();
        composer.insert_paste("Lun🌙");
        composer.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        composer.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(composer.text(), "Lu🌙");
    }

    #[test]
    fn paste_preserves_lines_and_removes_terminal_controls() {
        let mut composer = Composer::default();
        composer.insert_paste("first\n\u{1b}]8;;bad\u{7}second\tline");
        assert_eq!(composer.text(), "first\n]8;;badsecond line");
        assert_eq!(
            sanitize_terminal_text("safe\u{1b}[31m\nline"),
            "safe[31m\nline"
        );
    }
}
