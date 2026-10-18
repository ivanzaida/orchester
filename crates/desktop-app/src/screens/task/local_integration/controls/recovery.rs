//! Under the step: the recovery record once an operation holds something, and in the commit review (`Recovery
//! record` `xBGxu` in `kexy4`) — what it keeps and, each under its label in Geist Mono, its operation, how the
//! checkout was prepared, its merge and its restoration. Before that the review says that waiting holds no queue
//! (`Queue semantics` `qX5uF` in `YxF6B`; not at 960, `s4gj9`), and the preservation review shows neither (`UgHeC`).
//! The design's *View recovery journal* is not offered: Orchester has no journal viewer yet.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::Element,
};
use orchester_core::integration::local::{HookChange, LocalOperation, LocalPhase};
use orchester_workspace::{display, tasks::local_integration::IntegrationView};

use super::{Mode, phase_title};
use crate::{
  screens::task::local_integration::notes::panel,
  theme::LocalText,
  ui::card::{CardSurface, card_frame},
};

pub(super) fn card(ctx: &Ctx, view: &IntegrationView, mode: &Mode, narrow: bool) -> Option<Element> {
  if *mode == Mode::Preserve {
    return None;
  }
  let reviewing_commit = matches!(mode, Mode::Commit { .. });
  let recorded = view
    .operation
    .as_ref()
    .filter(|operation| reviewing_commit || operation.phase() != LocalPhase::Review || operation.reserves_checkout());
  let Some(operation) = recorded else {
    return (*mode == Mode::Review && !narrow).then(|| {
      panel(
        "local-queue",
        CardSurface::Outline,
        &ctx.t("local_integration.queue"),
        &ctx.t("local_integration.queueBody"),
      )
      .into()
    });
  };
  Some(rows(ctx, operation, reviewing_commit))
}

/// The record's rows; `reviewing_commit` while the person reviews the commit that would prepare the checkout.
fn rows(ctx: &Ctx, operation: &LocalOperation, reviewing_commit: bool) -> Element {
  let rows = [
    ("local_integration.operation", operation.run().to_string()),
    ("local_integration.preparation", preparation(ctx, operation)),
    ("local_integration.mergeRecord", merge(ctx, operation)),
    (
      "local_integration.restoration",
      ctx.t(restoration(operation, reviewing_commit)).to_string(),
    ),
  ];
  record(ctx)
    .with_children(rows.into_iter().map(|(label, value)| {
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(Text::new(&ctx.t(label)).variant(LocalText::Note.role()))
        .child(Text::new(&value).variant(LocalText::Value.role()))
    }))
    .into()
}

/// The run's merge: Orchester's, or the commit that held its work when it was found merged without Orchester.
fn merge(ctx: &Ctx, operation: &LocalOperation) -> String {
  match (operation.merged(), operation.merged_outside()) {
    (Some(merged), _) => display::short_revision(merged).to_owned(),
    (None, Some(outside)) => ctx
      .t_args(
        "local_integration.mergedOutside",
        [("commit", display::short_revision(outside))],
      )
      .to_string(),
    (None, None) => ctx.t("local_integration.notRecorded").to_string(),
  }
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
    .t(
      match (operation.phase(), operation.commit_tree(), operation.hook_change()) {
        (LocalPhase::Committing, ..) => "local_integration.committing",
        (_, Some(_), Some(HookChange::Stopped { .. })) => "local_integration.commitStopped",
        (_, Some(_), Some(HookChange::MovedHead { .. })) => "local_integration.committedMoved",
        (_, Some(_), None) => "local_integration.committed",
        (LocalPhase::Review, None, _) => "local_integration.noCommit",
        (_, None, _) => "local_integration.nothingPrepared",
      },
    )
    .to_string()
}

fn restoration(operation: &LocalOperation, reviewing_commit: bool) -> &'static str {
  if operation.preservation().is_none() {
    let committed = reviewing_commit
      || operation.phase() == LocalPhase::Committing
      || (operation.commit_tree().is_some() && !matches!(operation.hook_change(), Some(HookChange::Stopped { .. })));
    return if committed {
      "local_integration.notRequired"
    } else {
      "local_integration.nothingPreserved"
    };
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
