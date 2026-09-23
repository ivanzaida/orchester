use lurq::{
  app::theme::ThemeTypography,
  layout::text_style::{FontWeight, TextStyle},
  node::color::Color,
};

use super::tokens::{FONT, MONO, MUTED, TEXT};

/// Type scale read from the canonical screens (Overview `Twtqc`, Task `vpgrJ`) and
/// Foundations components.
pub fn typography() -> ThemeTypography {
  let text = Color::from_hex(TEXT);
  let muted = Color::from_hex(MUTED);
  let style = |font_size: f32, weight: FontWeight, color: Color| TextStyle {
    font_family: FONT.into(),
    font_size,
    weight,
    color,
    ..TextStyle::default()
  };

  ThemeTypography {
    // Page title (`Q7Rw67`).
    heading: style(30.0, FontWeight::Medium, text),
    // Section title, e.g. "Tasks" (`ypXrG`).
    title: style(16.0, FontWeight::Medium, text),
    // Page description and navigation labels (`jVWaW`, `TSJpn`).
    body: style(13.0, FontWeight::Normal, text),
    // Supporting copy under titles (`Hkhzg`).
    description: TextStyle {
      line_height: 1.5,
      ..style(12.0, FontWeight::Normal, muted)
    },
    // Metadata, counts and property rows (`wr35v`, `tfGRt`).
    caption: style(11.0, FontWeight::Normal, muted),
    // Panel and card headings, e.g. "Recent activity" (`q7aFf`).
    label: style(13.0, FontWeight::Medium, text),
    field_label: style(12.0, FontWeight::Normal, muted),
    // Button label (`u1XBP`).
    button: style(12.0, FontWeight::Medium, text),
    // Inline actions, e.g. "Answer question →" (`p9lUhC`).
    link: style(11.0, FontWeight::Medium, text),
    // Revisions, shortcuts and counts (`q91Jf8`, `dNVoK`).
    mono: TextStyle {
      font_family: MONO.into(),
      ..style(10.0, FontWeight::Normal, muted)
    },
  }
}
