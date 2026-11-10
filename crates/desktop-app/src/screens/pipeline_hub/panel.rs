//! The panel of the chosen pipeline (`Detail` `KGBgh` in `Pipelines · v2` P1): its head (what it is and its version
//! over its name), the chips under it (its version, whether it can run or what it lacks, and the tasks that use it),
//! and the body, which [`super::steps`] fills. The body scrolls; the Save bar of PV2-10 will be pinned under it.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::{model::Entry, steps, words};
use crate::{
  project::ShownProject,
  screens::team_hub::panel::{body, head_named},
  theme::HubText,
  ui::chip::{BadgeTone, badge},
};

/// The panel of `entry`.
pub(super) fn panel(ctx: &mut Ctx, project: &ShownProject, entry: &Entry, now: Timestamp) -> Element {
  let head = head_named(
    &entry.name,
    (
      ctx.t("pipeline_hub.panel.where").to_string(),
      Some(words::head_version(ctx, now, entry)),
    ),
    None,
  );
  let body = body()
    .child(head)
    .child(chips(ctx, entry))
    .with_children(steps::parts(ctx, project, entry));
  let scroll = ScrollVertical::new(body)
    .id("pipeline-hub-panel-scroll")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  Column::new()
    .id("pipeline-hub-panel")
    .describe("pipeline", entry.id.to_string())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .into()
}

/// The chips under the head, then *used by 13 tasks* at their end; it ends in an ellipsis when it runs out of room.
fn chips(ctx: &Ctx, entry: &Entry) -> Row {
  let used = entry.archived.is_none().then(|| words::used_by(ctx, entry));
  Row::new()
    .id("pipeline-hub-chips")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      standing(ctx, entry)
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
/// one, then what keeps it from being published; for an archived one that it is.
fn standing(ctx: &Ctx, entry: &Entry) -> Vec<(Arc<str>, BadgeTone)> {
  let version = entry
    .number
    .map(|number| (Arc::from(number.to_string()), BadgeTone::Neutral));
  let rest = if entry.archived.is_some() {
    vec![(ctx.t("pipeline_hub.chip.archived"), BadgeTone::Neutral)]
  } else if entry.is_draft() {
    let problem = words::needs(ctx, entry).map(|words| (words, BadgeTone::Warning));
    std::iter::once((ctx.t("pipeline_hub.chip.draft"), BadgeTone::Info))
      .chain(problem)
      .collect()
  } else {
    vec![match words::needs(ctx, entry) {
      Some(needs) => (needs, BadgeTone::Warning),
      None => (ctx.t("pipeline_hub.chip.ready"), BadgeTone::Success),
    }]
  };
  version.into_iter().chain(rest).collect()
}
