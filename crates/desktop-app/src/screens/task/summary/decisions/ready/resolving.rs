//! A merge of the base branch waits in the run's worktree for the person to resolve its conflicts (*Resolve in
//! editor*, [`super::resolve`]; board 4 of `fvCVT`, *Resolving in the editor*): the notice says where the worktree
//! is and whether the editor opened (`jI4tM`), *Conflicting files* says of each whether it still holds a conflict
//! marker, is resolved, or waits for the person to keep a side because git could not merge it by content (a binary
//! file, or one a side deleted; no frame has this yet), *Continue* commits the merge and waits, saying why, while a
//! file is not resolved (`IerPj`, `Forys`), *Abort the merge* puts the worktree back (`X1UbH`), and *Merge into main*
//! waits until the merge is done (`k3oSNb`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::resolve::{ConflictFile, FileState, Resolution, Side, Undecided};
use orchester_workspace::tasks::{
  changes::run_worktree,
  detail::RunView,
  integration_jobs::{EditorOpened, JobState, ResolveArgs, ResolveStep, RunJobState, RunJobs},
  resolve::{self, Editor},
};

use super::{
  super::{super::section_head, TaskContext},
  card::{FILE_ROW_HEIGHT, merge_actions},
  pull_request::PullRequestRead,
  resolve::resolve_args,
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// The merge waiting in `run`'s worktree with `resolution`'s files, and the actions to merge after it.
pub fn resolving(
  ctx: &mut Ctx,
  task: &TaskContext,
  run: &RunView,
  (resolution, pull): (&Resolution, &PullRequestRead),
) -> Vec<Element> {
  let worktree = resolve::native(&run_worktree(&task.project.open.repo_root, run.id()));
  let base = run.run.base_branch().map_or("", |branch| branch.as_str()).to_owned();
  let job = task.jobs.run(run.id()).resolve;
  let body = ctx.t_args(
    "decisions.ready.resolving.body",
    [("branch", base.clone()), ("path", worktree.display().to_string())],
  );
  let editor = editor_said(ctx, job.editor.as_ref());
  let notice = Notice::new(
    "task-resolving",
    NoticeTone::Warning,
    ctx.t_args("decisions.ready.resolving.title", [("branch", base)]),
  )
  .icon(Icon::GitCompareArrows)
  .body(ctx.t_args("decisions.ready.resolving.lines", [("body", body), ("editor", editor)]));
  let failure = match &job.state {
    JobState::Failed(message) => Some(field_error("task-resolving-failed", &message_text(ctx, message)).into()),
    _ => None,
  };
  let presses = Presses::new(ctx, task, run);
  vec![
    notice.into(),
    files(ctx, &presses, resolution),
    actions(ctx, &presses, resolution),
  ]
  .into_iter()
  .chain(failure)
  .chain(std::iter::once(merge_actions(ctx, task, run, (pull, true))))
  .collect()
}

/// What became of the editor when the merge stopped at its conflicts.
fn editor_said(ctx: &Ctx, editor: Option<&EditorOpened>) -> Arc<str> {
  match editor {
    Some(EditorOpened::Opened(Editor::Named(program))) => {
      let name = program.file_name().map_or_else(
        || program.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
      );
      ctx.t_args("decisions.ready.resolving.openedNamed", [("program", name)])
    }
    Some(EditorOpened::Opened(Editor::VsCode(_))) => ctx.t("decisions.ready.resolving.openedVsCode"),
    Some(EditorOpened::Opened(Editor::SystemFolders)) => ctx.t("decisions.ready.resolving.openedFolder"),
    Some(EditorOpened::PersonOnly) => ctx.t("decisions.ready.resolving.personOnly"),
    Some(EditorOpened::NoEditor) => ctx.t("decisions.ready.resolve.noEditor"),
    Some(EditorOpened::Failed(_)) => ctx.t("decisions.ready.resolving.editorFailed"),
    None => ctx.t("decisions.ready.resolving.openYourself"),
  }
}

/// Presses on the merge waiting in the run's worktree: each starts its resolving step, one at a time.
struct Presses {
  job: RunJobState,
  jobs: RunJobs,
  args: ResolveArgs,
  base: Arc<str>,
}

impl Presses {
  fn new(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> Self {
    let services = Services::of(ctx);
    Self {
      job: task.jobs.run(run.id()),
      jobs: services.jobs.of(task.project.open.project.id()).of(run.id()),
      args: resolve_args(ctx, task, run),
      base: run.run.base_branch().map_or("", |branch| branch.as_str()).into(),
    }
  }

  /// `button` starting `step`: under way while it is, waiting while the run's other work is.
  fn button(&self, button: ActionButton, step: ResolveStep) -> ActionButton {
    if self.job.resolving() && self.job.resolve.step.as_ref() == Some(&step) {
      return button.loading();
    }
    if self.job.busy() {
      return button.disabled();
    }
    let (jobs, args) = (self.jobs.clone(), self.args.clone());
    button.on_click(move || {
      jobs.start_resolving(step.clone(), args.clone());
    })
  }
}

/// *Conflicting files* (`IBx7h`): each conflicted while it holds a marker, waiting for a side to be kept when git could
/// not merge it by content, else resolved.
fn files(ctx: &mut Ctx, presses: &Presses, resolution: &Resolution) -> Element {
  let rows: Vec<Element> = resolution
    .files
    .iter()
    .enumerate()
    .map(|(index, file)| file_row(ctx, presses, (index, file)))
    .collect();
  let list = card("task-resolving-files", CardSurface::Outline)
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Hair.role()))
    .with_children(rows);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(section_head(
      ctx,
      "task-resolving-files-head",
      "decisions.ready.conflicts",
      Some(resolution.files.len()),
    ))
    .child(list)
    .into()
}

