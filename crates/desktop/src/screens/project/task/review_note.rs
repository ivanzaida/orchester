//! What auto mode did with a request, in words: *Approved by auto mode: …* with its reviewer's reason,
//! *Auto mode asked you: …* when the reviewer left it to the person, why a hard rule left it to the person
//! without a review, or why the review gave no verdict. The approval dialog, the conversation's card and
//! the activity's marks say it the same way.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Text,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::review::{Review, ReviewFailure, ReviewHold};

use crate::theme::ExtraTypography;

/// `review` in words.
pub(in crate::screens::project) fn review_note(ctx: &Ctx, review: &Review) -> Arc<str> {
  match review {
    Review::Approved { reason, .. } => ctx.t_args("runs.autoMode.note.approved", [("reason", reason.as_str())]),
    Review::Asked { reason, .. } => ctx.t_args("runs.autoMode.note.asked", [("reason", reason.as_str())]),
    Review::Held { hold } => ctx.t(match hold {
      ReviewHold::Secrets => "runs.autoMode.note.held.secrets",
      ReviewHold::ReadOnlyStep => "runs.autoMode.note.held.readOnlyStep",
      ReviewHold::NetworkOff => "runs.autoMode.note.held.networkOff",
      ReviewHold::OutsideFolders => "runs.autoMode.note.held.outsideFolders",
      ReviewHold::PermissionsUnknown => "runs.autoMode.note.held.permissionsUnknown",
    }),
    Review::Failed { failure, .. } => ctx.t(match failure {
      ReviewFailure::TimedOut => "runs.autoMode.note.failed.timedOut",
      ReviewFailure::Unreadable => "runs.autoMode.note.failed.unreadable",
      ReviewFailure::CouldNotRun => "runs.autoMode.note.failed.couldNotRun",
      ReviewFailure::NoReviewer => "runs.autoMode.note.failed.noReviewer",
    }),
  }
}

/// `review` as a line under a request, `id` for tooling.
pub(in crate::screens::project) fn review_line(ctx: &Ctx, id: String, review: &Review) -> Element {
  Text::new(&review_note(ctx, review))
    .id(id)
    .variant(ExtraTypography::CellLabel.role())
    .color(PaletteColor::TextPrimary)
    .width(Dimension::Pct(100.0))
    .into()
}
