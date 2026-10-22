//! The evidence side of the page, 16 between its parts (`Evidence and working area` `W6q37U`): what the page is
//! about now, then the review's snapshots with the shown one's diff and where ignored files stand; a commit's
//! selected snapshots, its diff and what it leaves (`kexy4`); a preservation's selection and that it discards
//! nothing (`UgHeC`); the checked result's diff; or the resolver while conflicts wait.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::local::{HookChange, LocalPath, LocalPhase};
use orchester_workspace::{
  display,
  tasks::local_integration::{IntegrationView, Preview, Snapshot},
};

use super::{
  LocalPage, Mode, TaskContext,
  diff::{self, Source},
  files::{self, List},
  notes, resolver, words,
  work::Refusal,
};
use crate::ui::{
  card::CardSurface,
  notice::{Notice, NoticeTone},
  state::loading,
};

pub(super) fn column(
  ctx: &mut Ctx,
  page: &LocalPage,
  task: &TaskContext,
  view: &IntegrationView,
  busy: bool,
  narrow: bool,
) -> Column {
  let phase = view.operation.as_ref().map_or(LocalPhase::Review, |op| op.phase());
  let mode = page.mode.get();
  let outside = view.operation.as_ref().is_some_and(|op| op.merged_outside().is_some());
  // Work found integrated outside Orchester: its card stands for what the page is about (`itt7z`, `pdT4W`, `UWLNj`).
  let phase_note = (!view.found_outside()).then(|| notes::phase(ctx, (phase, outside), &mode, narrow));
  let mut column = Column::new()
    .id("local-evidence")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Lg)
    .with_children(phase_note)
    .with_children(external_notice(ctx, task, view))
    .with_children(restore_next(ctx, view))
    .with_children(hook_notice(ctx, view, phase));
  if let Some(refused) = page.work.state().get().error {
    column = column.child(refusal(ctx, view, &refused));
  }
  if busy {
    let stopping = page.stop.load(std::sync::atomic::Ordering::Acquire);
    column = column.child(loading(
      "local-working",
      &ctx.t(if stopping {
        "local_integration.stopping"
      } else {
        "local_integration.busy"
      }),
    ));
  }
  if matches!(phase, LocalPhase::TaskConflicts | LocalPhase::RestoreConflicts) || page.editor_dirty.get() {
    return column.child(resolver::mount(ctx, page, task, view, busy, narrow));
  }
  if view.found_outside() {
    return column;
  }
  column.with_children(content(ctx, page, view, (busy, narrow)))
}

/// Why the page's work was refused. A refusal as stale names the paths a hashed check found changed since the
/// review, which the list may not show when Git's stat cache hides an edit, in the panel of a change made after the
/// review (`External change notice` `z56AZ` in `i6Cx4`).
fn refusal(ctx: &Ctx, view: &IntegrationView, refused: &Refusal) -> Element {
  if refused.changed.is_empty() {
    return Notice::new("local-work-error", NoticeTone::Danger, ctx.t(refused.problem.0)).into();
  }
  let paths = refused
    .changed
    .iter()
    .map(LocalPath::as_str)
    .collect::<Vec<_>>()
    .join(&ctx.t("local_integration.separator"));
  notes::panel(
    "local-work-error",
    CardSurface::Pending,
    &ctx.t_args(
      "local_integration.staleTitle",
      [("branch", view.checkout.branch.as_str())],
    ),
    &ctx.t_args("local_integration.staleBody", [("paths", paths.as_str())]),
  )
  .into()
}

/// Beside work found whole in the branch, where the record can take that merge and keeps preserved originals: that
/// restoring them comes next (`Restore step · next` in `pdT4W`).
fn restore_next(ctx: &Ctx, view: &IntegrationView) -> Option<Element> {
  let operation = view.operation.as_ref()?;
  // A cancellation put its originals back already: removing the kept ones follows, not a restoration.
  let retained =
    operation.phase() != LocalPhase::Canceled && operation.preservation().is_some_and(|saved| saved.retained);
  (view.outside_merge_provable() && operation.can_record_merged_outside() && retained).then(|| {
    notes::panel(
      "local-restore-next",
      CardSurface::Outline,
      &ctx.t("local_integration.restoreNext"),
      &ctx.t("local_integration.restoreBody"),
    )
    .into()
  })
}

