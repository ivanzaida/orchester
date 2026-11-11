//! The chips under a panel's head (`Chips` in `Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): its version, whether it can
//! run or what it lacks, then *used by 13 tasks* at their end. A pipeline that is ready says *Ready* in `success`, one
//! that is not what it lacks in `warning`; the status chip is about the version tasks use. While the panel holds an edit
//! some steps of which cannot run, the chips speak of the edit, which has no version yet: how many steps are not ready.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::dimension::Dimension,
};

use super::{model::Entry, words};
use crate::{
  theme::HubText,
  ui::chip::{BadgeTone, badge},
};

/// The chips under the head, then *used by 13 tasks* at their end; it ends in an ellipsis when it runs out of room.
/// `default`: New task offers the pipeline first, which a last chip says. `edited`: how many steps of the edit the panel
/// holds cannot run, while it holds changes.
pub(super) fn chips(ctx: &Ctx, entry: &Entry, (default, edited): (bool, Option<usize>)) -> Row {
  let used = entry.archived.is_none().then(|| words::used_by(ctx, entry));
  Row::new()
    .id("pipeline-hub-chips")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      standing(ctx, entry, (default, edited))
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

/// How the pipeline stands, as the chips: its version; then that it is ready or what it lacks; for an archived one that
/// it is; for an edit with steps not ready only how many; last, that New task offers it first.
fn standing(ctx: &Ctx, entry: &Entry, (default, edited): (bool, Option<usize>)) -> Vec<(Arc<str>, BadgeTone)> {
  let default = default.then(|| (ctx.t("pipeline_hub.chip.default"), BadgeTone::Info));
  if let Some(steps) = edited.and_then(|count| words::steps_not_ready(ctx, count)) {
    return [(steps, BadgeTone::Warning)].into_iter().chain(default).collect();
  }
  let version = entry
    .number
    .map(|number| (Arc::from(number.to_string()), BadgeTone::Neutral));
  let status = if entry.archived.is_some() {
    (ctx.t("pipeline_hub.chip.archived"), BadgeTone::Neutral)
  } else {
    words::needs(ctx, entry).map_or_else(
      || (ctx.t("pipeline_hub.chip.ready"), BadgeTone::Success),
      |needs| (needs, BadgeTone::Warning),
    )
  };
  version.into_iter().chain([status]).chain(default).collect()
}
