//! *Not merged* (`Section · Not merged` `g7qwcU` in 09 `Os7b3`): a row per run whose last attempt did not land, in
//! `warning` but while Orchester tries again by itself, in `info` (`Cjprb`). Its action moves the run on: *Try again*
//! merges it through the task page's own job (`RunJobs::start_merge`), *Try now* ends the wait for Orchester's next
//! try, *Put files back* puts back what a stopped bring-in left changed; *Resolve* and *Record merge* open the run's
//! attempt page, where its conflicts are resolved and the merge Git made is recorded.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::integration::attempt::AttemptOutcome;
use orchester_workspace::tasks::{
  integration_jobs::{JobState, MergeArgs},
  merge_attempts::merges::{MergeRun, NotMergedRun},
};

use super::{
  actions::Actions,
  attempt_page, retrying,
  row::{self, Shown},
  words,
};
use crate::{
  project::ShownProject,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The section, or nothing without a run not merged.
pub(super) fn section(
  ctx: &mut Ctx,
  project: &ShownProject,
  actions: &Actions,
  runs: &[NotMergedRun],
) -> Option<lurq::node::Element> {
  if runs.is_empty() {
    return None;
  }
  let rows = runs
    .iter()
    .enumerate()
    .map(|(at, run)| {
      let shown = shown(ctx, project, actions, run);
      row::row(ctx, &run.run, shown, at + 1 == runs.len())
    })
    .collect();
  Some(row::section(
    "merges-not-merged",
    &ctx.t("merges.section.notMerged"),
    rows,
  ))
}

fn shown(ctx: &mut Ctx, project: &ShownProject, actions: &Actions, run: &NotMergedRun) -> Shown {
  let id = format!("merges-not-merged-{}", run.run.run);
  let (glyph, line) = match run.not_merged.retrying {
    Some(retrying) => (
      (Icon::RefreshCw, PaletteColor::Info),
      retrying::line(ctx, &id, run, retrying),
    ),
    None => {
      let glyph = match run.not_merged.attempt.attempt.outcome() {
        AttemptOutcome::Conflicts { .. } => Icon::GitMerge,
        AttemptOutcome::LeftChanged { .. } => Icon::TriangleAlert,
        _ => Icon::CircleAlert,
      };
      let line = row::line(&id, &words::outcome(ctx, run), PaletteColor::Warning);
      ((glyph, PaletteColor::Warning), line)
    }
  };
  let action = action(ctx, project, actions, (&id, run));
  Shown {
    glyph,
    line: (line, outcome_name(run)),
    meta: words::not_merged_meta(ctx, run),
    action: Some(action),
    opens: attempt_page(ctx, project, &run.run),
    id,
  }
}

/// What moves the run on, as its last attempt left it.
fn action(
  ctx: &mut Ctx,
  project: &ShownProject,
  actions: &Actions,
  (id, run): (&str, &NotMergedRun),
) -> lurq::node::Element {
  let button = |key: &str, glyph: Icon, ctx: &Ctx| {
    ActionButton::new(format!("{id}-action"), ButtonKind::Secondary, ctx.t(key))
      .icon(glyph)
      .small()
  };
  let run_id = run.run.run;
  if run.not_merged.retrying.is_some() {
    let actions = actions.clone();
    return button("merges.action.tryNow", Icon::RotateCcw, ctx)
      .on_click(move || actions.try_now(run_id))
      .into();
  }
  if !run.left_changed.is_empty() {
    let shown = button("merges.action.putFilesBack", Icon::Undo2, ctx);
    if actions.putting_back.get().0 == Some(run_id) {
      return shown.loading().into();
    }
    let actions = actions.clone();
    return shown.on_click(move || actions.put_files_back(run_id)).into();
  }
  match run.not_merged.attempt.attempt.outcome() {
    AttemptOutcome::Conflicts { .. } => button("merges.action.resolve", Icon::CodeXml, ctx)
      .opens(attempt_page(ctx, project, &run.run))
      .into(),
    AttemptOutcome::NotRecorded { .. } => button("merges.action.recordMerge", Icon::Database, ctx)
      .opens(attempt_page(ctx, project, &run.run))
      .into(),
    _ => try_again(
      ctx,
      project,
      button("merges.action.tryAgain", Icon::RotateCcw, ctx),
      &run.run,
    ),
  }
}

/// *Try again*: merges the run as the task page's *Merge into <base>* does, on the project's shared operations; under
/// way while it merges, waiting while another run of the project merges, unavailable without the branch it merges into.
pub(super) fn try_again(
  ctx: &mut Ctx,
  project: &ShownProject,
  button: ActionButton,
  run: &MergeRun,
) -> lurq::node::Element {
  let Some(branch) = run.base.clone() else {
    return button.unavailable(ctx.t("merges.action.noBranch")).into();
  };
  let jobs = project.feed.job_state.get();
  if jobs.run(run.run).merge == JobState::Running {
    return button.loading().into();
  }
  if jobs.merging().is_some_and(|other| other != run.run) {
    return button.unavailable(ctx.t("merges.action.anotherMerge")).into();
  }
  let services = Services::of(ctx);
  let feed = project.feed.clone();
  let args = MergeArgs {
    open: project.open.clone(),
    task: run.task,
    key: run.key.to_string(),
    number: run.number.get(),
    branch: branch.as_str().to_owned(),
    run: run.run,
    programs: services.programs,
    activity: services.activity,
    separator: ctx.t("task.listSeparator"),
    changed: Arc::new(move || feed.saved()),
  };
  let (job, vault) = (project.feed.jobs.of(run.run), services.vault);
  // A merge that does not begin (another one took the project meanwhile) leaves the job's state as it is.
  let start = move || {
    if lurq::app::events::is_synthetic_input() {
      job.start_synthetic_merge(args.clone(), vault.clone());
    } else {
      job.start_merge(args.clone());
    }
  };
  button.on_click(start).into()
}

/// The row's `outcome` description: how the run's last attempt ended, or that Orchester tries again.
fn outcome_name(run: &NotMergedRun) -> &'static str {
  if run.not_merged.retrying.is_some() {
    return "retrying";
  }
  run.not_merged.attempt.attempt.outcome().name()
}
