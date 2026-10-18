//! A merge of the base branch waits in the run's worktree for the person to resolve its conflicts (*Resolve in
//! editor*, [`super::resolve`]; board 4 of `fvCVT`, *Resolving in the editor*): the notice says where the worktree
//! is and whether the editor opened (`jI4tM`), *Conflicting files* says of each whether it still holds a conflict
//! marker, *Continue* commits the merge and waits, saying why, while a marker is left (`IerPj`, `Forys`), *Abort the
//! merge* puts the worktree back (`X1UbH`), and *Merge into main* waits until the merge is done (`k3oSNb`).

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
use orchester_core::integration::resolve::Resolution;
use orchester_workspace::tasks::{
  changes::run_worktree,
  detail::RunView,
  integration_jobs::{EditorOpened, JobState, ResolveStep},
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
  let worktree = run_worktree(&task.project.open.repo_root, run.id());
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
  vec![
    notice.into(),
    files(ctx, resolution),
    actions(ctx, task, run, resolution),
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

/// *Conflicting files* (`IBx7h`), each still conflicted while it holds a marker, else resolved.
fn files(ctx: &mut Ctx, resolution: &Resolution) -> Element {
  let rows: Vec<Element> = resolution
    .conflicted
    .iter()
    .map(|file| {
      let conflicted = resolution.markers.contains(file);
      let (glyph, color, state) = if conflicted {
        (
          Icon::GitCompareArrows,
          PaletteColor::Warning,
          "decisions.ready.resolving.conflicted",
        )
      } else {
        (
          Icon::CircleCheck,
          PaletteColor::Success,
          "decisions.ready.resolving.resolved",
        )
      };
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(FILE_ROW_HEIGHT))
        .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .describe("conflicted", if conflicted { "true" } else { "false" })
        .child(icon(glyph, IconSize::Base, color))
        .child(Text::new(file).variant(TypographyStyle::Mono).flex(1.0).nowrap())
        .child(Text::new(&ctx.t(state)).variant(TypographyStyle::Caption))
        .into()
    })
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
      Some(resolution.conflicted.len()),
    ))
    .child(list)
    .into()
}

/// *Continue*, which waits while a marker is left and says where, and *Abort the merge*.
fn actions(ctx: &mut Ctx, task: &TaskContext, run: &RunView, resolution: &Resolution) -> Element {
  let job = task.jobs.run(run.id());
  let services = Services::of(ctx);
  let jobs = services.jobs.of(task.project.open.project.id()).of(run.id());
  let args = resolve_args(ctx, task, run);
  let button = |id: &str, kind, label: Arc<str>, glyph, step: ResolveStep| {
    let button = ActionButton::new(id.to_owned(), kind, label).icon(glyph);
    if job.resolving() && job.resolve.step == Some(step) {
      return button.loading();
    }
    if job.busy() {
      return button.disabled();
    }
    let (jobs, args) = (jobs.clone(), args.clone());
    button.on_click(move || {
      jobs.start_resolving(step, args.clone());
    })
  };
  let markers =
    (!resolution.markers.is_empty()).then(|| message_text(ctx, &resolve::markers_left(&resolution.markers)));
  let resume = button(
    "task-resolve-continue",
    ButtonKind::Primary,
    ctx.t("decisions.ready.resolving.continue"),
    Icon::Check,
    ResolveStep::Continue,
  );
  let resume = match &markers {
    Some(reason) => resume.unavailable(reason.clone()),
    None => resume,
  };
  let abort = button(
    "task-resolve-abort",
    ButtonKind::Secondary,
    ctx.t("decisions.ready.resolving.abort"),
    Icon::CircleX,
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
    .with_children(markers.map(|reason| reason_line("task-resolve-continue-reason", &reason)))
    .into()
}
