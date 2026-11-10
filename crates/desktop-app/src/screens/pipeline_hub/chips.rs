//! The chips under a panel's head (`Chips` in `Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): its version, whether it can
//! run or what it lacks, then *used by 13 tasks* at their end. A pipeline that is ready says *Ready* in `success`; one
//! that was never saved as a version says *draft* in `neutral`, and a warning chip says how many of its steps are not
//! ready.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::dimension::Dimension,
};

use super::{model::Entry, ready::steps_not_ready, words};
use crate::{
  theme::HubText,
  ui::chip::{BadgeTone, badge},
};

/// The chips under the head, then *used by 13 tasks* at their end; it ends in an ellipsis when it runs out of room.
/// `default`: New task offers the pipeline first, which a last chip says.
pub(super) fn chips(ctx: &Ctx, entry: &Entry, default: bool) -> Row {
  let used = entry.archived.is_none().then(|| words::used_by(ctx, entry));
  Row::new()
    .id("pipeline-hub-chips")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      standing(ctx, entry, default)
        .into_iter()
        .enumerate()
        .map(|(index, (words, tone))| badge(&format!("pipeline-hub-chip-{index}"), tone, &words)),
    )
    .child(Row::new().flex(1.0))
    .with_children(used.map(|used| {
      Text::new(&used)
        .id("pipeline-hub-chips-end")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis)
    }))
}

/// How the pipeline stands, as the chips: its version, then that it is ready or what it lacks; for a draft that it is
/// one, then how many steps are not ready or its first problem when it has one; for an archived one that it is; last,
/// that New task offers it first.
fn standing(ctx: &Ctx, entry: &Entry, default: bool) -> Vec<(Arc<str>, BadgeTone)> {
  let version = entry
    .number
    .map(|number| (Arc::from(number.to_string()), BadgeTone::Neutral));
  let rest = if entry.archived.is_some() {
    vec![(ctx.t("pipeline_hub.chip.archived"), BadgeTone::Neutral)]
  } else if entry.is_draft() {
    let problem = not_ready(ctx, entry);
    std::iter::once((ctx.t("pipeline_hub.chip.draft"), BadgeTone::Neutral))
      .chain(problem)
      .collect()
  } else {
    vec![match not_ready(ctx, entry) {
      Some(chip) => chip,
      None => (ctx.t("pipeline_hub.chip.ready"), BadgeTone::Success),
    }]
  };
  let default = default.then(|| (ctx.t("pipeline_hub.chip.default"), BadgeTone::Info));
  version.into_iter().chain(rest).chain(default).collect()
}

/// The warning chip: *1 step not ready* when a step cannot run, else what the pipeline lacks as the row says it.
fn not_ready(ctx: &Ctx, entry: &Entry) -> Option<(Arc<str>, BadgeTone)> {
  let steps = steps_not_ready(entry);
  let words = match steps {
    0 => words::needs(ctx, entry)?,
    1 => ctx.t("pipeline_hub.chip.stepNotReady"),
    count => ctx.t_args("pipeline_hub.chip.stepsNotReady", [("count", count.to_string())]),
  };
  Some((words, BadgeTone::Warning))
}
