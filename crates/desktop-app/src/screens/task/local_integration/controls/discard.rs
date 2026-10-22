//! Discarding the selected changes, confirmed in place of the *Prepare this checkout* card in its style: what
//! discarding does to each selected file, *Discard N files* (danger) and *Keep them*; and once some files stayed,
//! which were discarded, which not and the system's reason, with *Try again*. Only a person's press discards.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::invocation::Platform,
  integration::local::{Discard, LocalPath},
};
use orchester_workspace::tasks::local_integration::{DiscardFailure, DiscardReport, DiscardedFile};

use super::{Action, Inspector, Mode, Props, action_panel, guard_note, mode_button};
use crate::{theme::LocalText, ui::button::ButtonKind};

impl Inspector {
  /// The discarding card while the person confirms it or looks at what it left; `None` in any other mode.
  pub(super) fn discarding(&self, ctx: &Ctx, props: &Props, mode: &Mode) -> Option<Element> {
    match mode {
      Mode::Discard => Some(confirmation(ctx, props)),
      Mode::Discarded(report) => Some(result(ctx, props, report)),
      _ => None,
    }
  }
}

fn confirmation(ctx: &Ctx, props: &Props) -> Element {
  let paths = props.selected.get().paths();
  let lines = paths.iter().filter_map(|path| {
    let file = props.view.checkout.files.iter().find(|file| file.path == *path)?;
    Some(line(ctx, path, plan(file.discard())))
  });
  let count = paths.len();
  let label = if count == 1 {
    ctx.t("local_integration.discardConfirm.one")
  } else {
    ctx.t_args(
      "local_integration.discardConfirm.other",
      [("count", count.to_string().as_str())],
    )
  };
  let confirm = props
    .trigger
    .labeled(
      ("local-discard-confirm", ButtonKind::Danger, label),
      Some(Action::Discard(paths.clone())),
      paths.is_empty(),
    )
    .fill_width();
  action_panel(
    ctx,
    "local_integration.discardTitle",
    Some("local_integration.discardBody"),
  )
  .child(files("local-discard-files", lines))
  .child(confirm)
  .child(keep(ctx, props))
  .child(guard_note(ctx, "local_integration.discardNote"))
  .into()
}

fn result(ctx: &Ctx, props: &Props, report: &DiscardReport) -> Element {
  let lines = report.files().iter().map(|file| outcome(ctx, file));
  // Only what is still a change of the checkout is tried again.
  let again = report
    .failed()
    .into_iter()
    .filter(|path| props.view.checkout.files.iter().any(|file| file.path == *path))
    .collect::<Vec<_>>();
  let retry = props
    .trigger
    .button(
      ctx,
      ("local-discard-again", ButtonKind::Danger, "common.tryAgain"),
      Some(Action::Discard(again.clone())),
      again.is_empty(),
    )
    .fill_width();
  action_panel(
    ctx,
    "local_integration.discardedTitle",
    Some("local_integration.discardedBody"),
  )
  .child(files("local-discarded-files", lines))
  .child(retry)
  .child(keep(ctx, props))
  .child(guard_note(ctx, "local_integration.discardNote"))
  .into()
}

/// The way back to the review, the selected files as they are.
fn keep(ctx: &Ctx, props: &Props) -> Element {
  mode_button(
    ctx,
    props,
    ("local-discard-keep", ButtonKind::Ghost),
    "local_integration.discardKeep",
    Mode::Review,
  )
  .into()
}

fn files(id: &str, lines: impl Iterator<Item = Element>) -> Column {
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(lines)
}

/// What discarding does, as the confirmation says it of one file.
fn plan(discard: Discard) -> &'static str {
  let windows = matches!(Platform::current(), Platform::Windows);
  match discard {
    Discard::Trash if windows => "local_integration.discardTrashWindows",
    Discard::Trash => "local_integration.discardTrashMac",
    Discard::RestoreKeepingCopy if windows => "local_integration.discardRestoreWindows",
    Discard::RestoreKeepingCopy => "local_integration.discardRestoreMac",
    Discard::Restore => "local_integration.discardRestoreDeleted",
    Discard::Unstage => "local_integration.discardUnstage",
  }
}

fn line(ctx: &Ctx, path: &LocalPath, key: &str) -> Element {
  Text::new(&ctx.t_args(key, [("path", path.as_str())]))
    .id(format!("local-discard-file-{path}"))
    .variant(LocalText::Explanation.role())
    .into()
}

/// Whether `file` was discarded, or why not: the system's own words when the Recycle Bin / Trash refused it.
fn outcome(ctx: &Ctx, file: &DiscardedFile) -> Element {
  let Some(failure) = &file.failure else {
    return line(ctx, &file.path, "local_integration.discardedDone");
  };
  let reason: Arc<str> = match failure {
    DiscardFailure::Trash(reason) => Arc::from(reason.as_str()),
    DiscardFailure::Changed => ctx.t("local_integration.discardFailure.changed"),
    DiscardFailure::Unsupported => ctx.t("local_integration.discardFailure.unsupported"),
    DiscardFailure::Git => ctx.t("local_integration.discardFailure.git"),
    DiscardFailure::Io => ctx.t("local_integration.discardFailure.io"),
  };
  Text::new(&ctx.t_args(
    "local_integration.discardedFailed",
    [("path", file.path.as_str()), ("reason", &*reason)],
  ))
  .id(format!("local-discard-file-{}", file.path))
  .variant(LocalText::Explanation.role())
  .into()
}
