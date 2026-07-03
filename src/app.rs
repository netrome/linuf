/// All the mutable state of the toy: the letters typed so far, plus an optional
/// status line (used to surface audio errors without crashing the app).
pub struct App {
    pub word: String,
    pub status: Option<String>,
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
        // A fresh keystroke clears any previous error message.
        self.status = None;
    }

    pub fn backspace(&mut self) {
        self.word.pop();
    }

    pub fn clear(&mut self) {
        self.word.clear();
    }
}
