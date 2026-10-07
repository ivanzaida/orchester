//! How the Usage dashboard draws its charts on a lurq canvas (`C2pTYF`, `e5PbG`, "Chart colors"):
//! the series color follows the CLI, the grid is `hairline`, the baseline and reset rules `border`,
//! axis text `subtle` 10 (figures and clock times mono, days proportional), the warning threshold
//! `muted` dashed, single-series marks `muted` /
//! `subtle`; text never takes a series color. Bars are stacked with a 2px gap and a 4px rounded
//! top; weekly windows are solid lines, 5-hour windows dashed, a stretch without reports dotted
//! and 1px.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  canvas::CanvasFont,
  layout::text_style::TextStyle,
  node::color::Color,
};

use super::{ExtraColor, ExtraTypography};

/// Room for the y axis labels at the left and the x axis labels below the plot.
pub const AXIS_GUTTER: f32 = 44.0;
pub const AXIS_BAND: f32 = 20.0;
/// Room above the plot for its top label, and between an axis label and the plot.
pub const PLOT_TOP: f32 = 8.0;
pub const LABEL_GAP: f32 = 6.0;
/// The canvas of *Tokens over time* and *Plan limits*: a plot 190 tall from its top grid line to
/// the baseline (`C2pTYF`), with the room for its labels above and below.
pub const CHART_HEIGHT: f32 = 190.0 + PLOT_TOP + AXIS_BAND;
/// The canvas of the cache share line.
pub const SMALL_PLOT_HEIGHT: f32 = 120.0;
/// Between the stacked parts of a bar, and the rounding of its top.
pub const BAR_GAP: f32 = 2.0;
pub const BAR_RADIUS: f32 = 4.0;
/// The share of its slot a bar takes.
pub const BAR_FILL: f32 = 0.6;
pub const LINE_WIDTH: f32 = 1.5;
pub const GAP_LINE_WIDTH: f32 = 1.0;
pub const RULE_WIDTH: f32 = 1.0;
pub const POINT_RADIUS: f32 = 3.0;
pub const DASH: [f32; 2] = [4.0, 3.0];
pub const DOT: [f32; 2] = [1.0, 3.0];
pub const HATCH_PITCH: f32 = 6.0;
/// A hover tooltip's width (`BLY4R`) and how far it keeps from the pointer.
pub const TOOLTIP_WIDTH: f32 = 220.0;
pub const TOOLTIP_OFFSET: f32 = 12.0;
/// A legend's color swatch, and the height of a *Where tokens go* bar.
pub const SWATCH_SIZE: f32 = 8.0;
/// A window's line in the *Plan limits* legend: 14 by 2, dashed in three pieces for 5-hour windows.
pub const LINE_SWATCH_WIDTH: f32 = 14.0;
pub const LINE_SWATCH_HEIGHT: f32 = 2.0;
pub const DASH_PIECES: usize = 3;
pub const BREAKDOWN_BAR_HEIGHT: f32 = 8.0;
/// The share of the cache share plot's height 100 % takes, so the label over a point near it stays
/// in the canvas.
pub const HEADROOM: f32 = 0.9;

/// The chart colors and the axis font, resolved from the theme for drawing.
#[derive(Clone)]
pub struct ChartStyle {
  /// Codex, Claude Code, a third CLI.
  pub series: [Color; 3],
  pub grid: Color,
  pub rule: Color,
  pub axis: Color,
  pub threshold: Color,
  pub single: Color,
  pub single_light: Color,
  pub hatch: Color,
  pub band: Color,
  /// Figures and clock times on the axes.
  pub font: CanvasFont,
  /// Days on the x axis.
  pub day_font: CanvasFont,
}

impl ChartStyle {
  pub fn of(ctx: &Ctx) -> Self {
    let theme = ctx.theme();
    let color = |role: PaletteColor| theme.palette_color(role);
    let canvas_font = |style: &TextStyle| {
      let mut font = CanvasFont::new(Arc::clone(&style.font_family), style.font_size);
      font.weight = style.weight;
      font
    };
    let font = canvas_font(&theme.typography_style(TypographyStyle::Mono));
    let day_font = canvas_font(&theme.typography_style(ExtraTypography::AxisLabel.role()));
    Self {
      series: [
        color(ExtraColor::ChartSeries1.role()),
        color(ExtraColor::ChartSeries2.role()),
        color(ExtraColor::ChartSeries3.role()),
      ],
      grid: color(ExtraColor::Hairline.role()),
      rule: color(PaletteColor::Border),
      axis: color(PaletteColor::TextMuted),
      threshold: color(PaletteColor::TextSecondary),
      single: color(PaletteColor::TextSecondary),
      single_light: color(PaletteColor::TextMuted),
      hatch: color(ExtraColor::ChartHatch.role()),
      band: color(PaletteColor::AccentMuted),
      font,
      day_font,
    }
  }
}
