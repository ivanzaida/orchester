//! The panel of the chosen pipeline (`Detail` `KGBgh` in `Pipelines · v2` P1): its head (what it is and its version
//! over its name), the chips under it (its version, whether it can run or what it lacks, and the tasks that use it),
//! and the body, which [`super::steps`] fills. The body scrolls; the Save bar of PV2-10 will be pinned under it. The head
//! with its ⋯ menu is [`super::panel_head`]; an archived pipeline's panel is [`super::archived`].

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::{
  archived,
  model::Entry,
  panel_head::{self, Props as HeadProps},
  steps, words,
};
use crate::{
  project::ShownProject,
  screens::{local_hub::archive::Asking, team_hub::panel::body},
  theme::{ExtraSpacing, HubText},
  ui::chip::{BadgeTone, badge},
};

/// What the panel needs besides the entry.
pub(super) struct Shown<'a> {
  pub project: &'a ShownProject,
  pub now: Timestamp,
  /// Whether New task offers this pipeline first.
  pub default: bool,
  pub asking: &'a Asking,
}

/// The panel of `entry`.
pub(super) fn panel(ctx: &mut Ctx, shown: &Shown<'_>, entry: &Entry) -> Element {
  if entry.archived.is_some() {
    return archived::panel(
      ctx,
      archived::Props {
        project: shown.project.clone(),
        entry: entry.clone(),
        now: shown.now,
      },
    );
  }
  let head = panel_head::head(
    ctx,
    HeadProps {
      project: shown.project.clone(),
      entry: entry.clone(),
      default: shown.default,
      now: shown.now,
      asking: shown.asking.clone(),
    },
  );
  let body = body()
    .spacing(ExtraSpacing::Inset.role())
    .child(head)
    .child(chips(ctx, entry, shown.default))
    .with_children(steps::parts(ctx, shown.project, entry));
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
/// one, then its first problem when it has one; for an archived one that it is; last, that New task offers it first.
fn standing(ctx: &Ctx, entry: &Entry, default: bool) -> Vec<(Arc<str>, BadgeTone)> {
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
  let default = default.then(|| (ctx.t("pipeline_hub.chip.default"), BadgeTone::Info));
  version.into_iter().chain(rest).chain(default).collect()
}
