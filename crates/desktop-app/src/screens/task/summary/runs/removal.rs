//! *Remove worktree*'s confirmation, in place in the run's row rather than a dialog: what removing does
//! (and, for an interrupted run, that it can't continue afterwards), each worktree with its uncommitted
//! files, each branch with whether it is deleted (its commits are merged) or kept, the switch that agrees
//! to discard the uncommitted changes when there are any, that another run of the task at work keeps
//! its worktree, git's refusal, and Cancel and Remove. Git removes exactly the worktrees and changes the
//! confirmation listed, on a thread of the project's operations (`ProjectJobs`), so it goes on when the
//! page closes; one removal of the task at a time.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::RunId,
  integration::{LeftBranch, LeftWorktree, Leftovers, WorktreeOwner},
  run::{Run, RunNumber, RunStatus},
};
use orchester_workspace::{
  messages::Message,
  tasks::{
    integration::{file_list, unread_entry},
    integration_jobs::{JobState, RemoveArgs, Remover, RunJobs},
  },
};

use super::Runs;
use crate::{
  project::ShownProject,
  screens::task::{PageState, Removal},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    icon::{Icon, icon},
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// The confirmation of `run`'s removal, while it has worktrees and no live work.
pub fn confirm_for(ctx: &mut Ctx, runs: &Runs, run: RunId) -> Option<Element> {
  let detail = runs.detail;
  let record = detail.history.iter().find(|record| record.id == run)?;
  let ended = record.without_live_work()?;
  let leftovers = runs.left.of(run)?.ok().filter(|left| !left.worktrees.is_empty())?;
  let state = runs.jobs.run(run);
  let failed = match &state.remove {
    JobState::Failed(error) => Some(error.clone()),
    _ => None,
  };
  let working = detail
    .history
    .iter()
    .filter_map(|record| record.run.as_ref())
    .find(|run| run.status().has_live_work())
    .map(|run| run.number());
  let job = runs.project.feed.jobs.of(run);
  let discard = PageState::of(ctx).discard;
  let key = detail.task.key().to_string();
  Some(confirmation(
    ctx,
    &Confirm {
      project: runs.project,
      key: &key,
      run: ended,
      leftovers: &leftovers,
      removing: state.removing(),
      failed: failed.as_ref(),
      waits: detail
        .history
        .iter()
        .any(|other| other.id != run && runs.jobs.run(other.id).busy()),
      working,
      discard,
      job,
    },
  ))
}

/// What the confirmation is drawn from.
struct Confirm<'a> {
  project: &'a ShownProject,
  key: &'a str,
  run: &'a Arc<Run>,
  leftovers: &'a Leftovers,
  /// Git removes the run's worktrees now, whoever asked.
  removing: bool,
  /// Why git did not remove them the last time it was asked.
  failed: Option<&'a Message>,
  /// Another run of the task is being removed or merged: this one waits.
  waits: bool,
  /// Another run of the task at work, whose worktree is not touched.
  working: Option<RunNumber>,
  discard: Signal<bool>,
  job: RunJobs,
}

fn confirmation(ctx: &mut Ctx, confirm: &Confirm) -> Element {
  let separator = ctx.t("task.listSeparator");
  let number = confirm.run.number().get().to_string();
  let several = confirm.leftovers.worktrees.len() > 1;
  let explanation = if several {
    "summary.runs.remove.explanationOther"
  } else {
    "summary.runs.remove.explanationOne"
  };
  card(&format!("run-remove-confirm-{number}"), CardSurface::Decision)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ctx.t_args(explanation, [("run", number.as_str())]))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(interrupted(ctx, confirm.run))
    .with_children(
      confirm
        .leftovers
        .worktrees
        .iter()
        .map(|worktree| worktree_line(ctx, confirm.run, worktree, &separator)),
    )
    .with_children(confirm.leftovers.branches.iter().map(|branch| branch_line(ctx, branch)))
    .with_children(discard(ctx, confirm))
    .with_children(confirm.working.map(|working| {
      note(
        Icon::Info,
        &ctx.t_args("summary.runs.remove.otherWorking", [("run", working.get().to_string())]),
      )
    }))
    .with_children(confirm.failed.map(|failed| {
      Text::new(&message_text(ctx, failed))
        .id(format!("run-remove-error-{number}"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Danger)
        .width(Dimension::Pct(100.0))
    }))
    .child(actions(ctx, confirm))
    .into()
}

/// For an interrupted run: that Continue run takes it up in this worktree, so without it the run stays
/// interrupted for good.
fn interrupted(ctx: &Ctx, run: &Run) -> Option<Element> {
  if !matches!(run.status(), RunStatus::Interrupted(_)) {
    return None;
  }
  let number = run.number().get().to_string();
  Some(note(
    Icon::CirclePause,
    &ctx.t_args("summary.runs.remove.interrupted", [("run", number.as_str())]),
  ))
}

fn note(glyph: Icon, text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, crate::theme::IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(text).variant(ExtraTypography::Note.role()).flex(1.0))
    .into()
}

