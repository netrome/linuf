# linuf

A playful terminal alphabet-and-sound toy for a small kid.

Press a letter → it appears in giant colorful glyphs and its **sound** plays.
Press **Enter** → the whole "word" is spoken out loud. It doesn't matter whether
it's a real word or a random jumble like `linuf` — it gets pronounced either way.

> Named after playing with my son Linus's name — writing and pronouncing silly
> variations of it, which he finds hilarious.

## What it does

- **Per-letter sounds (phonics).** Each keystroke plays the letter's *sound*
  (like `mmm`, `aaa`) rather than its alphabet name (`emm`, `aa`) — because
  sounds blend into words and names don't. Swedish alphabet, incl. `å ä ö`.
- **Whole-word speech on Enter.** The typed string is synthesized and spoken.
- **Real words *and* nonsense both work.** This falls out of the design (see
  below) — there's no "is this a real word?" check.
- **Big, colorful, terminal-native.** Giant glyphs, a color that changes as the
  word grows. Fun for a 3-year-old; runs anywhere a Linux terminal does.

## How it works (design notes)

The whole thing is deliberately one code path:

```
keypress ─┐
          ├─▶ text ──▶ espeak-ng --stdout ──▶ WAV bytes ──▶ rodio ──▶ 🔊
enter  ───┘             (offline, g2p)         (in memory)
```

- **espeak-ng** is a tiny, offline, rule-based speech synthesizer. It works by
  **grapheme-to-phoneme (g2p)** conversion — it reads letters and applies
  pronunciation rules, rather than looking words up in a dictionary. That's
  exactly why nonsense strings are pronounced just fine: there's nothing special
  to handle. Swedish orthography is fairly phonetic, so this works well.
- **rodio** plays the resulting WAV bytes straight from memory (no temp files).
  Playback happens on a background thread, so input never blocks.
- Synthesized clips are **cached by input string**, so repeats are instant.

### Why these choices

| Decision            | Choice                        | Why |
|---------------------|-------------------------------|-----|
| Per-letter audio    | **Phonics sounds**            | Blend into words; best for early reading |
| Whole-word TTS      | **espeak-ng**                 | Tiny, instant, offline, g2p → nonsense works. Robotic but intelligible |
| Terminal UI         | **Ratatui + tui-big-text**    | Big colorful glyphs delight a small kid |

### The one hand-tuned part: `src/pronunciation.rs`

Getting a clean *isolated* consonant sound is the only fiddly bit. A bare "b"
gets read as the letter name "be", so consonants use espeak's phoneme input
(`[[b@]]` ≈ "buh"). Vowels are easy — in Swedish the letter name already *is*
the sound, so we feed the plain letter.

**This table is meant to be tuned by ear.** If a letter sounds off, edit its
string in `src/pronunciation.rs`. You can test a sound without rebuilding:

```sh
espeak-ng -v sv "[[b@]]"   # a consonant sound
espeak-ng -v sv "a"        # a vowel sound
espeak-ng -v sv "linuf"    # a whole word
```

## Requirements

A Rust toolchain (`rustup`), plus at build/run time:

```sh
# Debian / Ubuntu
sudo apt install espeak-ng libasound2-dev pkg-config

# Fedora
sudo dnf install espeak-ng alsa-lib-devel pkgconf-pkg-config

# Arch
sudo pacman -S espeak-ng alsa-lib pkgconf
```

- `espeak-ng` — the speech synthesizer (runtime).
- `libasound2-dev` / `alsa-lib-devel` + `pkg-config` — ALSA dev headers that
  `rodio`/`cpal` need to **build** on Linux.
- A running audio server (PipeWire or PulseAudio — you'll already have one).

## Build & run

```sh
cargo run --release
```

### Controls

| Key            | Action                          |
|----------------|---------------------------------|
| a–ö            | Add letter + play its sound     |
| Enter          | Speak the whole word            |
| Space          | Clear the word (reset)          |
| Backspace      | Delete last letter              |
| Esc / Ctrl-C   | Quit                            |

## Known gotchas

- **`tui-big-text` version drift.** If `cargo build` complains that
  `.build()` in `src/ui.rs` returns a `Result`, that version wants
  `.build()?`. Older ones returned a `Result`; newer ones return the widget
  directly. One-character fix.
- This scaffold hasn't been compiled on the machine it was written on (no audio
  stack / ALSA headers there). Expect to iterate a little on first `cargo build`.

## Roadmap / ideas

- **Nicer voice:** swap espeak-ng for [Piper](https://github.com/rhasspy/piper)
  (neural, natural-sounding Swedish; it even uses espeak-ng as its phonemizer).
  The `Speaker` in `src/audio.rs` is the only thing that would change.
- **Your own voice:** replace per-letter synthesis with recorded clips.
- **Interrupt vs. overlap:** currently sounds overlap when keys are mashed; hold
  a single `Sink` and `stop()` it to make new sounds cut off older ones.
- Per-letter colors, little animations, a "say it again" key, etc.
