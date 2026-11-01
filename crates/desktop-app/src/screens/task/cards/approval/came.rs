//! A tool request that came to the person by the run's approval mode, by its reason (`Card · outside the worktree`
//! `vO7CW` and the cards beside it in 04c `vZcoN`, `WOOIC` in 04b `p6tXIO`): its head says what the agent asks
//! (`Head` `V65HM4`), and its reason line why it came (`Reason` `R332d`) — a path outside the worktree (with the
//! path), a request not on the agent's allowed list, the reviewer unsure or denying it (with its words) or unable to
//! run (why, with a link to its settings: [`super::could_not_run`]), a push to a remote, another ref (with the run's
//! branch).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::{CameToPerson, PendingToolApproval, PersonReason};
use orchester_workspace::display;

use super::super::Place;
use crate::{
  theme::{CardsText, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The head `{id}-head` of `request`'s card, which `came` to the person: a 14 `warning` glyph by the reason, what the
/// agent asks in 13/600 `text_primary` and, 8 after it, the step it works in and when it asked in 12 `text_muted`.
pub(super) fn head(
  ctx: &Ctx,
  id: &str,
  (request, step): (&PendingToolApproval, Option<&str>),
  came: &CameToPerson,
) -> Element {
  let time = display::clock(request.requested_at);
  let meta = match step {
    Some(step) => ctx.t_args("cards.approval.asks.meta", [("step", step), ("time", time.as_str())]),
    None => time.as_str().into(),
  };
  Row::new()
    .id(format!("{id}-head"))
    .describe("reason", came.reason.as_str())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph(came.reason), IconSize::Md, PaletteColor::Warning))
    .child(
      Text::new(&title(ctx, request, came.reason))
        .id(format!("{id}-title"))
        .variant(CardsText::ApprovalTitle.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&meta)
        .id(format!("{id}-meta"))
        .variant(CardsText::Quiet.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .into()
}

/// The head's glyph for `reason`: a locked folder outside the worktree, a crossed shield for what the allowed list or
/// the reviewer refused, a shield with a question while the reviewer was unsure, a shield with an alert when it could
/// not run, an upload for a push, a branch for another ref.
const fn glyph(reason: PersonReason) -> Icon {
  match reason {
    PersonReason::Outside => Icon::FolderLock,
    PersonReason::NotAllowed | PersonReason::ReviewerDenied => Icon::ShieldX,
    PersonReason::ReviewerUnsure => Icon::ShieldQuestion,
    PersonReason::ReviewerCouldNotRun => Icon::ShieldAlert,
    PersonReason::Push => Icon::Upload,
    PersonReason::Refs => Icon::GitBranch,
  }
}

/// What the agent asks, by why it came: *Builder asks to edit a file outside the worktree*.
fn title(ctx: &Ctx, request: &PendingToolApproval, reason: PersonReason) -> Arc<str> {
  let key = match (reason, request.command) {
    (PersonReason::Outside, true) => "cards.approval.asks.outside",
    (PersonReason::Outside, false) => "cards.approval.asks.outsideTool",
    (
      PersonReason::NotAllowed
      | PersonReason::ReviewerUnsure
      | PersonReason::ReviewerCouldNotRun
      | PersonReason::ReviewerDenied,
      true,
    ) => "cards.approval.asks.notAllowed",
    (
      PersonReason::NotAllowed
      | PersonReason::ReviewerUnsure
      | PersonReason::ReviewerCouldNotRun
      | PersonReason::ReviewerDenied,
      false,
    ) => "cards.approval.asks.notAllowedTool",
    (PersonReason::Push, _) => "cards.approval.asks.push",
    (PersonReason::Refs, _) => "cards.approval.asks.refs",
  };
  ctx.t_args(key, [("agent", request.agent.as_str())])
}

/// The reason line `{id}-came` of a request of `agent` that `came` to the person, at `place` ([`reason_line`]).
pub(super) fn line(ctx: &Ctx, id: &str, (place, agent): (&Place, &str), came: &CameToPerson) -> Element {
  reason_line(id, came.reason, &words(ctx, (place, agent), came), None)
}

/// The reason line `{id}-came` for `reason`: a 12 `warning` info glyph and, 8 from it, `words` in 12.5
/// `text_secondary`, then `link` where one opens the setting behind it.
pub(super) fn reason_line(id: &str, reason: PersonReason, words: &str, link: Option<Element>) -> Element {
  Row::new()
    .id(format!("{id}-came"))
    .describe("reason", reason.as_str())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::Warning))
    .child(
      Text::new(words)
        .id(format!("{id}-came-text"))
        .variant(ExtraTypography::Lead.role())
        .selectable(true)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(link)
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
    // Why it could not run is said by `could_not_run`, once the reviewer is read.
    (PersonReason::ReviewerCouldNotRun, _) => ctx.t("cards.approval.cameToYou.reviewerCouldNotRunSilent"),
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
