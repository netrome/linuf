//! Rate limiting: how fast the toy is willing to react.
//!
//! Holding a key down makes the terminal deliver key-repeat events as fast as
//! it can, and every one of them means a redraw and a fresh mixer voice. So each
//! kind of input goes through a `Gate` that enforces a minimum gap since the
//! last *accepted* press; anything arriving early is simply dropped.
//!
//! Letters get one extra twist: pressing the **same** letter again and again
//! widens its gate step by step, while switching to a different letter always
//! gets the short base gap. So leaning on one key gets slower and slower, while
//! exploring the keyboard stays quick and lively — including sounds overlapping
//! into chords, which is half the fun.
//!
//! The gaps below are deliberately loose. The first version of this module was
//! written against a CPU load that turned out to be a leak in `audio.rs` rather
//! than the cost of reacting to a keypress; with that fixed, a press is cheap
//! (cached bytes, one decoder) and the only thing worth defending against is a
//! key-repeat flood. So these are set by what feels good to a small kid, and
//! `MAX_CONCURRENT` is what actually protects the CPU.

use std::time::{Duration, Instant};

/// Shortest gap between two accepted letters — short enough to feel instant.
const LETTER_MIN: Duration = Duration::from_millis(60);
/// Added to `LETTER_MIN` for each consecutive press of the *same* letter …
const REPEAT_STEP: Duration = Duration::from_millis(60);
/// … up to this ceiling, so a held-down key still trickles through instead of
/// going completely dead (which would just read as "broken" to a small kid).
const REPEAT_MAX: Duration = Duration::from_millis(300);
/// A pause this long means someone is typing deliberately rather than mashing,
/// so the same-letter penalty resets — "mamma" typed slowly never feels sluggish.
const RELAX: Duration = Duration::from_millis(800);
/// Enter speaks the whole word. Loose enough to re-trigger while still
/// listening, since hearing the word again is usually the point.
const WORD_MIN: Duration = Duration::from_millis(300);
/// Cap on the same-letter streak — `REPEAT_MAX` is reached long before this, so
/// it only exists to keep `REPEAT_STEP * streak` from ever overflowing.
const STREAK_MAX: u32 = 8;

/// What to do with a press the gate just looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Go ahead.
    Allow,
    /// Too soon — drop it, quietly.
    Ignore,
    /// Too soon, and it keeps happening: worth one gentle nudge on screen.
    Slow,
}

/// One rate-limited input channel.
struct Gate {
    last_pass: Option<Instant>,
    /// Presses dropped since the last accepted one.
    dropped: u32,
}

impl Gate {
    const fn new() -> Self {
        Self {
            last_pass: None,
            dropped: 0,
        }
    }

    /// Accept this press if at least `min` has elapsed since the last accepted
    /// one. The caller passes `min` per call because the letter gate widens it
    /// for repeats.
    fn check(&mut self, now: Instant, min: Duration) -> Decision {
        if let Some(last) = self.last_pass {
            if now.duration_since(last) < min {
                self.dropped += 1;
                // One early press is just fast fingers; a run of them means
                // someone's leaning on the key. Say so exactly once per burst —
                // returning `Slow` for every dropped event would trigger a
                // redraw for each of the events we're trying to ignore.
                return if self.dropped == 2 {
                    Decision::Slow
                } else {
                    Decision::Ignore
                };
            }
        }
        self.dropped = 0;
        self.last_pass = Some(now);
        Decision::Allow
    }

    /// Time since the last accepted press (enormous if there hasn't been one).
    fn since_pass(&self, now: Instant) -> Duration {
        self.last_pass
            .map_or(Duration::MAX, |last| now.duration_since(last))
    }
}

/// The app's gates: one for letters, one for Enter.
pub struct Throttle {
    letters: Gate,
    words: Gate,
    /// The letter accepted most recently, and how many times in a row it has
    /// been accepted.
    last_letter: Option<char>,
    streak: u32,
}

impl Throttle {
    pub const fn new() -> Self {
        Self {
            letters: Gate::new(),
            words: Gate::new(),
            last_letter: None,
            streak: 0,
        }
    }

    /// The letter accepted most recently. `main` compares the incoming letter
    /// against this to tell a fresh letter from a held key.
    pub fn last_letter(&self) -> Option<char> {
        self.last_letter
    }

    /// Should we show and speak `c`?
    pub fn letter(&mut self, c: char, now: Instant) -> Decision {
        // A real pause wipes the slate clean.
        if self.letters.since_pass(now) >= RELAX {
            self.streak = 0;
        }

        let repeated = self.last_letter == Some(c);
        let min = if repeated {
            (LETTER_MIN + REPEAT_STEP * self.streak).min(REPEAT_MAX)
        } else {
            LETTER_MIN
        };

        let decision = self.letters.check(now, min);
        if decision == Decision::Allow {
            self.streak = if repeated {
                (self.streak + 1).min(STREAK_MAX)
            } else {
                0
            };
            self.last_letter = Some(c);
        }
        decision
    }

