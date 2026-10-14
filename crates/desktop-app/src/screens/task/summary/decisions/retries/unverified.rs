//! *What Verify could not check*: for a run whose step reported `unverified`, each check it could not do with why,
//! in the report card the evidence uses for what a failing step reported. A step that was not run again on an
//! unchanged revision shows the list its earlier unverified result left, if it left one.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{resource::StepKey, run::UnverifiedCheck};
use orchester_workspace::tasks::detail::RunView;

use crate::{
  theme::{DecisionSpacing, DecisionText, ExtraSpacing, IconSize, TaskText},
  ui::{
    card::{CardSurface, card_frame},
    icon::{Icon, icon},
  },
};

/// The heading and the list, when the step listed anything it could not check.
pub fn section(ctx: &Ctx, run: &RunView, step: &StepKey) -> Option<Element> {
  let checks = run.unverified();
  if checks.is_empty() {
    return None;
  }
  let name = run
    .steps
    .iter()
    .find(|view| view.key == *step)
    .map_or_else(|| step.as_str().to_owned(), |view| view.name.to_string());
  let head = Text::new(&ctx.t_args("decisions.retries.unverifiedHeading", [("step", name.as_str())]))
    .id("task-unverified-head")
    .variant(TaskText::SectionTitle.role());
  // Rows a hairline apart, as the latest attempt's steps.
  let list = card_frame("task-unverified", CardSurface::Raised)
    .with_children(checks.iter().enumerate().map(|(index, check)| check_row(index, check)));
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(head)
      .child(list)
      .into(),
  )
}

/// A check it could not do: the mark on the line of its name, and why under the name.
fn check_row(index: usize, check: &UnverifiedCheck) -> Column {
  let name = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleDashed, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&check.check)
        .variant(DecisionText::Subhead.role())
        .flex(1.0)
        .selectable(true),
    );
  let row = Column::new()
    .id(format!("task-unverified-{index}").as_str())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Tight.role())
    .child(name)
    // A check listed without a reason has no line for one.
    .with_children((!check.reason.is_empty()).then(|| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding_left(DecisionSpacing::CheckIndent.role())
        .child(
          Text::new(&check.reason)
            .variant(TypographyStyle::Description)
            .flex(1.0)
            .selectable(true),
        )
    }));
  if index > 0 {
    row.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
  } else {
    row
  }
}
