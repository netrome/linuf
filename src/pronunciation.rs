//! Maps a typed letter to the text we hand espeak-ng.
//!
//! We simply feed the plain letter, which makes espeak read out the letter's
//! **Swedish name** (a, be, se, de, e, ef, … ex, y, säta, å, ä, ö). Letter names
//! are full syllables, so every letter comes out loud and clear — unlike trying
//! to synthesize isolated phonics sounds, where bare consonants like /n/ are
//! almost inaudible.
//!
//! This is still the seam to tune: to override how one letter sounds, special-
//! case it below and return custom text (plain text, or espeak phonemes in
//! `[[ ... ]]`). Test a change without rebuilding, e.g. `espeak-ng -v sv "x"`.

pub fn letter_input(c: char) -> String {
    // Plain letter → espeak says its Swedish name. Special-case here if needed.
    c.to_string()
}
