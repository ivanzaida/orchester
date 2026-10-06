//! What auto mode did with a request, on a line of its own under the request: the auto mode glyph, a label
//! the app words (*Auto mode:*, *Auto mode asked you:*) and the reviewer's reason in quotation marks, muted
//! and whole (wrapped; it is stored as one plain line of at most 200 characters). The reason is the reviewer's own
//! text, so it never reads as the app's words or as a decision: it is never put into a mark or a chip. A hard rule that
//! left the request to the person, or a review that gave no verdict, is the app's own sentence. The approval
//! dialog, the conversation's card and the activity's rows show it the same way.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::review::{Review, ReviewFailure, ReviewHold};

use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// `review` in words: the app's label or sentence, and the reviewer's reason, quoted, when it gave one.
fn review_words(ctx: &Ctx, review: &Review) -> (Arc<str>, Option<Arc<str>>) {
  let quoted = |reason: &str| Some(ctx.t_args("runs.autoMode.note.quoted", [("reason", reason)]));
  match review {
    Review::Approved { reason, .. } => (ctx.t("runs.autoMode.note.approved"), quoted(reason)),
    Review::Asked { reason, .. } => (ctx.t("runs.autoMode.note.asked"), quoted(reason)),
    Review::Held { hold } => (ctx.t(held_key(*hold)), None),
    Review::Failed { failure, .. } => (
      ctx.t(match failure {
        ReviewFailure::TimedOut => "runs.autoMode.note.failed.timedOut",
        ReviewFailure::Unreadable => "runs.autoMode.note.failed.unreadable",
        ReviewFailure::CouldNotRun => "runs.autoMode.note.failed.couldNotRun",
        ReviewFailure::NoReviewer => "runs.autoMode.note.failed.noReviewer",
      }),
      None,
    ),
  }
}

const fn held_key(hold: ReviewHold) -> &'static str {
  match hold {
    ReviewHold::Secrets => "runs.autoMode.note.held.secrets",
    ReviewHold::Unreadable => "runs.autoMode.note.held.unreadable",
    ReviewHold::Destructive => "runs.autoMode.note.held.destructive",
    ReviewHold::Push => "runs.autoMode.note.held.push",
    ReviewHold::ReadOnlyStep => "runs.autoMode.note.held.readOnlyStep",
    ReviewHold::NetworkOff => "runs.autoMode.note.held.networkOff",
    ReviewHold::OutsideFolders => "runs.autoMode.note.held.outsideFolders",
    ReviewHold::PermissionsUnknown => "runs.autoMode.note.held.permissionsUnknown",
    ReviewHold::TooLong => "runs.autoMode.note.held.tooLong",
    ReviewHold::Escalation => "runs.autoMode.note.held.escalation",
    ReviewHold::ProjectProgram => "runs.autoMode.note.held.projectProgram",
  }
}

/// `review` as a line under a request, `id` for tooling: `{id}-label` and `{id}-reason` for its parts.
pub(in crate::screens::project) fn review_line(ctx: &Ctx, id: String, review: &Review) -> Element {
  let (label, reason) = review_words(ctx, review);
  let quoted = reason.is_some();
  let reason_id = format!("{id}-reason");
  let label = Text::new(&label)
    .id(format!("{id}-label"))
    .variant(ExtraTypography::CellLabel.role())
    .color(PaletteColor::TextSecondary);
  // A label before a quoted reason stays whole; a sentence of the app's own may wrap.
  let label = if quoted {
    label.nowrap().flex_shrink(0.0)
  } else {
    label.flex_shrink(1.0).min_width(Dimension::Px(0.0))
  };
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::Bot, IconSize::Small, PaletteColor::TextMuted).flex_shrink(0.0))
    .child(label)
    .with_children(reason.map(|reason| {
      Text::new(&reason)
        .id(reason_id)
        .variant(ExtraTypography::CellLabel.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .selectable(true)
    }))
    .into()
}
