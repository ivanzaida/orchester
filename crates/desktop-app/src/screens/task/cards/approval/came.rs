//! Why a tool request came to the person by the run's approval mode (`Reason` `R332d` in `Card · outside the worktree`
//! `vO7CW`, and the cards beside it in 04c `vZcoN`): a 12 `warning` info glyph and, 8 from it, the reason in 12.5
//! `text_secondary` — a path outside the worktree (with the path), a request not on the agent's allowed list, the
//! reviewer unsure or denying it (with its words), a push to a remote, another ref (with the run's branch).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{CameToPerson, PersonReason};

use super::super::Place;
use crate::{
  theme::{ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The reason line `{id}-came` of a request of `agent` that `came` to the person, at `place`.
pub(super) fn line(ctx: &Ctx, id: &str, (place, agent): (&Place, &str), came: &CameToPerson) -> Element {
  Row::new()
    .id(format!("{id}-came"))
    .describe("reason", came.reason.as_str())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::Warning))
    .child(
      Text::new(&words(ctx, (place, agent), came))
        .id(format!("{id}-came-text"))
        .variant(ExtraTypography::Lead.role())
        .selectable(true)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The reason in words, with the path, the reviewer's words or the run's branch it names.
fn words(ctx: &Ctx, (place, agent): (&Place, &str), came: &CameToPerson) -> Arc<str> {
  let detail = came.detail.as_deref();
  match (came.reason, detail) {
    (PersonReason::Outside, Some(path)) => ctx.t_args(
      "cards.approval.cameToYou.outsidePath",
      [("path", path), ("agent", agent)],
    ),
    (PersonReason::Outside, None) => ctx.t_args("cards.approval.cameToYou.outside", [("agent", agent)]),
    (PersonReason::NotAllowed, _) => ctx.t_args("cards.approval.cameToYou.notAllowed", [("agent", agent)]),
    (PersonReason::ReviewerUnsure, Some(reason)) => {
      ctx.t_args("cards.approval.cameToYou.reviewerUnsure", [("reason", reason)])
    }
    (PersonReason::ReviewerUnsure, None) => ctx.t("cards.approval.cameToYou.reviewerUnsureSilent"),
    (PersonReason::ReviewerDenied, Some(reason)) => ctx.t_args(
      "cards.approval.cameToYou.reviewerDenied",
      [("reason", reason), ("agent", agent)],
    ),
    (PersonReason::ReviewerDenied, None) => {
      ctx.t_args("cards.approval.cameToYou.reviewerDeniedSilent", [("agent", agent)])
    }
    (PersonReason::Push, _) => ctx.t("cards.approval.cameToYou.push"),
    (PersonReason::Refs, _) => {
      let branch = place.detail.run.as_ref().map_or("", |run| run.run.branch().as_str());
      ctx.t_args("cards.approval.cameToYou.refs", [("branch", branch)])
    }
  }
}