/// A conflicted file's row; one git could not merge by content offers to keep the run's side or the base branch's.
fn file_row(ctx: &Ctx, presses: &Presses, (index, file): (usize, &ConflictFile)) -> Element {
  let branch = &*presses.base;
  let (glyph, color, state, name) = match file.state {
    FileState::Markers => (
      Icon::GitCompareArrows,
      PaletteColor::Warning,
      ctx.t("decisions.ready.resolving.conflicted"),
      "conflicted",
    ),
    FileState::Undecided(why) => (
      Icon::CircleAlert,
      PaletteColor::Warning,
      undecided(ctx, why, branch),
      "undecided",
    ),
    FileState::Resolved => (
      Icon::CircleCheck,
      PaletteColor::Success,
      ctx.t("decisions.ready.resolving.resolved"),
      "resolved",
    ),
  };
  let keep = |id: &str, label: Arc<str>, side: Side| -> Element {
    let step = ResolveStep::Decide {
      path: Arc::from(file.path.as_str()),
      side,
    };
    let button = ActionButton::new(format!("{id}-{index}"), ButtonKind::Secondary, label).small();
    presses.button(button, step).into()
  };
  let decisions: Vec<Element> = if matches!(file.state, FileState::Undecided(_)) {
    vec![
      keep(
        "task-resolve-keep-run",
        ctx.t("decisions.ready.resolving.keepRun"),
        Side::Run,
      ),
      keep(
        "task-resolve-keep-base",
        ctx.t_args("decisions.ready.resolving.keepBase", [("branch", branch)]),
        Side::Base,
      ),
    ]
  } else {
    Vec::new()
  };
  Row::new()
    .id(format!("task-resolving-file-{index}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FILE_ROW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("state", name)
    .child(icon(glyph, IconSize::Base, color))
    .child(Text::new(&file.path).variant(TypographyStyle::Mono).flex(1.0).nowrap())
    .child(Text::new(&state).variant(TypographyStyle::Caption))
    .with_children(decisions)
    .into()
}

/// Why a file waits for a side to be kept.
fn undecided(ctx: &Ctx, why: Undecided, branch: &str) -> Arc<str> {
  match why {
    Undecided::Unmergeable => ctx.t("decisions.ready.resolving.unmergeable"),
    Undecided::DeletedByBase => ctx.t_args("decisions.ready.resolving.deletedByBase", [("branch", branch)]),
    Undecided::DeletedByRun => ctx.t_args("decisions.ready.resolving.deletedByRun", [("branch", branch)]),
  }
}

/// *Continue*, which waits while a marker is left or a side is to be kept and says where, and *Abort the merge*.
fn actions(ctx: &Ctx, presses: &Presses, resolution: &Resolution) -> Element {
  let markers = resolution.with_markers();
  let undecided = resolution.undecided();
  let reason = if !markers.is_empty() {
    Some(message_text(ctx, &resolve::markers_left(&markers)))
  } else if !undecided.is_empty() {
    Some(message_text(ctx, &resolve::undecided(&undecided)))
  } else {
    None
  };
  let resume = presses.button(
    ActionButton::new(
      "task-resolve-continue",
      ButtonKind::Primary,
      ctx.t("decisions.ready.resolving.continue"),
    )
    .icon(Icon::Check),
    ResolveStep::Continue,
  );
  let resume = match &reason {
    Some(reason) => resume.unavailable(reason.clone()),
    None => resume,
  };
  let abort = presses.button(
    ActionButton::new(
      "task-resolve-abort",
      ButtonKind::Secondary,
      ctx.t("decisions.ready.resolving.abort"),
    )
    .icon(Icon::CircleX),
    ResolveStep::Abort,
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Row::new()
        .id("task-resolving-actions")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(resume)
        .child(abort),
    )
    .with_children(reason.map(|reason| reason_line("task-resolve-continue-reason", &reason)))
    .into()
}
