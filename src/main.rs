mod app;
mod audio;
mod pronunciation;
mod throttle;
mod ui;

use std::time::Instant;

use anyhow::Result;
use app::App;
use audio::Speaker;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use throttle::{Decision, Throttle};

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
    // Keeps the toy from reacting faster than it can comfortably keep up with —
    // see `throttle.rs` for the rates.
    let mut throttle = Throttle::new();
    // Only draw when something actually changed. A held-down key floods us with
    // repeat events that the throttle drops, and rendering a frame for each of
    // them is exactly the kind of busywork that spins up the fans.
    let mut dirty = true;

    loop {
        if dirty {
            terminal.draw(|frame| ui::render(frame, app))?;
            dirty = false;
        }

        // Blocking read — there's no animation to keep alive, so we only wake up
        // when the kid actually presses a key.
        let event = event::read()?;
        // A resize changes the layout without touching any state.
        if matches!(event, Event::Resize(..)) {
            dirty = true;
            continue;
        }
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let now = Instant::now();

        match key.code {
            KeyCode::Esc => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,

            // Enter: synthesize the whole word and play it. espeak-ng does
            // grapheme-to-phoneme conversion, so real words and nonsense
            // ("linuf") are both pronounced by the same rules. Capped at twice a
            // second — mashing Enter is the most expensive thing you can do here.
            KeyCode::Enter => match throttle.word(now) {
                Decision::Allow => {
                    app.clear_status();
                    let word = app.word.clone();
                    if !word.is_empty() {
                        if let Err(e) = speaker.speak_word(&word) {
                            app.error(format!("{e:#}"));
                        }
                    }
                    dirty = true;
                }
                Decision::Slow => {
                    app.hint("Vänta lite — lyssna klart först!");
                    dirty = true;
                }
                Decision::Ignore => {}
            },

            // Space = easy reset button.
            KeyCode::Char(' ') => {
                app.clear();
                dirty = true;
            }
            KeyCode::Backspace => {
                app.backspace();
                dirty = true;
            }

            // Any letter (including å ä ö): show it and play its sound — as long
            // as the throttle lets it through and there's room on screen. The cap
            // follows the current terminal width (borders take 2 columns); when
            // it's reached we play nothing and show a gentle hint instead.
            KeyCode::Char(c) if c.is_alphabetic() => {
                // char::to_lowercase can yield multiple chars in theory; in
                // practice for our alphabet it's always one.
                let lc = c.to_lowercase().next().unwrap_or(c);
                // Reaching for a *different* letter means any nudge on screen has
                // done its job; holding one key down leaves it up.
                let switched = throttle.last_letter() != Some(lc);

                match throttle.letter(lc, now) {
                    Decision::Allow => {
                        if switched {
                            app.clear_status();
                        }
                        let width = terminal.size().map(|s| s.width).unwrap_or(0);
                        let cap = ui::capacity(width.saturating_sub(2));
                        if app.word.chars().count() >= cap {
                            app.hint("Skärmen är full — tryck mellanslag för att rensa");
                        } else {
                            app.push(lc);
                            if let Err(e) = speaker.speak_letter(lc) {
                                app.error(format!("{e:#}"));
                            }
                        }
                        dirty = true;
                    }
                    Decision::Slow => {
                        app.hint("Sakta lite — prova en annan bokstav!");
                        dirty = true;
                    }
                    Decision::Ignore => {}
                }
            }

            _ => {}
        }
    }
    Ok(())
}
