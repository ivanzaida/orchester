//! The design values of the project's Usage page (layer `Usage` `et35n`: `Usage · project · dark` `CFRdj`,
//! its table view `d3lc3`, states `cK32U` and interactions `p732q`) that the UI kit's roles do not carry,
//! registered as lurq `Extra` roles; and the colors and fonts its canvas charts draw with, resolved from
//! the theme.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, Theme, ThemePalette, TypographyStyle},
  },
  canvas::CanvasFont,
  layout::text_style::{FontWeight, TextStyle},
  node::color::Color,
};
use orchester_core::usage::SERIES;

use super::{
  ExtraColor,
  fonts::ICONS,
  typography::{mono, style},
};

/// Text of the Usage page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageText {
  /// Mono 10 `text_muted`: a chart's figures and clock times (`Tick 2M` in `t2E26`, `Threshold label`
  /// `YfvNp`).
  Axis,
  /// 10 `text_muted`: a day under a chart (`X Wed 24` in `y5aeqR`), *No Claude data* (`dpVZL`).
  AxisDay,
  /// 10/500 `text_secondary`: *Today* under a chart (`X Today` `bOe4q`), a line's direct label (`HeWml`).
  AxisStrong,
  /// 11 `text_muted`, 1.5: a footnote under a chart (`Footnote` `LW81Y`).
  Footnote,
  /// 11/500 `text_muted`, tracking 0.4, upper case: a table's column heading (`DAY` `PxHBc`, `TASK`
  /// `uuYPZ`).
  ColumnHeading,
  /// 12 `text_muted`, 1.4: what a card's rows mean under its title (`Subtitle` `Pde5c`, `UBN8h`).
  Subtitle,
  /// 13 `text_secondary`, 1.3: what the page shows under its title (`Description` `f8ubk`).
  Description,
  /// 22/600 `text_primary`, tracking -0.3: an efficiency figure (`Value` `UP0UG`).
  Figure,
  /// Mono 12/600 `text_primary`: a figure of a table's *Total* row (`b2wVz` in `fxX9i`).
  TotalFigure,
  /// 13/600 `text_primary`, tracking -0.1: an efficiency card's title (`ZlfOf` in `cYwPd`).
  SmallTitle,
  /// 13/600 `text_primary`: what a card's state says (`No runs in Kontur today` `zsdqb`).
  StateTitle,
  /// Lucide 20: a card's state glyph (`Q4vty1`).
  StateGlyph,
}

