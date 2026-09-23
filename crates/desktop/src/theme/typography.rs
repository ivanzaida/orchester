use std::collections::HashMap;

use lurq::{
  app::theme::{ThemeTypography, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
  node::color::Color,
};

use super::tokens::{FONT, ICON_FONT, MONO, MUTED, SUBTLE, TEXT};

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
  /// Product name in the Orchester brand (`qudYK`).
  Brand,
  /// Product name heading the setup rail (`PIceE`).
  SetupBrand,
  /// First launch headline (`cmrMC`).
  Display,
  /// First launch introduction under the headline (`oJkK5`).
  Lead,
  /// Spaced uppercase label above a group, e.g. "WELCOME TO ORCHESTER" (`rrrpd`, `M58ysd`, `K1H7ld`).
  Eyebrow,
  /// Step and explanation headings, e.g. "Connect a repository" (`WTDFX` in `Qazxp`, `Qbj0J`, `yhFcb`).
  Subheading,
  /// Numbered step marker, e.g. "01" (`AtKp7` in `Qazxp`).
  StepNumber,
  /// Chevrons and other 12px icons (`B1lur`, `Y9sVM2`).
  IconSmall,
  /// Button and secondary navigation icons (`xfnek`, `ssq5w`).
  IconButton,
  /// Primary navigation icons (`sQnmq`).
  IconNav,
  /// Footer icon on the first launch screen (`OYpN8`).
  IconFeature,
  /// Recovery panel icon (`xEwzT`).
  IconPanel,
}

impl ExtraTypography {
  const ALL: [Self; 16] = [
    Self::SectionHeading,
    Self::CardTitle,
    Self::Overline,
    Self::Metric,
    Self::Brand,
    Self::SetupBrand,
    Self::Display,
    Self::Lead,
    Self::Eyebrow,
    Self::Subheading,
    Self::StepNumber,
    Self::IconSmall,
    Self::IconButton,
    Self::IconNav,
    Self::IconFeature,
    Self::IconPanel,
  ];

  fn name(self) -> &'static str {
    match self {
      Self::SectionHeading => "section-heading",
      Self::CardTitle => "card-title",
      Self::Overline => "overline",
      Self::Metric => "metric",
      Self::Brand => "brand",
      Self::SetupBrand => "setup-brand",
      Self::Display => "display",
      Self::Lead => "lead",
      Self::Eyebrow => "eyebrow",
      Self::Subheading => "subheading",
      Self::StepNumber => "step-number",
      Self::IconSmall => "icon-small",
      Self::IconButton => "icon-button",
      Self::IconNav => "icon-nav",
      Self::IconFeature => "icon-feature",
      Self::IconPanel => "icon-panel",
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

fn icon(font_size: f32) -> TextStyle {
  TextStyle {
    font_family: ICON_FONT.into(),
    line_height: 1.0,
    ..style(font_size, FontWeight::Normal, MUTED)
  }
}

impl ExtraTypography {
  fn style(self) -> TextStyle {
    match self {
      Self::SectionHeading => style(15.0, FontWeight::Medium, TEXT),
      Self::CardTitle => style(14.0, FontWeight::Medium, TEXT),
      Self::Overline => style(9.0, FontWeight::SemiBold, SUBTLE),
      Self::Metric => mono(23.0, TEXT),
      Self::Brand => TextStyle {
        letter_spacing: -0.7,
        ..style(19.0, FontWeight::SemiBold, TEXT)
      },
      Self::SetupBrand => style(23.0, FontWeight::SemiBold, TEXT),
      Self::Display => TextStyle {
        letter_spacing: -1.2,
        line_height: 1.18,
        ..style(44.0, FontWeight::Medium, TEXT)
      },
      Self::Lead => TextStyle {
        line_height: 1.65,
        ..style(16.0, FontWeight::Normal, MUTED)
      },
      Self::Eyebrow => TextStyle {
        letter_spacing: 1.6,
        ..style(10.0, FontWeight::Normal, MUTED)
      },
      Self::Subheading => style(17.0, FontWeight::Medium, TEXT),
      Self::StepNumber => mono(12.0, SUBTLE),
      Self::IconSmall => icon(12.0),
      Self::IconButton => icon(15.0),
      Self::IconNav => icon(17.0),
      Self::IconFeature => icon(19.0),
      Self::IconPanel => icon(24.0),
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
    // Revisions, shortcuts and counts (`q91Jf8`, `dNVoK`).
    mono: mono(10.0, MUTED),
    extra: ExtraTypography::ALL
      .into_iter()
      .map(|extra| (extra.name().into(), extra.style()))
      .collect::<HashMap<_, _>>(),
  }
}