    /// Should we speak the whole word?
    pub fn word(&mut self, now: Instant) -> Decision {
        self.words.check(now, WORD_MIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `base` plus `ms` milliseconds — lets the tests drive the clock by hand.
    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn the_first_press_always_passes() {
        let t = Instant::now();
        assert_eq!(Throttle::new().letter('a', t), Decision::Allow);
        assert_eq!(Throttle::new().word(t), Decision::Allow);
    }

    #[test]
    fn varied_letters_pass_at_the_base_rate() {
        let t = Instant::now();
        let mut th = Throttle::new();
        assert_eq!(th.letter('a', t), Decision::Allow);
        assert_eq!(th.letter('b', at(t, 40)), Decision::Ignore);
        assert_eq!(th.letter('b', at(t, 60)), Decision::Allow);
        assert_eq!(th.letter('c', at(t, 120)), Decision::Allow);
    }

    #[test]
    fn holding_one_letter_gets_slower_and_slower() {
        let t = Instant::now();
        let mut th = Throttle::new();
        assert_eq!(th.letter('a', t), Decision::Allow); // a repeat now needs 60ms
        assert_eq!(th.letter('a', at(t, 60)), Decision::Allow); // streak 1 → needs 120ms
        assert_eq!(th.letter('a', at(t, 160)), Decision::Ignore); // only 100ms in
        assert_eq!(th.letter('a', at(t, 180)), Decision::Allow); // streak 2 → needs 180ms
        assert_eq!(th.letter('a', at(t, 300)), Decision::Ignore); // only 120ms in
        assert_eq!(th.letter('a', at(t, 360)), Decision::Allow); // 180ms in
    }

    #[test]
    fn a_burst_of_dropped_presses_asks_for_a_nudge_once() {
        let t = Instant::now();
        let mut th = Throttle::new();
        assert_eq!(th.letter('a', t), Decision::Allow);
        assert_eq!(th.letter('a', at(t, 5)), Decision::Ignore);
        assert_eq!(th.letter('a', at(t, 10)), Decision::Slow);
        // …and then stays quiet for the rest of the burst.
        assert_eq!(th.letter('a', at(t, 15)), Decision::Ignore);
        assert_eq!(th.letter('a', at(t, 20)), Decision::Ignore);
    }

    #[test]
    fn a_different_letter_escapes_the_penalty() {
        let t = Instant::now();
        let mut th = Throttle::new();
        // Build up a streak of three a's (gaps of 60, 120, 180ms).
        for ms in [0, 60, 180, 360] {
            assert_eq!(th.letter('a', at(t, ms)), Decision::Allow);
        }
        // Another 'a' would now have to wait 240ms; a 'b' only waits the base gap.
        assert_eq!(th.letter('a', at(t, 420)), Decision::Ignore);
        assert_eq!(th.letter('b', at(t, 420)), Decision::Allow);
    }

    #[test]
    fn a_pause_resets_the_penalty() {
        let t = Instant::now();
        let mut th = Throttle::new();
        for ms in [0, 60, 180] {
            assert_eq!(th.letter('a', at(t, ms)), Decision::Allow);
        }
        // A repeat would need 180ms now, but after a proper pause the same letter
        // is back to the base gap …
        assert_eq!(th.letter('a', at(t, 180 + 800)), Decision::Allow);
        // … and starts climbing again from there.
        assert_eq!(th.letter('a', at(t, 180 + 800 + 60)), Decision::Ignore);
    }

    #[test]
    fn the_repeat_penalty_stops_growing() {
        let t = Instant::now();
        let mut th = Throttle::new();
        // Pressing the same letter at the ceiling rate never gets throttled,
        // however long the streak.
        for i in 0..20 {
            assert_eq!(th.letter('a', at(t, i * 300)), Decision::Allow, "press {i}");
        }
    }

    #[test]
    fn enter_is_rate_limited() {
        let t = Instant::now();
        let mut th = Throttle::new();
        assert_eq!(th.word(t), Decision::Allow);
        assert_eq!(th.word(at(t, 150)), Decision::Ignore);
        assert_eq!(th.word(at(t, 200)), Decision::Slow);
        assert_eq!(th.word(at(t, 300)), Decision::Allow);
        assert_eq!(th.word(at(t, 600)), Decision::Allow);
    }

    #[test]
    fn the_gates_are_independent() {
        let t = Instant::now();
        let mut th = Throttle::new();
        assert_eq!(th.word(t), Decision::Allow);
        // Mashing Enter must not hold letters back, or vice versa.
        assert_eq!(th.letter('a', t), Decision::Allow);
        assert_eq!(th.word(at(t, 60)), Decision::Ignore);
        assert_eq!(th.letter('b', at(t, 60)), Decision::Allow);
    }
}
