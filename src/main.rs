mod app;
mod audio;
mod pronunciation;
mod ui;

use anyhow::Result;
use app::App;
use audio::Speaker;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

fn main() -> Result<()> {
    // ratatui::init() enters the alternate screen, turns on raw mode, and
    // installs a panic hook that restores the terminal — so a crash won't leave
    // the terminal in a broken state.
    let mut terminal = ratatui::init();

    // Set up audio first. If there's no output device (e.g. running headless),
    // fail cleanly instead of leaving the terminal mangled.
    let speaker = match Speaker::new() {
        Ok(s) => s,
        Err(e) => {
            ratatui::restore();
            eprintln!("Could not start audio: {e:#}");
            return Ok(());
        }
    };

    let mut app = App::new();
    let mut speaker = speaker;
    let result = run(&mut terminal, &mut app, &mut speaker);

    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App, speaker: &mut Speaker) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        // Blocking read — there's no animation to keep alive, so we only wake up
        // when the kid actually presses a key.
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Esc => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,

            // Enter: synthesize the whole word and play it. espeak-ng does
            // grapheme-to-phoneme conversion, so real words and nonsense
            // ("linuf") are both pronounced by the same rules.
            KeyCode::Enter => {
                let word = app.word.clone();
                if !word.is_empty() {
                    if let Err(e) = speaker.speak_word(&word) {
                        app.status = Some(format!("{e:#}"));
                    }
                }
            }

            // Space = easy reset button.
            KeyCode::Char(' ') => app.clear(),
            KeyCode::Backspace => app.backspace(),

            // Any letter (including å ä ö): show it and play its phonics sound —
            // but only while there's room on screen, so a kid mashing keys can't
            // push glyphs off the edge. The cap follows the current terminal
            // width (borders take 2 columns); when it's reached we play nothing
            // and show a gentle hint instead.
            KeyCode::Char(c) if c.is_alphabetic() => {
                let width = terminal.size().map(|s| s.width).unwrap_or(0);
                let cap = ui::capacity(width.saturating_sub(2));
                // char::to_lowercase can yield multiple chars in theory; in
                // practice for our alphabet it's always one.
                for lc in c.to_lowercase() {
                    if app.word.chars().count() >= cap {
                        app.status = Some("Skärmen är full — tryck mellanslag för att rensa".into());
                        break;
                    }
                    app.push(lc);
                    if let Err(e) = speaker.speak_letter(lc) {
                        app.status = Some(format!("{e:#}"));
                    }
                }
            }

            _ => {}
        }
    }
    Ok(())
}