fn content(ctx: &mut Ctx, page: &LocalPage, view: &IntegrationView, (busy, narrow): (bool, bool)) -> Vec<Element> {
  let signals = (&page.selected, &page.shown_file);
  match page.mode.get() {
    Mode::Commit { preview, .. } => commit_review(ctx, page, view, &preview, busy),
    Mode::Preserve => {
      let title = ctx.t("local_integration.preservationSelection");
      let kept = files::kept_whole(ctx, view, &page.selected.get());
      let mut parts = vec![files::list(ctx, (view, List::Preserve), signals, &title, busy)];
      parts.extend(notes::kept_whole(ctx, &kept));
      parts.push(notes::no_discard(ctx));
      parts
    }
    _ if view.operation.as_ref().is_some_and(|op| op.checked().is_some()) => {
      diff::cards(ctx, "local-checked-diff", &view.diff, Source::Diff, None)
    }
    mode => {
      let locked = busy || mode != Mode::Review || view.found_outside();
      let title = ctx.t("local_integration.files");
      let mut parts = vec![files::list(ctx, (view, List::Review), signals, &title, locked)];
      if let Some(shown) = page.shown_file.get().0 {
        let source = match shown.snapshot {
          Snapshot::Untracked => Source::NewFile(shown.path.as_str()),
          Snapshot::Staged | Snapshot::Unstaged => Source::Diff,
        };
        parts.extend(diff::cards(
          ctx,
          "local-file-diff",
          &view.diff,
          source,
          Some(shown.path.as_str()),
        ));
      }
      parts.push(notes::ignored(ctx, narrow));
      parts
    }
  }
}

/// The commit review (`kexy4`): the selected snapshots, the diff of the one shown (the first until another is
/// pressed), and what the commit leaves.
fn commit_review(
  ctx: &mut Ctx,
  page: &LocalPage,
  view: &IntegrationView,
  preview: &Preview,
  busy: bool,
) -> Vec<Element> {
  let selection = page.selected.get();
  let listed = files::entries(view, &selection, List::Commit);
  let count = listed.len().to_string();
  let title = ctx.t_args("local_integration.commitSelection", [("count", count.as_str())]);
  let mut parts = vec![files::list(
    ctx,
    (view, List::Commit),
    (&page.selected, &page.shown_file),
    &title,
    busy,
  )];
  let shown = page
    .shown_file
    .get()
    .0
    .map(|shown| shown.path)
    .filter(|path| listed.iter().any(|entry| entry.file.path == *path))
    .or_else(|| listed.first().map(|entry| entry.file.path.clone()));
  let source = shown.as_ref().map_or(Source::Diff, |path| Source::File(path.as_str()));
  parts.extend(diff::cards(ctx, "local-selected-diff", preview, source, None));
  parts.push(notes::remaining(ctx, &files::excluded(ctx, view, &selection)));
  parts
}

/// An integration made outside Orchester: merged without Orchester into the run's delivery target, as the Summary
/// says and names it, or what needs reconciliation.
fn external_notice(ctx: &Ctx, task: &TaskContext, view: &IntegrationView) -> Option<Element> {
  if let Some(into) = view.merged_outside() {
    let branch = task
      .detail
      .run
      .as_ref()
      .map(|run| run.run.branch().as_str())
      .unwrap_or_default();
    let (title, body) = words::merged_outside_words(ctx, branch, into);
    return Some(
      Notice::new("local-external-integration", NoticeTone::Success, title)
        .body(body)
        .into(),
    );
  }
  let evidence = view.external.as_ref()?;
  Some(
    Notice::new(
      "local-external-integration",
      NoticeTone::Warning,
      ctx.t("local_integration.externalTitle"),
    )
    .body(ctx.t_args(
      words::evidence_body(evidence.kind),
      [
        ("source", evidence.source.as_str()),
        ("target", evidence.target.as_str()),
      ],
    ))
    .into(),
  )
}

/// What the repository's commit hooks did that the person did not review: changed the last commit, which stopped
/// before the branch moved, or moved HEAD after it landed.
fn hook_notice(ctx: &Ctx, view: &IntegrationView, phase: LocalPhase) -> Option<Element> {
  let change = view.operation.as_ref()?.hook_change()?;
  if !matches!(phase, LocalPhase::Review | LocalPhase::Prepared) {
    return None;
  }
  let (title, body) = match change {
    HookChange::Stopped { paths, message } => {
      let paths = paths
        .iter()
        .map(|path| path.as_str())
        .collect::<Vec<_>>()
        .join(&ctx.t("local_integration.separator"));
      let body = match (paths.is_empty(), message) {
        (false, true) => ctx.t_args("local_integration.hookBoth", [("paths", paths.as_str())]),
        (false, false) => ctx.t_args("local_integration.hookPaths", [("paths", paths.as_str())]),
        (true, _) => ctx.t("local_integration.hookMessage"),
      };
      ("local_integration.hookTitle", body)
    }
    HookChange::MovedHead { commit, head } => (
      "local_integration.hookMovedTitle",
      ctx.t_args(
        "local_integration.hookMoved",
        [
          ("commit", display::short_revision(commit)),
          ("head", display::short_revision(head)),
        ],
      ),
    ),
  };
  Some(
    Notice::new("local-hook-change", NoticeTone::Warning, ctx.t(title))
      .body(body)
      .into(),
  )
}
