//! Discarding the selected changes, confirmed in place of the *Prepare this checkout* card in its style: what
//! discarding does to each selected file and which of its versions go to the Recycle Bin / Trash, *Discard N files*
//! (danger) and *Keep them*; and once some files stayed, which were discarded, which not and why, with *Try again*,
//! which asks again for those. Discarding takes a path whole, so a path selected only in part is refused. Only a
//! person's press discards.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::invocation::Platform,
  integration::local::{Discard, LocalFile, LocalPath},
};
use orchester_exec::local_git::STAGED;
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

/// The key for Windows' words or the Mac's.
fn on_platform(windows: &'static str, mac: &'static str) -> &'static str {
  match Platform::current() {
    Platform::Windows => windows,
    Platform::MacOs => mac,
  }
}

fn confirmation(ctx: &Ctx, props: &Props) -> Element {
  let selection = props.selected.get();
  let paths = selection.paths();
  let partial = super::super::files::partly_selected(&props.view, &selection);
  let lines = paths.iter().flat_map(|path| {
    let file = props.view.checkout.files.iter().find(|file| file.path == *path);
    file.map(|file| plan(ctx, file)).unwrap_or_default()
  });
  let refusals = partial
    .iter()
    .map(|path| line(ctx, path, "local_integration.discardPartial", "partial"));
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
      paths.is_empty() || !partial.is_empty(),
    )
    .fill_width();
  action_panel(
    ctx,
    "local_integration.discardTitle",
    Some(on_platform(
      "local_integration.discardBodyWindows",
      "local_integration.discardBodyMac",
    )),
  )
  .child(files("local-discard-files", lines.chain(refusals)))
  .child(confirm)
  .child(keep(ctx, props))
  .child(guard_note(ctx, "local_integration.discardNote"))
  .into()
}

/// What discarding does to `file`, and the copy of its staged version when it keeps one.
fn plan(ctx: &Ctx, file: &LocalFile) -> Vec<Element> {
  let key = match file.discard() {
    Discard::Trash => on_platform(
      "local_integration.discardTrashWindows",
      "local_integration.discardTrashMac",
    ),
    Discard::RestoreKeepingCopy => on_platform(
      "local_integration.discardRestoreWindows",
      "local_integration.discardRestoreMac",
    ),
    Discard::Restore => "local_integration.discardRestoreDeleted",
    Discard::Unstage => "local_integration.discardUnstage",
  };
  let mut lines = vec![line(ctx, &file.path, key, "plan")];
  if file.keeps_staged_copy() {
    let name = file.path.as_str().rsplit('/').next().unwrap_or(file.path.as_str());
    let copy = format!("{name}{STAGED}");
    let key = on_platform(
      "local_integration.discardStagedWindows",
      "local_integration.discardStagedMac",
    );
    lines.push(
      Text::new(&ctx.t_args(key, [("path", file.path.as_str()), ("copy", copy.as_str())]))
        .id(format!("local-discard-staged-{}", file.path))
        .variant(LocalText::Explanation.role())
        .into(),
    );
  }
  lines
}

fn result(ctx: &Ctx, props: &Props, report: &DiscardReport) -> Element {
  let lines = report.files().iter().map(|file| outcome(ctx, file));
  let unread = (!report.reviewed_again()).then(|| {
    Text::new(&ctx.t("local_integration.discardedUnread"))
      .id("local-discarded-unread")
      .variant(LocalText::Explanation.role())
      .into()
  });
  // Only what is still a change of the checkout is asked about again; the selection holds those paths.
  let again = report
    .failed()
    .into_iter()
    .any(|path| props.view.checkout.files.iter().any(|file| file.path == path));
  let mut panel = action_panel(
    ctx,
    "local_integration.discardedTitle",
    Some("local_integration.discardedBody"),
  )
  .child(files("local-discarded-files", lines.chain(unread)));
  if again {
    panel = panel.child(
      mode_button(
        ctx,
        props,
        ("local-discard-again", ButtonKind::Danger),
        "common.tryAgain",
        Mode::Discard,
      )
      .fill_width(),
    );
  }
  panel
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

fn line(ctx: &Ctx, path: &LocalPath, key: &str, kind: &str) -> Element {
  Text::new(&ctx.t_args(key, [("path", path.as_str())]))
    .id(format!("local-discard-{kind}-{path}"))
    .variant(LocalText::Explanation.role())
    .into()
}

/// Whether `file` was discarded, or why not: the system's own words when the Recycle Bin / Trash refused it.
fn outcome(ctx: &Ctx, file: &DiscardedFile) -> Element {
  let Some(failure) = &file.failure else {
    return line(ctx, &file.path, "local_integration.discardedDone", "done");
  };
  let reason: Arc<str> = match failure {
    DiscardFailure::Trash(reason) => Arc::from(reason.as_str()),
    DiscardFailure::NoBin => ctx.t("local_integration.discardFailure.noBin"),
    DiscardFailure::TooLarge => ctx.t("local_integration.discardFailure.tooLarge"),
    DiscardFailure::NotKept => ctx.t("local_integration.discardFailure.notKept"),
    DiscardFailure::Uncertain => ctx.t("local_integration.discardFailure.uncertain"),
    DiscardFailure::Changed => ctx.t("local_integration.discardFailure.changed"),
    DiscardFailure::Unsupported => ctx.t("local_integration.discardFailure.unsupported"),
    DiscardFailure::Git => ctx.t("local_integration.discardFailure.git"),
    DiscardFailure::Io => ctx.t("local_integration.discardFailure.io"),
  };
  Text::new(&ctx.t_args(
    "local_integration.discardedFailed",
    [("path", file.path.as_str()), ("reason", &*reason)],
  ))
  .id(format!("local-discard-failed-{}", file.path))
  .variant(LocalText::Explanation.role())
  .into()
}
