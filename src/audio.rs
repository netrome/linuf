//! Sound: everything goes through one code path.
//!
//! We shell out to `espeak-ng --stdout` to synthesize a WAV (for a single
//! letter's phonics sound, or for a whole word), then play the resulting bytes
//! with `rodio` straight from memory — no temp files.
//!
//! Synthesized bytes are cached by input string, so a repeated letter is
//! instant and a kid mashing keys stays responsive.

use std::collections::HashMap;
use std::io::Cursor;
use std::process::Command;

use anyhow::{bail, Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

use crate::pronunciation::letter_input;

pub struct Speaker {
    // Keep the stream alive for as long as the Speaker lives — dropping it
    // stops all audio.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    cache: HashMap<String, Vec<u8>>,
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
        // detach() lets the clip finish on rodio's background thread while we go
        // straight back to reading input. Sounds overlap freely if keys are
        // pressed quickly — nice and lively for a toy. (To make new sounds
        // *interrupt* older ones instead, hold onto a single Sink and stop it.)
        sink.detach();
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
