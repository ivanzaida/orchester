use std::collections::HashMap;

use lurq::{
  app::theme::{ThemeTypography, TypographyStyle},
  layout::text_style::{FontFeature, FontFeatures, FontWeight, TextStyle},
  node::color::Color,
};

use super::tokens::{FONT, ICON_FONT, MONO, MUTED, SELECT_ICON_SIZE, SUBTLE, TEXT};

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
  /// Page title of the setup steps, e.g. "Create your project" (`j6LVi`).
  PageTitle,
  /// Supporting detail under a notice heading or a rail action (`szYuk` in `q5LzI`, `et2jb`).
  Detail,
  /// Smallest supporting line, e.g. runner capacity (`tGGua`).
  Footnote,
  /// Emphasized notice heading and dialog option title (`T9TyR`, `Oq9Jc`).
  NoticeHeading,
  /// Title of a project screen below its topbar, e.g. "Tasks" (`eQKWX`, `E4mnm8`).
  ScreenTitle,
  /// Label above a field of the new task form, e.g. "Task title" (`Pg9E7`).
  FormLabel,
  /// Text of a status label, e.g. "Draft" (`JuW4o` in `DeizF`).
  StatusLabel,
  /// Count beside a navigation item (`LTXft` in `TSJpn`).
  NavCount,
  /// Task brief as it is written (`E9moJ`).
  Brief,
  /// Dialog title (`bhcaX`).
  DialogTitle,
  /// Dialog explanation under its title (`ClBsl`).
  Explanation,
  /// Chevrons and other 12px icons (`B1lur`, `Y9sVM2`).
  IconSmall,
  /// Acceptance criterion marker (`JjM8I`), 13px.
  IconMarker,
  /// Button and secondary navigation icons (`xfnek`, `ssq5w`).
  IconButton,
  /// Primary navigation icons (`sQnmq`).
  IconNav,
  /// Dismiss icon of a notice (`KGM0S`), 14px.
  IconDismiss,
  /// Option icons in a dialog, task selection and task state icons (`dsi3B`, `R7idX`, `UR2gw`), 16px.
  IconOption,
  /// Inline notice icon and the repository of the new task form (`GkfFF` in `q5LzI`, `iuGdG`), 18px.
  IconNotice,
  /// Footer icon on the first launch screen (`OYpN8`).
  IconFeature,
  /// Recovery panel icon (`xEwzT`).
  IconPanel,
  /// Window control icons in the title bar (`QQmsI`), 14px.
  IconWindowControl,
  /// Chevron of a select and check of its selected option (`eASPF`, `ClxwV`), 14px.
  IconSelect,
  /// Spaced mono label on the pipeline canvas, e.g. "ON TASK START" (`wRjZD`, `qwFek`, `RAoRx`).
  CanvasLabel,
  /// Small mono tag beside an inspector title, e.g. "STEP 05" (`kAJZA`, `N2NBl`).
  MonoTag,
  /// Mono value of a key/value property or a slug, e.g. "$1.50" (`yeeIR` in `BzWOd`, `cJsA7`).
  MonoValue,
  /// Step instructions as they are written (`v1Ky8`).
  Instructions,
  /// Short explanation under a field, e.g. "Choose a model here…" (`k66CB`, `Lwlf3`).
  Hint,
  /// Version figure in the publish confirmation, e.g. "v4" (`p8L552`).
  VersionFigure,
  /// Explanation under an empty state title (`xIG73`).
  Summary,
  /// Empty state icon (`Ujxx0`), 22px.
  IconLarge,
  /// Status label text in a list row (`JuW4o` in `v7McTX`).
  CompactStatusLabel,
  /// Title of an inspector panel, e.g. a connection's name (`kU9QO`).
  InspectorTitle,
}

