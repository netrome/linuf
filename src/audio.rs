//! Sound: everything goes through one code path.
//!
//! We shell out to `espeak-ng --stdout` to synthesize a WAV (for a single
//! letter's phonics sound, or for a whole word), then play the resulting bytes
//! with `rodio` straight from memory — no temp files.
//!
//! Synthesized bytes are cached by input string, so a repeated letter is
//! instant. Clips are allowed to overlap (up to `MAX_CONCURRENT`), which keeps
//! quick varied typing lively without letting a mashed key pile up voices
//! forever — `throttle.rs` handles the other half of that job.

use std::collections::HashMap;
use std::io::Cursor;
use std::process::Command;

use anyhow::{bail, Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

use crate::pronunciation::letter_input;

/// How many clips may overlap at once. Overlapping sounds are half the fun —
/// type a few different letters quickly and they layer into a chord — but each
/// one costs a decoder plus a mixer voice, so there's a ceiling to keep the CPU
/// quiet. Anything beyond it is dropped rather than queued, so the toy never
/// falls behind the keyboard.
const MAX_CONCURRENT: usize = 6;

pub struct Speaker {
    // Keep the stream alive for as long as the Speaker lives — dropping it
    // stops all audio.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    cache: HashMap<String, Vec<u8>>,
    /// Clips currently playing. A `Sink` stops its audio when dropped, so these
    /// are held until they've finished (see `speak`).
    active: Vec<Sink>,
    voice: String,
    /// Words-per-minute passed to espeak. Slower is friendlier for a small kid.
    speed: u32,
}

impl Speaker {
    pub fn new() -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("no audio output device found")?;
        Ok(Self {
            _stream: stream,
            handle,
            cache: HashMap::new(),
            active: Vec::new(),
            voice: "sv".to_string(),
            speed: 130,
        })
    }

    /// Play a single letter's phonics sound.
    pub fn speak_letter(&mut self, c: char) -> Result<()> {
        let input = letter_input(c);
        self.speak(&input)
    }

    /// Play a whole word, letting espeak's grapheme-to-phoneme rules do the
    /// work (so nonsense words are pronounced too, best-effort).
    pub fn speak_word(&mut self, word: &str) -> Result<()> {
        self.speak(word)
    }

    fn speak(&mut self, text: &str) -> Result<()> {
        // Forget the clips that have finished: they free up a slot under the
        // ceiling, and it keeps this list from growing all afternoon. Checked
        // before synthesizing, so a dropped sound costs nothing at all.
        self.active.retain(|sink| !sink.empty());
        if self.active.len() >= MAX_CONCURRENT {
            return Ok(());
        }

        if !self.cache.contains_key(text) {
            let wav = self.synth(text)?;
            self.cache.insert(text.to_string(), wav);
        }
        // Clone the cached bytes so the Cursor owns them (cheap for short clips).
        let wav = self.cache[text].clone();

        let sink = Sink::try_new(&self.handle).context("failed to create audio sink")?;
        let source =
            Decoder::new(Cursor::new(wav)).context("failed to decode espeak-ng audio")?;
        sink.append(source);
        // The clip plays out on rodio's background thread while we go straight
        // back to reading input, so sounds overlap freely — nice and lively for a
        // toy. We keep the Sink instead of detach()ing it purely so we can count
        // what's playing; dropping one would cut its sound off, which is why they
        // only get dropped once `empty()` says they're done. (To make new sounds
        // *interrupt* older ones instead, keep a single Sink and stop() it.)
        self.active.push(sink);
        Ok(())
    }

    fn synth(&self, text: &str) -> Result<Vec<u8>> {
        let output = Command::new("espeak-ng")
            .arg("-v")
            .arg(&self.voice)
            .arg("-s")
            .arg(self.speed.to_string())
            .arg("--stdout")
            .arg(text)
            .output()
            .context("failed to run espeak-ng — is it installed? (e.g. `apt install espeak-ng`)")?;

        if !output.status.success() {
            bail!(
                "espeak-ng exited with an error: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(output.stdout)
    }
}
