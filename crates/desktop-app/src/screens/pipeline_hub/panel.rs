//! The panel of the chosen pipeline (`Detail` `KGBgh` in `Pipelines · v2` P1): its head (what it is and its version
//! over its name), the chips under it (its version, whether it can run or what it lacks, and the tasks that use it),
//! and the body, which [`super::steps`] fills. The body scrolls; the Save bar of PV2-10 will be pinned under it. The
//! head with its ⋯ menu is [`super::panel_head`]; an archived pipeline's panel is [`super::archived`].

use lurq::{
  app::ctx::Ctx,
  components::{Column, ScrollVertical},
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::{
  archived, chips,
  model::Entry,
  panel_head::{self, Props as HeadProps},
  ready, steps,
};
use crate::{
  project::ShownProject,
  screens::{local_hub::archive::Asking, team_hub::panel::body},
  theme::ExtraSpacing,
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
    .child(chips::chips(ctx, entry, shown.default))
    .with_children(steps::parts(ctx, shown.project, entry))
    .with_children(ready::group(ctx, shown.project, entry));
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
