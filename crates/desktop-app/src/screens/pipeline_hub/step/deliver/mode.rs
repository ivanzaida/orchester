//! *Mode* of the Deliver step (`Mode` `vG2x0` in P15 `nPaMj`): a radio row per way a passing run is delivered, each its
//! name over what it does. *Approve to merge* and *Auto-merge* can be chosen; *Pull request* is faded with *Coming
//! later* and cannot be.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{DeliverMode, DeliverStep};

use super::{Open, change};
use crate::{
  screens::{hub::agent_row::tag_chip, team_hub::rows},
  theme::{DISABLED_OPACITY, ExtraSpacing, ExtraTypography, HubText},
  ui::{
    pressable::{Focus, pressable},
    toggle::radio_mark,
  },
};

/// A row of the group: its id, name and what it does.
struct Way {
  id: &'static str,
  name: &'static str,
  does: &'static str,
}

const APPROVE: Way = Way {
  id: "pipeline-hub-deliver-mode-approve",
  name: "pipeline_hub.deliver.approve",
  does: "pipeline_hub.deliver.approveBody",
};
const AUTO: Way = Way {
  id: "pipeline-hub-deliver-mode-auto",
  name: "pipeline_hub.deliver.auto",
  does: "pipeline_hub.deliver.autoBody",
};
const PULL_REQUEST: Way = Way {
  id: "pipeline-hub-deliver-mode-pull-request",
  name: "pipeline_hub.deliver.pullRequest",
  does: "pipeline_hub.deliver.pullRequestBody",
};

pub(super) fn group(ctx: &mut Ctx, open: &Open<'_>, deliver: &DeliverStep) -> Element {
  let rows = vec![
    choice(ctx, open, &APPROVE, (DeliverMode::ApproveToMerge, deliver.mode())),
    choice(ctx, open, &AUTO, (DeliverMode::AutoMerge, deliver.mode())),
    coming_later(ctx),
  ];
  rows::section(
    "pipeline-hub-deliver-mode",
    &ctx.t("pipeline_hub.deliver.mode"),
    Vec::new(),
    rows,
  )
  .into()
}

/// A mode that can be chosen: the radio, on when it is `chosen`; pressing it makes it the step's mode. While nothing
/// can be changed it keeps its place, faded.
fn choice(ctx: &Ctx, open: &Open<'_>, way: &Way, (mode, chosen): (DeliverMode, DeliverMode)) -> Row {
  let on = mode == chosen;
  let edit = open.edit.clone();
  let locked = open.locked;
  let button = Focus::Keyboard
    .apply(pressable(move || {
      if !locked {
        change(&edit, |deliver| deliver.with_mode(mode));
      }
    }))
    .id(way.id.to_owned())
    .describe("state", if on { "chosen" } else { "other" })
    .describe("a11y_name", ctx.t(way.name))
    .hit_test(if locked {
      HitTestBehavior::None
    } else {
      HitTestBehavior::Auto
    })
    .opacity(if locked { DISABLED_OPACITY } else { 1.0 })
    .width(Dimension::Pct(100.0))
    .child(face(ctx, way, on));
  Row::new().width(Dimension::Pct(100.0)).child(button)
}

/// *Pull request*: faded, its radio off, with *Coming later* at its end; it does nothing.
fn coming_later(ctx: &Ctx) -> Row {
  let id = PULL_REQUEST.id;
  Row::new()
    .id(id.to_owned())
    .describe("state", "unavailable")
    .width(Dimension::Pct(100.0))
    .opacity(DISABLED_OPACITY)
    .child(face(ctx, &PULL_REQUEST, false).child(tag_chip(
      &format!("{id}-tag"),
      &ctx.t("pipeline_hub.deliver.comingLater"),
    )))
}

/// The radio and the words beside it (`Approve to merge` `Vu3zX`): 10 over and under, 12 at the sides and between.
fn face(ctx: &Ctx, way: &Way, on: bool) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(radio_mark(on))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(&ctx.t(way.name))
            .id(format!("{}-name", way.id))
            .variant(ExtraTypography::OptionTitle.role()),
        )
        .child(
          Text::new(&ctx.t(way.does))
            .id(format!("{}-does", way.id))
            .variant(HubText::Status.role()),
        ),
    )
}
