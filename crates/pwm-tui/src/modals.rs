//! Modal dialogs and shared single-line UTF-8 editor (`TextInput`).

fn prev_char_boundary(s: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    let mut i = idx.min(s.len()) - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = (idx + 1).min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Shared single-line UTF-8 editor (cursor stays on char boundaries).
pub struct TextInput {
    buf: String,
    cursor: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            cursor: 0,
        }
    }

    /// Cursor at end of `s` (initial `to` line, default fee).
    pub fn from_end(s: String) -> Self {
        let cursor = s.len();
        Self { buf: s, cursor }
    }

    pub fn as_str(&self) -> &str {
        self.buf.as_str()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Tests / programmatic fills; moves cursor to end.
    pub(crate) fn set_text(&mut self, s: String) {
        self.buf = s;
        self.cursor = self.buf.len();
    }

    pub fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.buf.len());
    }

    pub fn move_left(&mut self) {
        self.cursor = prev_char_boundary(&self.buf, self.cursor);
    }

    pub fn move_right(&mut self) {
        self.cursor = next_char_boundary(&self.buf, self.cursor);
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.buf.len();
    }

    pub fn insert_char(&mut self, c: char) {
        let i = self.cursor.min(self.buf.len());
        self.buf.insert(i, c);
        self.cursor = i + c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let from = prev_char_boundary(&self.buf, self.cursor);
        self.buf.drain(from..self.cursor);
        self.cursor = from;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        let to = next_char_boundary(&self.buf, self.cursor);
        self.buf.drain(self.cursor..to);
    }
}

/// After a successful send: offer to append `to` to wallet YAML (same mechanism as `pwm wallet book-add`).
pub struct BookPromptModal {
    pub to_display: String,
    pub label: TextInput,
    pub status: String,
}

impl BookPromptModal {
    pub fn new(to_display: String) -> Self {
        Self {
            to_display,
            label: TextInput::new(),
            status: "Optional label for address book (Enter=save, Esc=skip)".into(),
        }
    }
}

/// F3 unlock dialog for encrypted wallets (passphrase never logged).
pub struct UnlockModal {
    pub passphrase: TextInput,
    pub status: String,
    pub status_is_error: bool,
}

impl UnlockModal {
    pub fn new() -> Self {
        Self {
            passphrase: TextInput::new(),
            status: "Enter passphrase (Enter=unlock, Esc=cancel)".into(),
            status_is_error: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EncryptField {
    Passphrase,
    Confirm,
}

/// F4 encrypt / re-key: new passphrase + confirm (never logged).
pub struct EncryptModal {
    pub active: EncryptField,
    pub passphrase: TextInput,
    pub confirm: TextInput,
    pub status: String,
    pub status_is_error: bool,
    /// When true, title explains re-key for an already-encrypted wallet.
    pub is_rekey: bool,
}

impl EncryptModal {
    pub fn new(is_rekey: bool) -> Self {
        Self {
            active: EncryptField::Passphrase,
            passphrase: TextInput::new(),
            confirm: TextInput::new(),
            status: if is_rekey {
                "New passphrase + confirm (Enter=apply, Esc=cancel)".into()
            } else {
                "Set passphrase to encrypt wallet (Enter=apply, Esc=cancel)".into()
            },
            status_is_error: false,
            is_rekey,
        }
    }

    pub fn clamp_cursors(&mut self) {
        self.passphrase.clamp_cursor();
        self.confirm.clamp_cursor();
    }

    pub fn next_field(&mut self) {
        self.active = match self.active {
            EncryptField::Passphrase => EncryptField::Confirm,
            EncryptField::Confirm => EncryptField::Passphrase,
        };
    }

    pub fn prev_field(&mut self) {
        self.next_field();
    }

    pub fn active_line_mut(&mut self) -> &mut TextInput {
        match self.active {
            EncryptField::Passphrase => &mut self.passphrase,
            EncryptField::Confirm => &mut self.confirm,
        }
    }

    pub fn move_left(&mut self) {
        self.active_line_mut().move_left();
    }

    pub fn move_right(&mut self) {
        self.active_line_mut().move_right();
    }

    pub fn move_home(&mut self) {
        self.active_line_mut().move_home();
    }

    pub fn move_end(&mut self) {
        self.active_line_mut().move_end();
    }

    pub fn insert_char(&mut self, c: char) {
        self.active_line_mut().insert_char(c);
    }

    pub fn backspace(&mut self) {
        self.active_line_mut().backspace();
    }

    pub fn delete(&mut self) {
        self.active_line_mut().delete();
    }
}

/// Masked passphrase line (byte cursor, same as other inline editors).
pub fn masked_with_caret(pass: &str, cursor: usize) -> String {
    let i = cursor.min(pass.len());
    let mut out = String::with_capacity(pass.len() + 1);
    out.push_str(&"*".repeat(i));
    out.push('|');
    out.push_str(&"*".repeat(pass.len().saturating_sub(i)));
    out
}