impl UsageText {
  pub const ALL: [Self; 12] = [
    Self::Axis,
    Self::AxisDay,
    Self::AxisStrong,
    Self::Footnote,
    Self::ColumnHeading,
    Self::Subtitle,
    Self::Description,
    Self::Figure,
    Self::TotalFigure,
    Self::SmallTitle,
    Self::StateTitle,
    Self::StateGlyph,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Axis => "usage_axis",
      Self::AxisDay => "usage_axis_day",
      Self::AxisStrong => "usage_axis_strong",
      Self::Footnote => "usage_footnote",
      Self::ColumnHeading => "usage_column_heading",
      Self::Subtitle => "usage_subtitle",
      Self::Description => "usage_description",
      Self::Figure => "usage_figure",
      Self::TotalFigure => "usage_total_figure",
      Self::SmallTitle => "usage_small_title",
      Self::StateTitle => "usage_state_title",
      Self::StateGlyph => "usage_state_glyph",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Axis => mono(style(10.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::AxisDay => style(10.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::AxisStrong => style(10.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::Footnote => style(11.0, FontWeight::Normal, 1.5, 0.0, palette.text_muted),
      Self::ColumnHeading => style(11.0, FontWeight::Medium, 1.3, 0.4, palette.text_muted),
      Self::Subtitle => style(12.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
      Self::Description => style(13.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Figure => style(22.0, FontWeight::SemiBold, 1.3, -0.3, palette.text_primary),
      Self::TotalFigure => mono(style(12.0, FontWeight::SemiBold, 1.4, 0.0, palette.text_primary)),
      Self::SmallTitle => style(13.0, FontWeight::SemiBold, 1.3, -0.1, palette.text_primary),
      Self::StateTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::StateGlyph => TextStyle {
        font_family: Arc::from(ICONS),
        ..style(20.0, FontWeight::Normal, 1.0, 0.0, palette.text_muted)
      },
    }
  }
}

/// Gaps and paddings of the Usage page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageSpacing {
  /// 40: the page's sides, the toolbar's and the sections' (`Toolbar · pinned` `W7V5GX`, `Scroll` `o862EX`).
  Page,
  /// 28: above the toolbar's title (`W7V5GX`).
  Top,
  /// 20: a card's sides (`Card · Tokens over time` `w1aVOX`).
  CardSide,
  /// 36: over and under what a card says in a chart's place (`State` `i9jMW`).
  State,
  /// 7: a legend to its chart, whose room for its top label (7) makes up the card's 14 (`Legend` `CuobA`,
  /// `Chart` `nySdK`).
  AboveChart,
  /// 22: when a window resets, under its name past its line (`Resets row` `dWNPd`).
  ResetsIndent,
}

impl UsageSpacing {
  pub const ALL: [Self; 6] = [
    Self::Page,
    Self::Top,
    Self::CardSide,
    Self::State,
    Self::AboveChart,
    Self::ResetsIndent,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "usage_page",
      Self::Top => "usage_top",
      Self::CardSide => "usage_card_side",
      Self::State => "usage_state",
      Self::AboveChart => "usage_above_chart",
      Self::ResetsIndent => "usage_resets_indent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
      Self::Top => 28.0,
      Self::CardSide => 20.0,
      Self::State => 36.0,
      Self::AboveChart => 7.0,
      Self::ResetsIndent => 22.0,
    }
  }
}

/// Radii of the Usage page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageRadius {
  /// 1: a *Where tokens go* bar's part (`OUyu8` in `YAqCn`) and a window's line in a legend (`kV4s6`).
  Bar,
}

impl UsageRadius {
  pub const ALL: [Self; 1] = [Self::Bar];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Bar => "usage_bar",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Bar => 1.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  UsageText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in UsageSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
  for radius in UsageRadius::ALL {
    theme.set_radius_value(radius.role(), radius.value());
  }
}

/// `style` as a canvas draws and measures it: family, size, weight, slant, letter spacing and OpenType
/// features.
pub fn canvas_font(style: &TextStyle) -> CanvasFont {
  let mut font = CanvasFont::new(Arc::clone(&style.font_family), style.font_size);
  font.weight = style.weight;
  font.style = style.style;
  font.letter_spacing = style.letter_spacing;
  font.font_features = style.font_features.clone();
  font
}

/// The colors and fonts a Usage chart draws with (`Plot` `cgGn2`, `jPHZ9`): a series in its account's color,
/// the grid in `border`, the baseline and reset rules in `border_strong`, axis text `text_muted` (figures
/// and clock times mono, days proportional, *Today* `text_secondary` 500), the warning threshold dashed in
/// `text_muted`, a single-series line in `text_muted`; text never takes a series color.
#[derive(Clone, PartialEq)]
pub struct ChartStyle {
  /// Accounts 1–3, Other accounts, No account (its stripes and outline, over `hatch`), by
  /// `AccountSeries::index`.
  pub series: [Color; SERIES],
  pub grid: Color,
  pub baseline: Color,
  pub axis: Color,
  pub strong: Color,
  pub threshold: Color,
  pub single: Color,
  pub hatch: Color,
  /// The hovered bar's band.
  pub band: Color,
  /// The ring around a line's current point (`fPyMV`).
  pub ring: Color,
  /// Figures and clock times on the axes.
  pub font: CanvasFont,
  /// Days on the x axis.
  pub day_font: CanvasFont,
  /// *Today* and a line's direct label.
  pub strong_font: CanvasFont,
}

impl ChartStyle {
  pub fn of(ctx: &Ctx) -> Self {
    let theme = ctx.theme();
    let color = |role: PaletteColor| theme.palette_color(role);
    let font = |text: UsageText| canvas_font(&theme.typography_style(text.role()));
    Self {
      series: [
        color(ExtraColor::Chart1.role()),
        color(ExtraColor::Chart2.role()),
        color(ExtraColor::Chart3.role()),
        color(ExtraColor::ChartOther.role()),
        color(ExtraColor::ChartNone.role()),
      ],
      grid: color(PaletteColor::Border),
      baseline: color(PaletteColor::BorderStrong),
      axis: color(PaletteColor::TextMuted),
      strong: color(PaletteColor::TextSecondary),
      threshold: color(PaletteColor::TextMuted),
      single: color(PaletteColor::TextMuted),
      hatch: color(ExtraColor::ChartHatch.role()),
      band: color(ExtraColor::Hover.role()),
      ring: color(PaletteColor::SurfacePanel),
      font: font(UsageText::Axis),
      day_font: font(UsageText::AxisDay),
      strong_font: font(UsageText::AxisStrong),
    }
  }
}