impl ExtraTypography {
  const ALL: [Self; 43] = [
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
    Self::PageTitle,
    Self::Detail,
    Self::Footnote,
    Self::NoticeHeading,
    Self::ScreenTitle,
    Self::FormLabel,
    Self::StatusLabel,
    Self::NavCount,
    Self::Brief,
    Self::DialogTitle,
    Self::Explanation,
    Self::IconSmall,
    Self::IconMarker,
    Self::IconButton,
    Self::IconNav,
    Self::IconDismiss,
    Self::IconOption,
    Self::IconNotice,
    Self::IconFeature,
    Self::IconPanel,
    Self::IconWindowControl,
    Self::IconSelect,
    Self::CanvasLabel,
    Self::MonoTag,
    Self::MonoValue,
    Self::Instructions,
    Self::Hint,
    Self::VersionFigure,
    Self::Summary,
    Self::IconLarge,
    Self::CompactStatusLabel,
    Self::InspectorTitle,
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
      Self::PageTitle => "page-title",
      Self::Detail => "detail",
      Self::Footnote => "footnote",
      Self::NoticeHeading => "notice-heading",
      Self::ScreenTitle => "screen-title",
      Self::FormLabel => "form-label",
      Self::StatusLabel => "status-label",
      Self::NavCount => "nav-count",
      Self::Brief => "brief",
      Self::DialogTitle => "dialog-title",
      Self::Explanation => "explanation",
      Self::IconSmall => "icon-small",
      Self::IconMarker => "icon-marker",
      Self::IconButton => "icon-button",
      Self::IconNav => "icon-nav",
      Self::IconDismiss => "icon-dismiss",
      Self::IconOption => "icon-option",
      Self::IconNotice => "icon-notice",
      Self::IconFeature => "icon-feature",
      Self::IconPanel => "icon-panel",
      Self::IconWindowControl => "icon-window-control",
      Self::IconSelect => "icon-select",
      Self::CanvasLabel => "canvas-label",
      Self::MonoTag => "mono-tag",
      Self::MonoValue => "mono-value",
      Self::Instructions => "instructions",
      Self::Hint => "hint",
      Self::VersionFigure => "version-figure",
      Self::Summary => "summary",
      Self::IconLarge => "icon-large",
      Self::CompactStatusLabel => "compact-status-label",
      Self::InspectorTitle => "inspector-title",
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

/// Geist Mono without ligatures: its `liga` turns `--` into one cell-wide glyph that overlaps the
/// space before it, so commands such as `gh auth login --hostname` would read wrong.
fn mono(font_size: f32, color: &str) -> TextStyle {
  TextStyle {
    font_family: MONO.into(),
    font_features: FontFeatures::from([FontFeature::disable(*b"liga"), FontFeature::disable(*b"calt")]),
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
      Self::PageTitle => style(27.0, FontWeight::Medium, TEXT),
      Self::Detail => TextStyle {
        line_height: 1.6,
        ..style(11.0, FontWeight::Normal, MUTED)
      },
      Self::Footnote => style(10.0, FontWeight::Normal, MUTED),
      Self::NoticeHeading => style(12.0, FontWeight::Medium, TEXT),
      Self::ScreenTitle => style(28.0, FontWeight::Medium, TEXT),
      Self::FormLabel => style(12.0, FontWeight::Medium, TEXT),
      Self::StatusLabel => style(11.0, FontWeight::Medium, TEXT),
      Self::NavCount => mono(11.0, MUTED),
      Self::Brief => TextStyle {
        line_height: 1.7,
        ..style(13.0, FontWeight::Normal, TEXT)
      },
      Self::DialogTitle => style(20.0, FontWeight::Medium, TEXT),
      Self::Explanation => TextStyle {
        line_height: 1.6,
        ..style(12.0, FontWeight::Normal, MUTED)
      },
      Self::IconSmall => icon(12.0),
      Self::IconMarker => icon(13.0),
      Self::IconButton => icon(15.0),
      Self::IconNav => icon(17.0),
      Self::IconDismiss => icon(14.0),
      Self::IconOption => icon(16.0),
      Self::IconNotice => icon(18.0),
      Self::IconFeature => icon(19.0),
      Self::IconPanel => icon(24.0),
      Self::IconWindowControl => icon(14.0),
      Self::IconSelect => icon(SELECT_ICON_SIZE),
      Self::CanvasLabel => TextStyle {
        letter_spacing: 1.0,
        ..mono(9.0, SUBTLE)
      },
      Self::MonoTag => mono(9.0, MUTED),
      Self::MonoValue => mono(11.0, TEXT),
      Self::Instructions => TextStyle {
        line_height: 1.65,
        ..style(12.0, FontWeight::Normal, TEXT)
      },
      Self::Hint => TextStyle {
        line_height: 1.5,
        ..style(10.0, FontWeight::Normal, MUTED)
      },
      Self::VersionFigure => style(18.0, FontWeight::Medium, TEXT),
      Self::Summary => TextStyle {
        line_height: 1.6,
        ..style(13.0, FontWeight::Normal, MUTED)
      },
      Self::IconLarge => icon(22.0),
      Self::CompactStatusLabel => style(10.0, FontWeight::Medium, TEXT),
      Self::InspectorTitle => style(18.0, FontWeight::Medium, TEXT),
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
