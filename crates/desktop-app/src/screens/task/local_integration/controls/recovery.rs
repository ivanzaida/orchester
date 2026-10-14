//! Under the step: the recovery record once an operation is recorded (`Recovery record` `xBGxu`) — what it keeps
//! and, each under its label, its operation, its state, how the checkout was prepared, its merge and its
//! restoration — and before that, that waiting holds no queue (`Queue semantics` `qX5uF`).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::Element,
};
use orchester_core::integration::local::{LocalOperation, LocalPhase};
use orchester_workspace::{display, tasks::local_integration::IntegrationView};

use super::phase_title;
use crate::{
  screens::task::local_integration::notes::panel,
  theme::LocalText,
  ui::card::{CardSurface, card_frame},
};

pub(super) fn card(ctx: &Ctx, view: &IntegrationView) -> Element {
  let Some(operation) = &view.operation else {
    return panel(
      "local-queue",
      CardSurface::Outline,
      &ctx.t("local_integration.queue"),
      &ctx.t("local_integration.queueBody"),
    )
    .into();
  };
  let rows = [
    ("local_integration.operation", operation.run().to_string()),
    (
      "local_integration.state",
      ctx.t(phase_title(operation.phase())).to_string(),
    ),
    ("local_integration.preparation", preparation(ctx, operation)),
    (
      "local_integration.mergeRecord",
      operation.merged().map_or_else(
        || ctx.t("local_integration.notRecorded").to_string(),
        |merged| display::short_revision(merged).to_owned(),
      ),
    ),
    (
      "local_integration.restoration",
      ctx.t(restoration(operation)).to_string(),
    ),
  ];
  record(ctx)
    .with_children(rows.into_iter().map(|(label, value)| {
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(Text::new(&ctx.t(label)).variant(LocalText::Note.role()))
        .child(Text::new(&value).variant(LocalText::Explanation.role()))
    }))
    .into()
}

fn record(ctx: &Ctx) -> Column {
  card_frame("local-recovery", CardSurface::Outline)
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("local_integration.recovery")).variant(LocalText::Heading.role()))
    .child(Text::new(&ctx.t("local_integration.recoveryKept")).variant(LocalText::Explanation.role()))
}

/// How the checkout was prepared: its named preservation and whether its originals are kept, or its commit.
fn preparation(ctx: &Ctx, operation: &LocalOperation) -> String {
  if let Some(saved) = operation.preservation() {
    let state = if !saved.retained {
      "local_integration.removed"
    } else if saved.verified {
      "local_integration.retained"
    } else {
      "local_integration.pending"
    };
    return ctx
      .t_args(
        "local_integration.preserved",
        [("name", saved.name.as_str()), ("state", &*ctx.t(state))],
      )
      .to_string();
  }
  ctx
    .t(match (operation.phase(), operation.commit_tree()) {
      (LocalPhase::Committing, _) => "local_integration.committing",
      (_, Some(_)) => "local_integration.committed",
      (_, None) => "local_integration.noCommit",
    })
    .to_string()
}

fn restoration(operation: &LocalOperation) -> &'static str {
  if operation.preservation().is_none() {
    return "local_integration.notRequired";
  }
  match operation.phase() {
    LocalPhase::RestorationVerified | LocalPhase::RemovingPreservation | LocalPhase::Complete => {
      "local_integration.verified"
    }
    LocalPhase::Restoring | LocalPhase::RestoreConflicts => "local_integration.pending",
    _ => "local_integration.notStarted",
  }
}

/// While fresh checkout evidence is read after an owned abort: the record says the operation paused.
pub(in crate::screens::task::local_integration) fn aborted(ctx: &Ctx) -> Element {
  record(ctx)
    .child(Text::new(&ctx.t(phase_title(LocalPhase::Paused))).variant(LocalText::Explanation.role()))
    .into()
}
