use std::collections::HashMap;

use lurq::{
  app::theme::{ThemeTypography, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
  node::color::Color,
};

use super::tokens::{FONT, MONO, MUTED, SUBTLE, TEXT};

/// Pencil text styles without a built-in lurq role, registered as `TypographyStyle::Extra`.
/// Use these instead of spelling extra names at call sites.
#[derive(Clone, Copy)]
pub enum ExtraTypography {
  /// Content section heading, e.g. "What changed" (`kiImw`).
  SectionHeading,
  /// Card and panel titles, e.g. "Needs you" (`qlo9F`).
  CardTitle,
  /// Small uppercase category label, e.g. "REVIEW PULL REQUEST" (`f1tfh3`).
  Overline,
  /// Large monospaced figure in project metrics (`PwPmH`).
  Metric,
}

impl ExtraTypography {
  const ALL: [Self; 4] = [Self::SectionHeading, Self::CardTitle, Self::Overline, Self::Metric];

  fn name(self) -> &'static str {
    match self {
      Self::SectionHeading => "section-heading",
      Self::CardTitle => "card-title",
      Self::Overline => "overline",
      Self::Metric => "metric",
    }
  }

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }
}

fn style(font_size: f32, weight: FontWeight, color: &str) -> TextStyle {
  TextStyle {
    font_family: FONT.into(),
    font_size,
    weight,
    color: Color::from_hex(color),
    ..TextStyle::default()
  }
}

fn mono(font_size: f32, color: &str) -> TextStyle {
  TextStyle {
    font_family: MONO.into(),
    ..style(font_size, FontWeight::Normal, color)
  }
}

impl ExtraTypography {
  fn style(self) -> TextStyle {
    match self {
      Self::SectionHeading => style(15.0, FontWeight::Medium, TEXT),
      Self::CardTitle => style(14.0, FontWeight::Medium, TEXT),
      Self::Overline => style(9.0, FontWeight::SemiBold, SUBTLE),
      Self::Metric => mono(23.0, TEXT),
    }
  }
}

/// Type scale read from the canonical screens (Overview `Twtqc`, Task `vpgrJ`) and
/// Foundations components.
pub fn typography() -> ThemeTypography {
  ThemeTypography {
    // Page title (`Q7Rw67`).
    heading: style(30.0, FontWeight::Medium, TEXT),
    // Section title, e.g. "Tasks" (`ypXrG`).
    title: style(16.0, FontWeight::Medium, TEXT),
    // Page description and navigation labels (`jVWaW`, `TSJpn`).
    body: style(13.0, FontWeight::Normal, TEXT),
    // Supporting copy under titles (`Hkhzg`).
    description: TextStyle {
      line_height: 1.5,
      ..style(12.0, FontWeight::Normal, MUTED)
    },
    // Metadata, counts and property rows (`wr35v`, `tfGRt`).
    caption: style(11.0, FontWeight::Normal, MUTED),
    // Panel and card headings, e.g. "Recent activity" (`q7aFf`).
    label: style(13.0, FontWeight::Medium, TEXT),
    field_label: style(12.0, FontWeight::Normal, MUTED),
    // Button label (`u1XBP`).
    button: style(12.0, FontWeight::Medium, TEXT),
    // Inline actions, e.g. "Answer question →" (`p9lUhC`).
    link: style(11.0, FontWeight::Medium, TEXT),
    // Revisions, shortcuts and counts (`q91Jf8`, `dNVoK`), in Geist Mono.
    mono: mono(10.0, MUTED),
    extra: ExtraTypography::ALL
      .into_iter()
      .map(|extra| (extra.name().into(), extra.style()))
      .collect::<HashMap<_, _>>(),
  }
}
