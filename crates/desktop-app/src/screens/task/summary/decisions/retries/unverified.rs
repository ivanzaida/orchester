//! *What Verify could not check*: for a run whose step reported `unverified`, each check it could not do with why,
//! in the report card the evidence uses for what a failing step reported. A step that was not run again on an
//! unchanged revision shows the list its earlier unverified result left, if it left one.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::StepKey;
use orchester_workspace::tasks::detail::RunView;

use crate::{
  theme::{DecisionText, ExtraSpacing, IconSize, TaskText},
  ui::{
    card::{CardSurface, card},
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
  let list = card("task-unverified", CardSurface::Raised)
    .spacing(SpacingSize::Md)
    .with_children(checks.iter().enumerate().map(|(index, check)| {
      Row::new()
        .id(format!("task-unverified-{index}").as_str())
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Start)
        .child(icon(Icon::CircleDashed, IconSize::Sm, PaletteColor::TextSecondary))
        .child(
          Column::new()
            .flex(1.0)
            .child(
              Text::new(&check.check)
                .variant(DecisionText::Subhead.role())
                .width(Dimension::Pct(100.0))
                .selectable(true),
            )
            .child(
              Text::new(&check.reason)
                .variant(TypographyStyle::Description)
                .width(Dimension::Pct(100.0))
                .selectable(true),
            ),
        )
    }));
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(head)
      .child(list)
      .into(),
  )
}
