//! Sound: everything goes through one code path.
//!
//! We shell out to `espeak-ng --stdout` to synthesize a WAV (for a single
//! letter's phonics sound, or for a whole word), then play the resulting bytes
//! with `rodio` straight from memory — no temp files. The header those bytes
//! come with needs patching first; see `repair_wav_sizes`.
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
const MAX_CONCURRENT: usize = 8;

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
        let mut wav = espeak(&self.voice, self.speed, text)?;
        repair_wav_sizes(&mut wav)?;
        Ok(wav)
    }
}

/// Synthesize `text` and return the WAV bytes espeak-ng wrote to stdout. They
/// are not playable as-is — see `repair_wav_sizes`.
fn espeak(voice: &str, speed: u32, text: &str) -> Result<Vec<u8>> {
    let output = Command::new("espeak-ng")
        .arg("-v")
        .arg(voice)
        .arg("-s")
        .arg(speed.to_string())
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

/// Rewrite the RIFF and `data` chunk lengths to match the bytes we actually got.
///
/// A WAV header states its own length up front, so a writer normally seeks back
/// and fills it in once it's done. Writing to a pipe there's nothing to seek, so
/// `espeak-ng --stdout` leaves a `0x7ffff000` (~2 GB) placeholder in both fields
/// — compare `espeak-ng -w file.wav`, which patches them properly.
///
/// Left alone, that placeholder is quietly ruinous: rodio believes the header,
/// so a 0.9 s clip claims to be 13.5 hours long and its decoder keeps handing
/// out silence past the end of the real samples instead of finishing. A source
/// that never finishes means `Sink::empty()` never turns true, so the sinks in
/// `active` are never reaped, `MAX_CONCURRENT` latches shut, and every sound
/// after the first few is dropped for the rest of the session. (Before those
/// sinks were kept around to be counted, the same clips were `detach()`ed and
/// leaked a silent, never-ending mixer voice per keypress instead — audible to
/// nobody, but re-summed on every sample forever, which is what was really
/// heating the CPU.)
///
/// We patch the bytes rather than switch to `-w` so everything stays in memory.
fn repair_wav_sizes(wav: &mut [u8]) -> Result<()> {
    let total = wav.len();
    if total < 12 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        bail!("espeak-ng did not return a WAV file");
    }
    wav[4..8].copy_from_slice(&((total - 8) as u32).to_le_bytes());

    // Walk the chunk list instead of assuming the canonical 44-byte header, so
    // an espeak-ng that starts emitting e.g. a LIST chunk doesn't break this.
    let mut pos = 12;
    while pos + 8 <= total {
        let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if &wav[pos..pos + 4] == b"data" {
            let actual = total - (pos + 8);
            wav[pos + 4..pos + 8].copy_from_slice(&(actual as u32).to_le_bytes());
            return Ok(());
        }
        // Chunk bodies are padded to an even length; the pad isn't in `size`.
        pos = pos
            .saturating_add(8)
            .saturating_add(size)
            .saturating_add(size & 1);
    }
    bail!("espeak-ng returned a WAV with no data chunk")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal WAV whose two length fields hold espeak's placeholder, plus
    /// `samples` bytes of audio.
    fn placeholder_wav(samples: usize) -> Vec<u8> {
        const PLACEHOLDER: u32 = 0x7fff_f000;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&PLACEHOLDER.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&[0; 16]);
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&PLACEHOLDER.to_le_bytes());
        wav.extend(std::iter::repeat_n(0u8, samples));
        wav
    }

    fn size_at(wav: &[u8], pos: usize) -> u32 {
        u32::from_le_bytes(wav[pos..pos + 4].try_into().unwrap())
    }

    #[test]
    fn the_placeholder_lengths_get_corrected() {
        let mut wav = placeholder_wav(1000);
        let total = wav.len();
        repair_wav_sizes(&mut wav).unwrap();
        assert_eq!(size_at(&wav, 4), (total - 8) as u32);
        assert_eq!(size_at(&wav, 40), 1000);
    }

    #[test]
    fn an_unknown_chunk_before_data_is_skipped() {
        let mut wav = placeholder_wav(64);
        // Splice a 3-byte LIST chunk (so it's padded to 4) in front of `data`.
        let data_at = 36;
        let mut list = b"LIST".to_vec();
        list.extend_from_slice(&3u32.to_le_bytes());
        list.extend_from_slice(&[0; 4]);
        wav.splice(data_at..data_at, list);

        let total = wav.len();
        repair_wav_sizes(&mut wav).unwrap();
        assert_eq!(size_at(&wav, 4), (total - 8) as u32);
        assert_eq!(size_at(&wav, data_at + 12 + 4), 64);
    }

    /// The regression test for the bug this repair exists for: a clip decoded
    /// straight from `--stdout` never ends, which silently wedges playback for
    /// the whole session (see `repair_wav_sizes`). Needs espeak-ng, same as the
    /// app itself; no audio device required.
    #[test]
    fn a_repaired_clip_actually_ends() {
        use rodio::Source;
        use std::time::Duration;

        let Ok(raw) = espeak("sv", 130, "a") else {
            eprintln!("skipping: espeak-ng not installed");
            return;
        };
        let one_letter = 22_050 * 5; // five seconds at espeak's output rate

        // Unrepaired, the ~2 GB placeholder makes a 0.9s clip claim to be hours
        // long, and the decoder keeps handing out silence rather than finishing.
        let bogus = Decoder::new(Cursor::new(raw.clone())).unwrap();
        assert!(bogus.total_duration().unwrap() > Duration::from_secs(3600));
        assert_eq!(bogus.take(one_letter).count(), one_letter, "clip ended?");

        let mut wav = raw;
        repair_wav_sizes(&mut wav).unwrap();
        let fixed = Decoder::new(Cursor::new(wav)).unwrap();
        let claimed = fixed.total_duration().unwrap();
        assert!(claimed < Duration::from_secs(5), "still bogus: {claimed:?}");
        // The part that matters: it terminates, so `Sink::empty()` can work.
        let played = fixed.take(one_letter).count();
        assert!(played > 0 && played < one_letter, "did not end: {played}");
    }

    #[test]
    fn garbage_is_rejected_rather_than_played() {
        assert!(repair_wav_sizes(&mut []).is_err());
        assert!(repair_wav_sizes(&mut b"not a wav at all".to_vec()).is_err());
        // A well-formed header with no `data` chunk.
        let mut headerless = placeholder_wav(0);
        headerless.truncate(36);
        assert!(repair_wav_sizes(&mut headerless).is_err());
    }
}
