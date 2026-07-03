//! Rendering: a bordered frame, the current word in giant colorful glyphs, and
//! a friendly help/status line at the bottom.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tui_big_text::{BigText, PixelSize};

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

// PixelSize::Full renders each glyph in an 8x8 cell.
const GLYPH_W: u16 = 8;
const GLYPH_H: u16 = 8;

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
    let color = PALETTE[app.word.chars().count() % PALETTE.len()];

    let text_w = display.chars().count() as u16 * GLYPH_W;
    let big_area = centered(inner, text_w, GLYPH_H);
    let big = BigText::builder()
        .pixel_size(PixelSize::Full)
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .lines(vec![Line::from(display)])
        .build();
    frame.render_widget(big, big_area);

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
