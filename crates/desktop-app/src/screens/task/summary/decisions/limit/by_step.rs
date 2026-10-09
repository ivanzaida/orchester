//! *Tokens by step* (`Tokens by step` `wsQa7`): the tokens each step's agent used in this run, one detail
//! row per step that used any (`By step` `QuX1T`), the step that stopped in `warning` with its attempt and
//! that it stopped mid-step. A tool step's command uses no tokens and is left out.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{time::Timestamp, tokens::compact_tokens};
use orchester_workspace::tasks::progress::{ProgressRow, ProgressState, progress};

use super::{
  super::{super::section_head, step_label},
  Stop,
};
use crate::{
  theme::ExtraTypography,
  ui::{
    card::{CardSurface, card},
    detail_row::detail_row_of,
  },
};

pub fn section(ctx: &mut Ctx, stop: &Stop<'_>, now: Timestamp) -> Option<Element> {
  let rows: Vec<Element> = progress(stop.run, false, now)
    .into_iter()
    .filter_map(|row| step_row(ctx, stop, &row))
    .collect();
  if rows.is_empty() {
    return None;
  }
  let list = card("raise-limit-by-step", CardSurface::Outline)
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Sm))
    .with_children(rows);
  Some(
    Column::new()
      .id("raise-limit-tokens-by-step")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(section_head(
        ctx,
        "raise-limit-by-step-head",
        "decisions.limit.byStepTitle",
        None,
      ))
      .child(list)
      .into(),
  )
}

/// "03 Implement · 1.31M tokens · Builder · attempt 2 · stopped mid-step", for a step that used tokens.
fn step_row(ctx: &Ctx, stop: &Stop<'_>, row: &ProgressRow) -> Option<Element> {
  let usage = row.usage?;
  let tokens = usage.tokens.filter(|tokens| *tokens > 0)?;
  let step = stop.run.steps.iter().find(|step| *step.key.as_str() == *row.id)?;
  let agent = row.agent.as_deref().unwrap_or_default();
  let used = compact_tokens(tokens);
  let stopped = row.state == ProgressState::StoppedAtLimit;
  let (value, color) = if stopped {
    let attempt = usage.attempt.to_string();
    let value = ctx.t_args(
      "decisions.limit.byStepStopped",
      [
        ("tokens", used.as_str()),
        ("agent", agent),
        ("attempt", attempt.as_str()),
      ],
    );
    (value, PaletteColor::Warning)
  } else {
    let value = ctx.t_args("decisions.limit.byStep", [("tokens", used.as_str()), ("agent", agent)]);
    (value, PaletteColor::TextPrimary)
  };
  let id = format!("raise-limit-step-{}", row.id);
  let text = Text::new(&value)
    .id(id.clone())
    .variant(ExtraTypography::Prose.role())
    .color(color)
    .flex(1.0);
  Some(detail_row_of(&id, &step_label(ctx, step), text).into())
}
