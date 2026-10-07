//! *Most expensive*'s columns: as `sOQmy` draws them at lurq's widest breakpoint, or narrower, which
//! the design does not draw, with each fixed column as wide as its heading or its widest cell,
//! measured with the theme's fonts, the columns 8 apart and a run's title under its key, so titles
//! and pipelines keep readable room and no heading is cut.

use std::sync::Arc;

use lurq::app::{
  ctx::Ctx,
  theme::{Breakpoint, SpacingSize, TypographyStyle},
};
use orchester_core::usage::RunOutcome;

use super::{
  format,
  measure::{Measurer, Piece, Shape},
  table::{TableColumn, Width, column},
};
use crate::{
  theme::ExtraTypography,
  ui::{
    icon::{Icon, IconSize},
    status_label::compact_room,
  },
};

/// Fixed column widths (`sOQmy`): a count, a token figure, a time, the outcome label, the chevron.
pub const COUNT_WIDTH: f32 = 56.0;
pub const TOKENS_WIDTH: f32 = 108.0;
pub const TIME_WIDTH: f32 = 104.0;
const OUTCOME_WIDTH: f32 = 132.0;
const CHEVRON_WIDTH: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
  Wide,
  Narrow,
}

impl Layout {
  pub fn of(ctx: &Ctx) -> Self {
    if matches!(ctx.breakpoint(), Some(Breakpoint::Xl)) {
      Self::Wide
    } else {
      Self::Narrow
    }
  }

  pub fn gap(self) -> SpacingSize {
    match self {
      Self::Wide => SpacingSize::Md,
      Self::Narrow => SpacingSize::Sm,
    }
  }

  /// Whether a run's title sits under its key rather than beside it.
  pub fn stacked(self) -> bool {
    self == Self::Narrow
  }
}

/// A fixed column: its heading, its width as `sOQmy` draws it, whether it holds figures, and what
/// its cells show, for the narrow layout to measure.
pub struct Fixed {
  heading: Arc<str>,
  width: f32,
  figures: bool,
  cells: Vec<Shape>,
}

impl Fixed {
  /// Figures in mono at the right.
  pub fn figures(ctx: &Ctx, heading: Arc<str>, width: f32, values: impl Iterator<Item = Arc<str>>) -> Self {
    let cells = values
      .map(|value| Shape::text(Piece::new(ctx, value, ExtraTypography::MonoValue.role())))
      .collect();
    Self {
      heading,
      width,
      figures: true,
      cells,
    }
  }

  /// Text at the left, e.g. when a run started.
  pub fn text(ctx: &Ctx, heading: Arc<str>, width: f32, values: impl Iterator<Item = Arc<str>>) -> Self {
    let cells = values
      .map(|value| Shape::text(Piece::new(ctx, value, TypographyStyle::Body)))
      .collect();
    Self {
      heading,
      width,
      figures: false,
      cells,
    }
  }

  /// *Outcome*: each label's icon and word on its fill.
  pub fn outcome(ctx: &Ctx, outcomes: impl Iterator<Item = RunOutcome>) -> Self {
    let room = compact_room(ctx);
    let cells = outcomes
      .map(|outcome| {
        let label = ctx.t(format::outcome_key(outcome));
        let pieces = vec![
          glyph(ctx, outcome_glyph(outcome)),
          Piece::new(ctx, label, ExtraTypography::CompactStatusLabel.role()),
        ];
        Shape::new(pieces, room)
      })
      .collect();
    Self {
      heading: ctx.t("usage.table.outcome"),
      width: OUTCOME_WIDTH,
      figures: false,
      cells,
    }
  }

  /// The chevron that says a row opens.
  pub fn chevron(ctx: &Ctx) -> Self {
    Self {
      heading: Arc::from(""),
      width: CHEVRON_WIDTH,
      figures: false,
      cells: vec![Shape::text(glyph(ctx, Icon::ChevronRight))],
    }
  }
}

fn glyph(ctx: &Ctx, icon: Icon) -> Piece {
  let text: Arc<str> = icon.glyph().to_string().into();
  Piece::new(ctx, text, IconSize::Small.role().role())
}

/// The icon of an outcome's label.
pub fn outcome_glyph(outcome: RunOutcome) -> Icon {
  match outcome {
    RunOutcome::Pending => Icon::Clock3,
    RunOutcome::Running => Icon::Loader,
    RunOutcome::Paused | RunOutcome::Interrupted => Icon::CirclePause,
    RunOutcome::Waiting => Icon::MessageCircle,
    RunOutcome::Succeeded => Icon::CircleCheck,
    RunOutcome::Failed => Icon::CircleX,
    RunOutcome::Cancelled => Icon::CircleSlash,
  }
}

/// `flexible` then `fixed`: the fixed columns at their design widths when wide; when narrow at
/// their measured widths, which the first layout of new cells does not have yet and so takes the
/// design widths for.
pub fn columns(
  ctx: &Ctx,
  (layout, measurer): (Layout, &Measurer),
  mut flexible: Vec<TableColumn>,
  fixed: Vec<Fixed>,
) -> Vec<TableColumn> {
  let measured = match layout {
    Layout::Wide => None,
    Layout::Narrow => {
      let shapes = fixed
        .iter()
        .map(|column| {
          let heading = Shape::text(Piece::new(ctx, column.heading.clone(), TypographyStyle::Caption));
          std::iter::once(heading).chain(column.cells.iter().cloned()).collect()
        })
        .collect();
      measurer.widths(shapes)
    }
  };
  flexible.extend(fixed.into_iter().enumerate().map(|(index, fixed)| {
    let width = measured
      .as_ref()
      .and_then(|widths| widths.get(index).copied())
      .unwrap_or(fixed.width);
    column(fixed.heading, Width::Px(width), fixed.figures)
  }));
  flexible
}