/// "Run #3 · 2 files not committed" or a step's own worktree, its uncommitted files under it.
fn worktree_line(ctx: &Ctx, run: &Run, worktree: &LeftWorktree, separator: &str) -> Element {
  let owner = match worktree.owner {
    WorktreeOwner::Run => ctx.t_args(
      "summary.runs.remove.runOwner",
      [("run", run.number().get().to_string())],
    ),
    WorktreeOwner::Step(execution) => {
      let step = run
        .execution(execution)
        .and_then(|execution| run.snapshot().body().step(execution.step()))
        .map_or_else(String::new, |step| step.name.as_str().to_owned());
      ctx.t_args("summary.runs.remove.stepOwner", [("step", step)])
    }
  };
  let files = worktree.uncommitted.len();
  let (glyph, key) = match (worktree.missing || worktree.forgotten, files) {
    (true, _) => (Icon::FolderX, "summary.runs.remove.gone"),
    (false, 0) => (Icon::Check, "summary.runs.remove.clean"),
    (false, 1) => (Icon::FileDiff, "summary.runs.remove.uncommittedOne"),
    (false, _) => (Icon::FileDiff, "summary.runs.remove.uncommittedOther"),
  };
  let state = ctx.t_args(key, [("owner", owner.to_string()), ("count", files.to_string())]);
  let listed = (files > 0).then(|| {
    Text::new(&file_list(&worktree.uncommitted, separator))
      .variant(ExtraTypography::MonoNote.role())
      .width(Dimension::Pct(100.0))
  });
  let unread = unread_entry(worktree).map(|problem| {
    Text::new(&message_text(ctx, &problem))
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::Danger)
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(note(glyph, &state))
    .with_children(listed)
    .with_children(unread)
    .into()
}

fn branch_line(ctx: &Ctx, branch: &LeftBranch) -> Element {
  let (glyph, key) = if branch.merged {
    (Icon::GitMerge, "summary.runs.remove.branchDeleted")
  } else {
    (Icon::GitBranch, "summary.runs.remove.branchKept")
  };
  note(glyph, &ctx.t_args(key, [("branch", branch.name.as_str())]))
}

/// The switch that agrees to discard the uncommitted changes listed, while removing loses files.
fn discard(ctx: &Ctx, confirm: &Confirm) -> Option<Element> {
  if !confirm.leftovers.loses_files() {
    return None;
  }
  let flip = confirm.discard.clone();
  let number = confirm.run.number().get();
  let switch = Toggle::new(
    format!("run-remove-discard-{number}"),
    ToggleKind::Switch,
    confirm.discard.get(),
    ctx.t("summary.runs.remove.discard"),
  );
  Some(if confirm.removing {
    switch.into()
  } else {
    switch.on_change(move |on| flip.set(on)).into()
  })
}

fn actions(ctx: &mut Ctx, confirm: &Confirm) -> Element {
  let number = confirm.run.number().get();
  let state = PageState::of(ctx);
  let cancel = ActionButton::new(
    format!("run-remove-cancel-{number}"),
    ButtonKind::Secondary,
    ctx.t("summary.runs.remove.cancel"),
  )
  .in_form();
  let cancel = if confirm.removing {
    cancel.disabled()
  } else {
    let state = state.clone();
    cancel.on_click(move || state.removal.set(Removal::closed()))
  };
  let label = if confirm.leftovers.worktrees.len() > 1 {
    "summary.runs.remove.confirmOther"
  } else {
    "summary.runs.remove.confirmOne"
  };
  let remove = ActionButton::new(format!("run-remove-start-{number}"), ButtonKind::Danger, ctx.t(label))
    .icon(Icon::FolderX)
    .in_form();
  let remove = if confirm.removing {
    remove.loading()
  } else if confirm.waits || (confirm.leftovers.loses_files() && !confirm.discard.get()) {
    remove.disabled()
  } else {
    let (job, args) = (confirm.job.clone(), remove_args(ctx, confirm, state));
    remove.on_click(move || job.start_remove(args.clone()))
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Spacer::new().flex(1.0))
    .child(cancel)
    .child(remove)
    .into()
}

/// The exact worktrees and uncommitted files that the confirmation listed. Once they are gone the
/// confirmation closes, and the task is read again.
fn remove_args(ctx: &mut Ctx, confirm: &Confirm, state: PageState) -> RemoveArgs {
  let services = Services::of(ctx);
  let feed = confirm.project.feed.clone();
  RemoveArgs {
    open: confirm.project.open.clone(),
    task: confirm.run.task(),
    key: confirm.key.to_owned(),
    number: confirm.run.number().get(),
    run: confirm.run.as_ref().clone(),
    confirmed: confirm.leftovers.clone(),
    programs: services.programs,
    activity: services.activity,
    separator: ctx.t("task.listSeparator"),
    changed: Arc::new(move || feed.saved()),
    by: Remover::Person(Arc::new(move || state.removal.set(Removal::closed()))),
  }
}
