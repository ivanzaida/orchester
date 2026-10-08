//! *Where tokens go* as tables (`kIsZS`, `v9L3oZ`, `Ncyg7`): each breakdown's rows with exact numbers.
//! *Spent tokens* and *Share* are each as wide as their heading or their widest figure in that table,
//! measured with the theme's fonts as *Most expensive* measures its narrow columns, at every width, so
//! no figure is cut; the name takes what is left, on one line, and gives way with an ellipsis. Columns
//! are 8 apart below lurq's `Xl` breakpoint and 16 from it, as in *Most expensive*.

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::TypographyStyle},
  components::{Stack, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{BreakdownKey, BreakdownRow, Breakdowns, percent};

use super::{
  breakdown::{label, reviewer_name},
  expensive_columns::Layout,
  format,
  measure::{Measurer, Piece, Shape},
  state::Dashboard,
  table::{Cell, Width, column, fit, keyed_data_row, spaced_table},
};
use crate::theme::ExtraTypography;

/// One of the card's breakdowns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Breakdown {
  Projects,
  Profiles,
  Models,
  Pipelines,
}

impl Breakdown {
  pub fn title_key(self) -> &'static str {
    match self {
      Self::Projects => "usage.breakdown.byProject",
      Self::Profiles => "usage.breakdown.byProfile",
      Self::Models => "usage.breakdown.byModel",
      Self::Pipelines => "usage.breakdown.byPipeline",
    }
  }

  pub fn rows(self, breakdowns: &Breakdowns) -> &[BreakdownRow] {
    match self {
      Self::Projects => &breakdowns.projects,
      Self::Profiles => &breakdowns.profiles,
      Self::Models => &breakdowns.models,
      Self::Pipelines => &breakdowns.pipelines,
    }
  }

  /// The table's id; its rows are `{id}-row-{index}`, and a row's name `{id}-row-{index}-name`.
  fn table_id(self) -> &'static str {
    match self {
      Self::Projects => "usage-breakdown-table-projects",
      Self::Profiles => "usage-breakdown-table-profiles",
      Self::Models => "usage-breakdown-table-models",
      Self::Pipelines => "usage-breakdown-table-pipelines",
    }
  }
}

#[derive(Clone, PartialEq)]
pub struct BreakdownTableProps {
  pub dashboard: Arc<Dashboard>,
  pub breakdown: Breakdown,
}

impl lurq::app::component::DevtoolsInspectable for BreakdownTableProps {}

pub struct BreakdownTable {
  measurer: Measurer,
}

impl Component for BreakdownTable {
  type Props = BreakdownTableProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      measurer: Measurer::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<BreakdownTableProps>().clone();
    let report = &props.dashboard.report;
    let rows = props.breakdown.rows(&report.breakdowns);
    let spent = report.summary.spent();
    let figures: Vec<[Arc<str>; 2]> = rows
      .iter()
      .map(|row| {
        [
          format::exact(ctx, row.total()),
          format::share(ctx, percent(row.total(), spent)),
        ]
      })
      .collect();
    let headings = [ctx.t("usage.table.spent"), ctx.t("usage.table.share")];
    let shapes = (0..headings.len())
      .map(|index| figure_shapes(ctx, &headings[index], figures.iter().map(|row| &row[index])))
      .collect();
    let [spent_heading, share_heading] = headings;
    // The first layout of new figures has no widths yet, and keeps the shares the table had before.
    let columns = match self.measurer.widths(shapes).as_deref() {
      Some(&[spent, share]) => [
        column(ctx.t("usage.table.name"), Width::Flex(1.0), false),
        column(spent_heading, Width::Px(spent), true),
        column(share_heading, Width::Px(share), true),
      ],
      _ => [
        column(ctx.t("usage.table.name"), Width::Flex(1.4), false),
        column(spent_heading, Width::Flex(1.0), true),
        column(share_heading, Width::Flex(0.6), true),
      ],
    };
    let gap = Layout::of(ctx).gap();
    let id = props.breakdown.table_id();
    let rows = rows
      .iter()
      .zip(figures)
      .enumerate()
      .map(|(index, (entry, [spent, share]))| {
        let row = format!("{id}-row-{index}");
        let name = name(ctx, &entry.key, format!("{row}-name"));
        keyed_data_row(&row, &columns, vec![name, Cell::Text(spent), Cell::Text(share)], gap)
      })
      .collect();
    Stack::new()
      .width(Dimension::Pct(100.0))
      .child(spaced_table(id, &columns, rows, gap))
      .child(self.measurer.canvas())
  }

  fn after_layout(&self) {
    self.measurer.after_layout();
  }
}

/// A figure column for the measurer: its heading in the heading style, then its figures in mono.
fn figure_shapes<'a>(ctx: &Ctx, heading: &Arc<str>, figures: impl Iterator<Item = &'a Arc<str>>) -> Vec<Shape> {
  std::iter::once(Shape::text(Piece::new(ctx, heading.clone(), TypographyStyle::Caption)))
    .chain(figures.map(|figure| Shape::text(Piece::new(ctx, figure.clone(), ExtraTypography::MonoValue.role()))))
    .collect()
}

/// A row's name, `id`: a model in mono as in the Chart view. The reviewer's keeps its model in sight and is
/// named by its model instead (`usage-breakdown-table-reviewer-{model}`).
fn name(ctx: &Ctx, key: &BreakdownKey, id: String) -> Cell {
  if let BreakdownKey::Reviewer(model) = key {
    return Cell::Element(reviewer_name(
      ctx,
      (TypographyStyle::Body, model),
      format!("usage-breakdown-table-reviewer-{model}"),
    ));
  }
  let (name, mono) = label(ctx, key);
  let text = Text::new(&name).id(id);
  Cell::Element(fit(if mono {
    text.variant(ExtraTypography::MonoValue.role())
  } else {
    text.variant(TypographyStyle::Body)
  }))
}

pub fn breakdown_table(ctx: &mut Ctx, dashboard: &Arc<Dashboard>, breakdown: Breakdown) -> Element {
  ctx.mount_keyed::<BreakdownTable>(
    breakdown.table_id(),
    BreakdownTableProps {
      dashboard: Arc::clone(dashboard),
      breakdown,
    },
  )
}
