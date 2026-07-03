# linuf

A playful terminal alphabet-and-sound toy for a small kid.

Press a letter → it appears in giant colorful glyphs and its **name** is read
aloud (Swedish: "be", "se", "ex", "ö", …). Press **Enter** → the whole "word" is
spoken out loud. It doesn't matter whether it's a real word or a random jumble
like `linuf` — it gets pronounced either way.

> Named after playing with my son Linus's name — writing and pronouncing silly
> variations of it, which he finds hilarious.

## What it does

- **Per-letter sounds.** Each keystroke reads the letter's Swedish *name*
  aloud. (We started with phonics sounds — `mmm`, `aaa` — but isolated
  consonants synthesize almost inaudibly; letter names are full syllables and
  come out uniformly loud and clear. See the design notes.) Swedish alphabet,
  incl. `å ä ö`.
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
| Per-letter audio    | **Letter names**              | Full syllables → uniformly loud; no per-consonant tuning |
| Whole-word TTS      | **espeak-ng**                 | Tiny, instant, offline, g2p → nonsense works. Robotic but intelligible |
| Terminal UI         | **Ratatui + font8x8**         | Big colorful glyphs delight a small kid |

### Why letter names instead of phonics

The original plan was phonics — play each letter's *sound* so they blend into
words. The catch: espeak synthesizes an isolated consonant like /n/ almost
silently. Measured RMS loudness of a bare `[[n]]` was ~300 vs. ~2500 for a
vowel — about 10× quieter, effectively inaudible. Adding a schwa (`[[n@]]`, i.e.
"nuh") fixes the loudness but is fiddly to tune per-letter by ear.

Feeding espeak the **plain letter** instead makes it say the letter's name
("be", "en", "ex", …). Names are full syllables, so every letter is loud and
clear with zero tuning. That's the current behavior.

`src/pronunciation.rs` is still the seam: it maps a typed letter to the text we
hand espeak. To customize one letter (or bring back phonics for some), special-
case it there. Test any change without rebuilding:

```sh
espeak-ng -v sv "x"        # a letter name  → "ex"
espeak-ng -v sv "[[n@]]"   # a phonics sound → "nuh"
espeak-ng -v sv "linuf"    # a whole word
```

### Rendering å ä ö

The giant glyphs come from the `font8x8` bitmap font. We render them ourselves
(in `src/ui.rs`) rather than via the `tui-big-text` crate, because that crate
only looks glyphs up in font8x8's ASCII block — so `å ä ö` come out blank. We
fall back to font8x8's LATIN block, which contains them. (A unit test asserts
every Swedish uppercase letter, including Å Ä Ö, has a non-blank glyph.)

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
| a–ö            | Add letter + say its name       |
| Enter          | Speak the whole word            |
| Space          | Clear the word (reset)          |
| Backspace      | Delete last letter              |
| Esc / Ctrl-C   | Quit                            |

## Roadmap / ideas

- **Nicer voice:** swap espeak-ng for [Piper](https://github.com/rhasspy/piper)
  (neural, natural-sounding Swedish; it even uses espeak-ng as its phonemizer).
  The `Speaker` in `src/audio.rs` is the only thing that would change.
- **Your own voice:** replace per-letter synthesis with recorded clips.
- **Phonics mode:** a toggle to switch per-letter audio from names back to
  sounds (using the `[[X@]]` schwa forms so they're audible) for reading practice.
- **Interrupt vs. overlap:** currently sounds overlap when keys are mashed; hold
  a single `Sink` and `stop()` it to make new sounds cut off older ones.
- Per-letter colors, little animations, a "say it again" key, etc.
