//! The text style of every extra typography role, from the Pencil values in `tokens`.

use lurq::{
  layout::text_style::{FontFeature, FontFeatures, FontWeight, TextStyle},
  node::color::Color,
};

use super::{
  super::tokens::{CODE_COMMENT, CODE_TEXT, FONT, ICON_FONT, MONO, MUTED, SELECT_ICON_SIZE, SUBTLE, TEXT},
  ExtraTypography,
};

pub(super) fn style(font_size: f32, weight: FontWeight, color: &str) -> TextStyle {
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
pub(super) fn mono(font_size: f32, color: &str) -> TextStyle {
  TextStyle {
    font_family: MONO.into(),
    font_features: FontFeatures::from([FontFeature::disable(*b"liga"), FontFeature::disable(*b"calt")]),
    ..style(font_size, FontWeight::Normal, color)
  }
}

/// `base` with `line_height` between wrapped lines.
fn leading(line_height: f32, base: TextStyle) -> TextStyle {
  TextStyle { line_height, ..base }
}

fn icon(font_size: f32) -> TextStyle {
  TextStyle {
    font_family: ICON_FONT.into(),
    line_height: 1.0,
    ..style(font_size, FontWeight::Normal, MUTED)
  }
}

impl ExtraTypography {
  /// The size of an icon role; `None` for a text role.
  const fn icon_size(self) -> Option<f32> {
    match self {
      Self::IconTag => Some(10.0),
      Self::IconSmall => Some(12.0),
      Self::IconReason => Some(11.0),
      Self::IconMarker => Some(13.0),
      Self::IconButton => Some(15.0),
      Self::IconNav => Some(17.0),
      Self::IconDismiss => Some(14.0),
      Self::IconOption => Some(16.0),
      Self::IconNotice => Some(18.0),
      Self::IconFeature => Some(19.0),
      Self::IconPanel => Some(24.0),
      Self::IconWindowControl => Some(14.0),
      Self::IconSelect => Some(SELECT_ICON_SIZE),
      Self::IconLarge => Some(22.0),
      Self::IconRow => Some(20.0),
      Self::IconEntry => Some(13.0),
      Self::IconEvent => Some(14.0),
      Self::IconState => Some(20.0),
      _ => None,
    }
  }

  /// The style of a Geist Mono role; `None` for the others.
  fn mono_style(self) -> Option<TextStyle> {
    Some(match self {
      Self::Metric => mono(23.0, TEXT),
      Self::StepNumber => mono(12.0, SUBTLE),
      Self::NavCount => mono(11.0, MUTED),
      Self::CanvasLabel => TextStyle {
        letter_spacing: 1.0,
        ..mono(9.0, SUBTLE)
      },
      Self::MonoTag => mono(9.0, MUTED),
      Self::MonoValue => mono(11.0, TEXT),
      Self::MonoLimit => TextStyle {
        weight: FontWeight::Medium,
        ..mono(11.0, TEXT)
      },
      Self::MonoField => mono(12.0, TEXT),
      Self::PullRequestContext => mono(11.0, MUTED),
      Self::DiffPath => TextStyle {
        weight: FontWeight::Medium,
        ..mono(14.0, TEXT)
      },
      Self::DiffLineNumber => mono(11.0, CODE_COMMENT),
      Self::Code => leading(1.6, mono(12.0, CODE_TEXT)),
      Self::Output => leading(1.6, mono(11.0, MUTED)),
      Self::PromptText => leading(1.8, mono(12.0, TEXT)),
      Self::VariableName => TextStyle {
        weight: FontWeight::Medium,
        ..mono(20.0, TEXT)
      },
      Self::MonoTitle => mono(12.0, TEXT),
      Self::MaskedValue => mono(13.0, MUTED),
      Self::MonoArea => leading(1.6, mono(12.0, TEXT)),
      Self::FirstPassword => TextStyle {
        letter_spacing: 0.5,
        ..mono(14.0, TEXT)
      },
      _ => return None,
    })
  }

  pub(super) fn style(self) -> TextStyle {
    if let Some(mono) = self.mono_style() {
      return mono;
    }
    match self {
      Self::SectionHeading => style(15.0, FontWeight::Medium, TEXT),
      Self::CardTitle => style(14.0, FontWeight::Medium, TEXT),
      Self::ReviewHeading => style(14.0, FontWeight::Normal, TEXT),
      Self::Overline => style(9.0, FontWeight::SemiBold, SUBTLE),
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
      Self::Lead => leading(1.65, style(16.0, FontWeight::Normal, MUTED)),
      Self::Eyebrow => TextStyle {
        letter_spacing: 1.6,
        ..style(10.0, FontWeight::Normal, MUTED)
      },
      Self::Subheading => style(17.0, FontWeight::Medium, TEXT),
      Self::PageTitle => style(27.0, FontWeight::Medium, TEXT),
      Self::Detail => leading(1.6, style(11.0, FontWeight::Normal, MUTED)),
      Self::Footnote => style(10.0, FontWeight::Normal, MUTED),
      Self::NoticeHeading => style(12.0, FontWeight::Medium, TEXT),
      Self::ScreenTitle => style(28.0, FontWeight::Medium, TEXT),
      Self::FormLabel => style(12.0, FontWeight::Medium, TEXT),
      Self::StatusLabel => style(11.0, FontWeight::Medium, TEXT),
      Self::Brief => leading(1.7, style(13.0, FontWeight::Normal, TEXT)),
      Self::DialogTitle => style(20.0, FontWeight::Medium, TEXT),
      Self::Explanation => leading(1.6, style(12.0, FontWeight::Normal, MUTED)),
      Self::Instructions => leading(1.65, style(12.0, FontWeight::Normal, TEXT)),
      Self::Hint => leading(1.5, style(10.0, FontWeight::Normal, MUTED)),
      Self::ReadBack => style(10.0, FontWeight::Normal, SUBTLE),
      Self::VersionFigure => style(18.0, FontWeight::Medium, TEXT),
      Self::Summary => leading(1.6, style(13.0, FontWeight::Normal, MUTED)),
      Self::CompactStatusLabel => style(10.0, FontWeight::Medium, TEXT),
      Self::InspectorTitle => style(18.0, FontWeight::Medium, TEXT),
      Self::RowTitle => style(15.0, FontWeight::Medium, TEXT),
      Self::PullRequestTitle => style(24.0, FontWeight::Medium, TEXT),
      Self::FileListHeading => style(10.0, FontWeight::Medium, MUTED),
      Self::FileName => style(12.0, FontWeight::Normal, TEXT),
      Self::RuleTitle => style(20.0, FontWeight::Medium, TEXT),
      Self::ContextNote => style(9.0, FontWeight::Normal, MUTED),
      Self::CellLabel => leading(1.5, style(11.0, FontWeight::Normal, MUTED)),
      Self::WrappingError => leading(1.5, style(11.0, FontWeight::Normal, TEXT)),
      Self::WrappingProperty => leading(1.6, style(11.0, FontWeight::Normal, TEXT)),
      Self::InspectorNote => leading(1.6, style(10.0, FontWeight::Normal, MUTED)),
      Self::EnforcementSetting => leading(1.25, style(12.0, FontWeight::Normal, TEXT)),
      Self::EnforcementMechanism => leading(1.3, mono(10.0, MUTED)),
      Self::PlanTitle => style(19.0, FontWeight::Medium, TEXT),
      Self::ReviewNote => leading(1.6, style(12.0, FontWeight::Normal, TEXT)),
      Self::AgentText => leading(1.6, style(12.0, FontWeight::Normal, TEXT)),
      Self::ColumnHeading => style(9.0, FontWeight::Normal, MUTED),
      Self::ChangeLine => leading(1.4, style(12.0, FontWeight::Normal, TEXT)),
      Self::Criterion => leading(1.5, style(12.0, FontWeight::Normal, TEXT)),
      Self::TabInitial => style(9.0, FontWeight::SemiBold, TEXT),
      Self::TimelineMessage => leading(1.6, style(13.0, FontWeight::Normal, TEXT)),
      Self::ResultHeading => leading(1.5, style(12.0, FontWeight::Normal, TEXT)),
      Self::Caution => leading(1.5, style(11.0, FontWeight::Normal, TEXT)),
      Self::HintHeading => style(11.0, FontWeight::Medium, TEXT),
      icon_role => icon(
        icon_role
          .icon_size()
          .expect("every role without a text style is an icon"),
      ),
    }
  }
}
