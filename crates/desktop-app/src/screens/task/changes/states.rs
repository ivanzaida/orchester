//! What the Changes tab shows instead of files, and the notices over them, in the styles of a pull
//! request's detail states (`Pull request · detail states` `c07NAC`): a state card (`yUrNX`) 32 from the
//! tab's sides, for a run that changed nothing yet or more than Orchester shows, or whose changes could
//! not be read; a notice (`R3Pyx`) over the files for the worktree's part, while the checkpointed files
//! still show. Reading shows the loading line, never an empty list.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  command::CommandError,
  diff::{Changes, MAX_DIFF_FILES, MAX_DIFF_LINES, git::GitDiffError},
};
use orchester_workspace::{messages::Message, tasks::changes::ChangesError};

use super::grouped;
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  message::message_text,
  notice::{Notice, NoticeTone},
  state::{EmptyState, loading},
};

/// The run's changes are being read.
pub fn reading(ctx: &Ctx) -> Element {
  loading("changes-loading", &ctx.t("changes.loading")).into()
}

/// The task has not run: nothing changed yet.
pub fn no_run(ctx: &Ctx) -> Element {
  card(EmptyState::new(
    "changes-no-run",
    Icon::GitCompare,
    ctx.t("changes.noRun.title"),
    ctx.t("changes.noRun.detail"),
  ))
}

/// Run `run` has no checkpoint and its worktree holds nothing: nothing of its work is there yet.
pub fn no_checkpoint(ctx: &Ctx, run: u32) -> Element {
  card(EmptyState::new(
    "changes-no-checkpoint",
    Icon::GitCommitHorizontal,
    ctx.t("changes.noCheckpoint.title"),
    ctx.t_args("changes.noCheckpoint.detail", [("run", run.to_string())]),
  ))
}

/// The run's checkpoint changes nothing, and its worktree holds nothing beyond it.
pub fn no_changes(ctx: &Ctx) -> Element {
  card(EmptyState::new(
    "changes-none",
    Icon::GitCompare,
    ctx.t("changes.none.title"),
    ctx.t("changes.none.detail"),
  ))
}

/// The run's checkpointed changes are larger than Orchester reads: measured, not shown. Its branch has them.
pub fn too_large(ctx: &Ctx, size: Changes, branch: &str) -> Element {
  let mut args = size_args(ctx, size);
  args.push(("branch", branch.to_owned()));
  card(EmptyState::new(
    "changes-too-large",
    Icon::Files,
    ctx.t("changes.tooLarge.title"),
    ctx.t_args(
      plural(size, ("changes.tooLarge.detail.one", "changes.tooLarge.detail.other")),
      args,
    ),
  ))
}

/// The run's checkpointed changes could not be read, and why; Try again reads them again.
pub fn failed(ctx: &Ctx, error: &Message, retry: impl Fn() + Send + Sync + 'static) -> Element {
  let again = ActionButton::new("changes-try-again", ButtonKind::Secondary, ctx.t("common.tryAgain"))
    .icon(Icon::RotateCcw)
    .on_click(retry);
  card(
    EmptyState::new(
      "changes-failed",
      Icon::FileX,
      ctx.t("changes.failed.title"),
      message_text(ctx, error),
    )
    .action(again),
  )
}

/// What the worktree holds is larger than Orchester reads; the checkpointed files still show.
pub fn worktree_too_large(ctx: &Ctx, size: Changes) -> Element {
  inset(
    Notice::new(
      "changes-notice",
      NoticeTone::Warning,
      ctx.t("changes.worktreeTooLarge.title"),
    )
    .icon(Icon::Files)
    .body(ctx.t_args(
      plural(
        size,
        (
          "changes.worktreeTooLarge.detail.one",
          "changes.worktreeTooLarge.detail.other",
        ),
      ),
      size_args(ctx, size),
    )),
  )
}

/// The worktree could not be read; Try again reads it again. The checkpointed files stay, so it reads as
/// information with a `danger` icon (`PEBSm`); its words are the worktree's, unless git is missing.
pub fn worktree_failed(ctx: &Ctx, error: &Message, retry: impl Fn() + Send + Sync + 'static) -> Element {
  let again = ActionButton::new(
    "changes-worktree-try-again",
    ButtonKind::Secondary,
    ctx.t("common.tryAgain"),
  )
  .icon(Icon::RotateCcw)
  .on_click(retry);
  let git_missing = ChangesError::Git(GitDiffError::Run(CommandError::NotFound)).message();
  let body = if *error == git_missing {
    message_text(ctx, error)
  } else {
    ctx.t("changes.worktreeFailed.detail")
  };
  inset(
    Notice::new(
      "changes-notice",
      NoticeTone::Info,
      ctx.t("changes.worktreeFailed.title"),
    )
    .icon(Icon::CircleAlert)
    .mark(PaletteColor::Danger)
    .body(body)
    .action(again),
  )
}

/// The run ended and its worktree was removed: what it checkpointed is all there is.
pub fn worktree_removed(ctx: &Ctx) -> Element {
  inset(
    Notice::new(
      "changes-notice",
      NoticeTone::Info,
      ctx.t("changes.worktreeRemoved.title"),
    )
    .icon(Icon::FolderX)
    .body(ctx.t("changes.worktreeRemoved.detail")),
  )
}

/// `size` and the limits, as the locale groups numbers ("20,000").
fn size_args(ctx: &Ctx, size: Changes) -> Vec<(&'static str, String)> {
  vec![
    ("files", grouped(ctx, size.files)),
    ("additions", grouped(ctx, size.additions)),
    ("deletions", grouped(ctx, size.deletions)),
    ("maxFiles", grouped(ctx, MAX_DIFF_FILES)),
    ("maxLines", grouped(ctx, MAX_DIFF_LINES)),
  ]
}

/// Of the keys `(one, other)`, the one that words `size`'s count of files.
const fn plural(size: Changes, (one, other): (&'static str, &'static str)) -> &'static str {
  if size.files == 1 { one } else { other }
}

/// A state card 32 from the tab's sides and top.
fn card(state: EmptyState) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(SpacingSize::Section))
    .child(state)
    .into()
}

/// A notice over the files, 32 from the tab's sides and 12 from the toolbar and the files.
fn inset(notice: Notice) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Section, SpacingSize::Md))
    .child(notice)
    .into()
}
