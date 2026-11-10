//! The chips under a panel's head (`Chips` in `Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): its version, whether it can
//! run or what it lacks, then *used by 13 tasks* at their end. A pipeline that is ready says *Ready* in `success`; one
//! with a draft stored says *draft* in `neutral` after its version, and a draft never published has a warning chip with
//! how many of its steps are not ready. A published pipeline's status chip is about the version tasks use.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::dimension::Dimension,
};

use super::{model::Entry, step_line::steps_not_ready, words};
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

/// How the pipeline stands, as the chips: its version; *draft* while a draft is stored (a pipeline's panel shows its
/// draft, though its list row and these chips speak of the version tasks use); then that it is ready or what it lacks;
/// for an archived one that it is; last, that New task offers it first.
fn standing(ctx: &Ctx, entry: &Entry, default: bool) -> Vec<(Arc<str>, BadgeTone)> {
  let version = entry
    .number
    .map(|number| (Arc::from(number.to_string()), BadgeTone::Neutral));
  let rest = if entry.archived.is_some() {
    vec![(ctx.t("pipeline_hub.chip.archived"), BadgeTone::Neutral)]
  } else {
    let draft = has_draft(entry).then(|| (ctx.t("pipeline_hub.chip.draft"), BadgeTone::Neutral));
    let status = not_ready(ctx, entry)
      .or_else(|| (!entry.is_draft()).then(|| (ctx.t("pipeline_hub.chip.ready"), BadgeTone::Success)));
    draft.into_iter().chain(status).collect()
  };
  let default = default.then(|| (ctx.t("pipeline_hub.chip.default"), BadgeTone::Info));
  version.into_iter().chain(rest).chain(default).collect()
}

/// Whether a draft is stored, which a published pipeline can have beside its version.
fn has_draft(entry: &Entry) -> bool {
  entry.flow.as_ref().is_some_and(|read| read.draft)
}

/// The warning chip. A draft never published says how many of its steps are not ready (*1 step not ready*), else its
/// first problem as the row says it. A published pipeline says what its version lacks as the row does, whatever its
/// draft holds.
fn not_ready(ctx: &Ctx, entry: &Entry) -> Option<(Arc<str>, BadgeTone)> {
  let words = match steps_not_ready(entry) {
    0 => words::needs(ctx, entry)?,
    1 => ctx.t("pipeline_hub.chip.stepNotReady"),
    count => ctx.t_args("pipeline_hub.chip.stepsNotReady", [("count", count.to_string())]),
  };
  Some((words, BadgeTone::Warning))
}
