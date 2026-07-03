//! Maps a letter to the text/phoneme string we feed espeak-ng so it plays the
//! letter's *sound* (phonics), not its alphabet name.
//!
//! ## The idea
//!
//! We want blending sounds — "l-i-n-u-f" — not letter names ("ell-ee-enn...").
//!
//! * **Swedish vowels are easy**: the letter *name* already *is* the sound
//!   (a=/ɑː/, e=/eː/, i=/iː/, o=/uː/, u=/ʉː/, y=/yː/, å=/oː/, ä=/ɛː/, ö=/øː/),
//!   so we just feed the plain letter.
//! * **Consonants are the tricky part**: a bare "b" gets read as the letter
//!   name "be", so we use espeak-ng's phoneme input `[[ ... ]]` with a light
//!   schwa (`@`) to get a short "buh"-style sound.
//!
//! ## This table is meant to be TUNED BY EAR
//!
//! Pronunciation of isolated sounds is subjective and voice-dependent, so treat
//! the consonant entries below as a starting point. If a letter sounds wrong,
//! just change its string here — you can use:
//!   * plain text:  `'m' => "mm".into()`      (espeak reads it as Swedish text)
//!   * phonemes:    `'b' => "[[b@]]".into()`   (espeak's `[[...]]` phoneme mode)
//!
//! Test a single sound from the shell without rebuilding, e.g.:
//!   `espeak-ng -v sv "[[b@]]"`   or   `espeak-ng -v sv "a"`

pub fn letter_input(c: char) -> String {
    match c {
        // Vowels — the plain letter already gives the correct Swedish sound.
        'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'å' | 'ä' | 'ö' => c.to_string(),

        // Plosives — nearly silent alone, so add a schwa to make them audible.
        'b' => "[[b@]]".into(),
        'd' => "[[d@]]".into(),
        'g' => "[[g@]]".into(),
        'k' => "[[k@]]".into(),
        'p' => "[[p@]]".into(),
        't' => "[[t@]]".into(),
        'q' => "[[k@]]".into(),

        // Continuants — these can be held on their own.
        'f' => "[[f]]".into(),
        'l' => "[[l]]".into(),
        'm' => "[[m]]".into(),
        'n' => "[[n]]".into(),
        'r' => "[[r]]".into(),
        's' => "[[s]]".into(),
        'v' => "[[v]]".into(),

        // Misc / context-dependent letters — best-effort defaults.
        'c' => "[[s@]]".into(),
        'h' => "[[h@]]".into(),
        'j' => "[[j]]".into(), // Swedish "j" is a /j/ (like English "y" in yes)
        'w' => "[[v]]".into(),
        'x' => "[[ks]]".into(),
        'z' => "[[s]]".into(),

        // Fallback: let espeak try to read the raw character.
        other => other.to_string(),
    }
}
