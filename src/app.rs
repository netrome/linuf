/// A message for the bottom line: either something went wrong, or a friendly
/// nudge (screen full, slow down a bit). They're rendered differently — an
/// error is a red warning, a hint is just a soft yellow suggestion.
pub enum Status {
    Error(String),
    Hint(String),
}

/// All the mutable state of the toy: the letters typed so far, plus an optional
/// status line (used to surface audio errors without crashing the app, and to
/// nudge a key-masher).
pub struct App {
    pub word: String,
    pub status: Option<Status>,
}

impl App {
    pub fn new() -> Self {
        Self {
            word: String::new(),
            status: None,
        }
    }

    pub fn push(&mut self, c: char) {
        self.word.push(c);
    }

    pub fn backspace(&mut self) {
        self.word.pop();
        self.status = None;
    }

    pub fn clear(&mut self) {
        self.word.clear();
        self.status = None;
    }

    pub fn hint(&mut self, msg: impl Into<String>) {
        self.status = Some(Status::Hint(msg.into()));
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.status = Some(Status::Error(msg.into()));
    }

    /// Back to the plain help line. Called when a keystroke shows the previous
    /// message has served its purpose — note that repeating the *same* letter
    /// deliberately doesn't clear it, so a "slow down" nudge stays put instead
    /// of blinking on and off while a key is held.
    pub fn clear_status(&mut self) {
        self.status = None;
    }
}
