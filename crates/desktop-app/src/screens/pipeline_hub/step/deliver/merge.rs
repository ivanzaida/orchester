//! *Merge* of the Deliver step (`Merge` `a6boL` in P15 `nPaMj`): *Merge method*, *Merge commit* or *Squash*, with what
//! the chosen one does under it; *Squash* says what its commit is titled and what its body holds.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, HitTestBehavior, dimension::Dimension},
};
use orchester_core::resource::{DeliverStep, MergeMethod};

use super::{Open, change};
use crate::{
  screens::team_hub::rows,
  theme::{DISABLED_OPACITY, HubText},
  ui::{
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

pub(super) fn group(ctx: &mut Ctx, open: &Open<'_>, deliver: &DeliverStep) -> Element {
  let label = ctx.t("pipeline_hub.deliver.method");
  let row = rows::row(
    "pipeline-hub-deliver-method-row",
    &label,
    None,
    value(ctx, open, deliver),
  );
  rows::section(
    "pipeline-hub-deliver-merge",
    &ctx.t("pipeline_hub.deliver.merge"),
    Vec::new(),
    vec![row],
  )
  .into()
}

/// The two methods as segments, and what the chosen one does under them, 8 apart (`Value` `z9iH1R`).
fn value(ctx: &Ctx, open: &Open<'_>, deliver: &DeliverStep) -> Column {
  let segments = [
    (
      MergeMethod::MergeCommit,
      "pipeline-hub-deliver-merge-commit",
      "pipeline_hub.deliver.mergeCommit",
    ),
    (
      MergeMethod::Squash,
      "pipeline-hub-deliver-squash",
      "pipeline_hub.deliver.squash",
    ),
  ]
  .map(|(method, id, label)| {
    let (edit, locked) = (open.edit.clone(), open.locked);
    Segment::new(
      id,
      SegmentFace::Text(ctx.t(label)),
      deliver.method() == method,
      move || {
        if !locked {
          change(&edit, |deliver| deliver.with_method(method));
        }
      },
    )
  });
  let control = segmented("pipeline-hub-deliver-method", segments, Focus::Keyboard)
    .hit_test(if open.locked {
      HitTestBehavior::None
    } else {
      HitTestBehavior::Auto
    })
    .opacity(if open.locked { DISABLED_OPACITY } else { 1.0 });
  let hints = match deliver.method() {
    MergeMethod::MergeCommit => vec![hint(
      ctx,
      "pipeline-hub-deliver-method-hint",
      "pipeline_hub.deliver.mergeCommitHint",
    )],
    MergeMethod::Squash => vec![
      hint(
        ctx,
        "pipeline-hub-deliver-method-hint",
        "pipeline_hub.deliver.squashTitle",
      ),
      hint(
        ctx,
        "pipeline-hub-deliver-squash-body",
        "pipeline_hub.deliver.squashBody",
      ),
    ],
  };
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .child(control)
    .with_children(hints)
}

/// A line under the segments, in `text_muted`.
fn hint(ctx: &Ctx, id: &str, key: &str) -> Text {
  Text::new(&ctx.t(key))
    .id(id.to_owned())
    .variant(HubText::Status.role())
    .color(lurq::app::theme::PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
}
