//! Rendering: a bordered frame, the current word in giant colorful glyphs, and
//! a friendly help/status line at the bottom.
//!
//! We draw the big glyphs ourselves from the `font8x8` bitmap font. Each glyph
//! is 8x8 pixels; we look it up in the ASCII (BASIC) block and fall back to the
//! LATIN block, which is where å ä ö (and Å Ä Ö) live. Set pixels become full
//! block characters, unset pixels become spaces.

use font8x8::{UnicodeFonts, BASIC_FONTS, LATIN_FONTS};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

/// Bright, high-contrast colors that cycle as the word grows.
const PALETTE: [Color; 6] = [
    Color::Red,
    Color::Yellow,
    Color::Green,
    Color::Cyan,
    Color::Magenta,
    Color::Blue,
];

const GLYPH_H: u16 = 8; // font8x8 glyphs are 8 rows tall
const CELL_W: u16 = 9; // 8 pixel columns + 1 gap column between letters

/// How many glyphs fit horizontally in an inner area `width` columns wide.
/// Callers use this to stop the word before it overflows the screen — both when
/// accepting new letters (`main.rs`) and when rendering after a resize (below).
pub fn capacity(width: u16) -> usize {
    (width / CELL_W) as usize
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Outer border + title.
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" linuf ")
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The big word — or a friendly placeholder when nothing's typed yet.
    let display = if app.word.is_empty() {
        "?".to_string()
    } else {
        app.word.to_uppercase()
    };
    // Never draw more glyphs than fit: `main.rs` already caps input for the
    // current size, but a mid-word terminal shrink could still leave the word
    // too long, so clamp here too (keep at least one so "?" always shows).
    let display: String = display
        .chars()
        .take(capacity(inner.width).max(1))
        .collect();
    let color = PALETTE[app.word.chars().count() % PALETTE.len()];

    let text_w = display.chars().count() as u16 * CELL_W;
    let big_area = centered(inner, text_w, GLYPH_H);
    frame.render_widget(
        Paragraph::new(big_lines(&display))
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        big_area,
    );

    // Help / status line pinned to the bottom row.
    let (help, help_color) = match &app.status {
        Some(err) => (format!("⚠ {err}"), Color::Red),
        None => (
            "Bokstav = ljud    Enter = säg ordet    Mellanslag = rensa    Esc = avsluta"
                .to_string(),
            Color::DarkGray,
        ),
    };
    let help_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(help)
            .alignment(Alignment::Center)
            .style(Style::default().fg(help_color)),
        help_area,
    );
}

/// Turn `text` into 8 rows of block characters ready to render.
fn big_lines(text: &str) -> Text<'static> {
    let glyphs: Vec<[u8; 8]> = text.chars().map(glyph).collect();
    let lines: Vec<Line> = (0..8)
        .map(|row| {
            let mut s = String::new();
            for g in &glyphs {
                // font8x8 rows are little-endian: bit 0 is the leftmost pixel.
                for col in 0..8 {
                    s.push(if (g[row] >> col) & 1 == 1 { '█' } else { ' ' });
                }
                s.push(' '); // gap between letters
            }
            Line::from(s)
        })
        .collect();
    Text::from(lines)
}

/// The 8x8 bitmap for `c`, trying the ASCII block then the LATIN block (which
/// holds å ä ö). Unknown characters render blank.
fn glyph(c: char) -> [u8; 8] {
    BASIC_FONTS
        .get(c)
        .or_else(|| LATIN_FONTS.get(c))
        .unwrap_or([0; 8])
}

/// A `w`×`h` rectangle centered inside `area` (clamped to fit).
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::glyph;

    #[test]
    fn swedish_uppercase_letters_all_have_glyphs() {
        // The whole point of the font8x8 LATIN fallback: Å Ä Ö must not be blank.
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZÅÄÖ".chars() {
            assert_ne!(glyph(c), [0u8; 8], "no glyph found for '{c}'");
        }
    }
}
