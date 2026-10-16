//! Run again for a task whose latest run failed or was canceled, or was interrupted and lost its
//! worktree (core's [`can_run_again_after`]): a button at the right of the title, unavailable with its
//! reason while core's rule says the task cannot run again. It opens its confirmation in place, under
//! the header, rather than a dialog: what the next run uses, the switch that removes the run before's
//! worktrees with it, and Cancel and Start. One request per confirmation: its idempotency key is made
//! when the confirmation opens, so pressing Start again, or retrying after a lost reply, never starts a
//! second run.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  integration::{Leftovers, WORKTREES_DIR},
  run::{RunAgainBlock, can_run_again_after},
  sharing::TaskRevision,
  task::TaskLimits,
  tokens::compact_tokens,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  budget::whole_minutes,
  display,
  messages::{self, Message},
  runner::Runners,
  sharing::Servers,
  tasks::{
    detail::{RunView, TaskDetail},
    integration::file_list,
    run_again::{NextRun, RunAgainCheck, block_message, request_messages},
  },
  workspace::OpenProject,
};

use super::controls::{Control, Reason, named_steps};
use crate::{
  project::ProjectFeed,
  theme::{ExtraTypography, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    detail_row::{DetailValue, detail_row},
    icon::Icon,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// Whether the task can run again after its latest run, and with what; nothing for a latest run that
/// cannot be followed by Run again.
#[derive(Clone, Debug, Default, PartialEq)]
struct Checked(Option<RunAgainCheck>);

impl lurq::app::component::DevtoolsInspectable for Checked {}

/// The open confirmation, with the key of its one request; `None` while it is closed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Confirming(pub Option<IdempotencyKey>);

impl lurq::app::component::DevtoolsInspectable for Confirming {}

/// What makes Run again's check read again: tasks were saved, a run changed, the project's limits
/// changed (the day's budget), counted as the project's feed counts them.
pub type CheckAgain = (u64, u64, u64);

/// Whether the task runs again after its latest run, read again whenever `again` changes; `None` for a
/// latest run that cannot be followed by Run again, and until it was read.
pub fn check(
  ctx: &mut Ctx,
  (root, programs): (PathBuf, Programs),
  detail: &TaskDetail,
  again: CheckAgain,
) -> Option<Result<RunAgainCheck, Message>> {
  let after = detail
    .run
    .as_ref()
    .filter(|run| can_run_again_after(&run.run, run.worktree_removed.is_some()))
    .map(RunView::id);
  let deps = (root, detail.task.id(), after, again);
  let load = ctx.future(deps, move |(root, task, after, _)| {
    let programs = programs.clone();
    async move {
      let Some(after) = after else {
        return Ok(Checked(None));
      };
      background::run(move || RunAgainCheck::load_in(&root, &programs, task, after))
        .await?
        .map(|check| Checked(Some(check)))
        .map_err(|error| {
          tracing::warn!(%error, "cannot check whether the task can run again");
          messages::store_error(&error)
        })
    }
  });
  let state = load.state().get();
  let after = after?;
  match (state.data, state.error) {
    // A check for an earlier run of the task says nothing about this one.
    (Some(Checked(Some(check))), None) if check.after == after => Some(Ok(check)),
    (_, Some(error)) => Some(Err(error)),
    _ => None,
  }
}

/// What one confirmation asks the runner.
#[derive(Clone)]
pub struct Request {
  key: IdempotencyKey,
  task: TaskId,
  /// The task as the confirmation showed it: one changed since does not run.
  revision: TaskRevision,
  after: RunId,
  remove: Option<Leftovers>,
  pipeline: Arc<str>,
  /// The pipeline's steps by key and name, so a refusal names a step as people read it.
  steps: Arc<[(Arc<str>, Arc<str>)]>,
}

/// Why the runner did not start the run the confirmation with `key` asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused {
  key: IdempotencyKey,
  messages: Vec<Message>,
}

impl lurq::app::component::DevtoolsInspectable for Refused {}

pub type StartAction = FutureAction<Request, (), Refused>;

/// Asks the runner to run the task again; closes the confirmation once the new run started. The task
/// is read again either way: the new run, or what blocked it.
pub fn start_action(
  ctx: &mut Ctx,
  (runners, servers): (Runners, Servers),
  (project, feed): (OpenProject, ProjectFeed),
  open: Signal<Confirming>,
) -> StartAction {
  ctx.future_action(move |request: Request| {
    let (runners, servers, project, feed, open) = (
      runners.clone(),
      servers.clone(),
      project.clone(),
      feed.clone(),
      open.clone(),
    );
    async move {
      let refused = |messages| Refused {
        key: request.key,
        messages,
      };
      // A shared task runs again only under the person's claim.
      let claim = servers
        .claim_for_work(&project.repo_root, request.task)
        .await
        .map_err(|refusal| refused(vec![refusal.message()]))?;
      let started = runners
        .run_again_removing_in(
          &project,
          request.key,
          (request.task, request.after, request.revision),
          request.remove.clone(),
        )
        .await;
      feed.saved();
      match started {
        Ok(run) => {
          tracing::info!(%run, "the task runs again");
          open.set(Confirming(None));
          Ok(())
        }
        Err(error) => {
          tracing::warn!(%error, "the runner did not run the task again");
          claim.release(&servers).await;
          let named = request_messages(&error, &request.pipeline)
            .into_iter()
            .map(|message| named_steps(message, &request.steps))
            .collect();
          Err(refused(named))
        }
      }
    }
  })
}

/// The Run again button, unavailable with why when the task cannot run again or no runner is connected
/// to start it; nothing until the check was read. `busy`: git removes or merges the latest run's worktrees, which must
/// end first.
pub fn button(
  ctx: &Ctx,
  check: Option<&Result<RunAgainCheck, Message>>,
  (open, remove): (&Signal<Confirming>, &Signal<bool>),
  (busy, online): (bool, bool),
) -> Option<Control> {
  let button =
    ActionButton::new("task-run-again", ButtonKind::Secondary, ctx.t("task.runAgain.action")).icon(Icon::RotateCcw);
  let reason = match check? {
    Err(error) => Reason {
      short: ctx.t("task.reasons.runAgainUnknown"),
      full: message_text(ctx, error),
    },
    Ok(check) => match &check.next {
      Err(block) => Reason {
        short: short_block(ctx, *block, &check.pipeline),
        full: message_text(ctx, &block_message(*block, &check.pipeline)),
      },
      Ok(_) if !online => Reason {
        short: ctx.t("task.reasons.offline"),
        full: ctx.t("task.controls.offline"),
      },
      Ok(_) if busy || open.get().0.is_some() => {
        return Some(Control {
          button: button.disabled(),
          reason: None,
        });
      }
      Ok(_) => {
        let (open, remove) = (open.clone(), remove.clone());
        let button = button.on_click(move || {
          remove.set(false);
          open.set(Confirming(Some(IdempotencyKey::generate())));
        });
        return Some(Control { button, reason: None });
      }
    },
  };
  Some(Control {
    button: button.unavailable(reason.full.clone()),
    reason: Some(reason),
  })
}

/// Why the task cannot run again, in a few words.
fn short_block(ctx: &Ctx, block: RunAgainBlock, pipeline: &str) -> Arc<str> {
  match block {
    RunAgainBlock::TaskBlocked => ctx.t("task.reasons.runAgainBlocked"),
    RunAgainBlock::NotEnded => ctx.t("task.reasons.runAgainChanged"),
    RunAgainBlock::RunActive { number } => {
      ctx.t_args("task.reasons.runAgainActive", [("run", number.get().to_string())])
    }
    RunAgainBlock::PipelineNotPublished => {
      ctx.t_args("task.reasons.runAgainUnpublished", [("pipeline", pipeline.to_owned())])
    }
    RunAgainBlock::DailyBudgetUsed { .. } => ctx.t("task.reasons.runAgainDailyBudget"),
  }
}

/// What the confirmation is drawn from.
pub struct Confirmation<'a> {
  pub detail: &'a TaskDetail,
  pub run: &'a RunView,
  pub check: &'a RunAgainCheck,
  pub next: &'a NextRun,
  pub key: IdempotencyKey,
  /// What the run before left, once read.
  pub leftovers: Option<&'a Leftovers>,
  pub remove: &'a Signal<bool>,
  pub open: &'a Signal<Confirming>,
  pub action: &'a StartAction,
  /// Git removes or merges the run before's worktrees: the run cannot start, nor remove them, until
  /// that ends.
  pub busy: bool,
}

/// The confirmation in place (`Run again` card), under the header.
pub fn confirmation(ctx: &mut Ctx, confirm: &Confirmation) -> Element {
  let state = confirm.action.state().get();
  let pending = state.is_pending();
  // A refusal of an earlier confirmation is not this one's.
  let errors = state
    .error
    .filter(|refused| !pending && refused.key == confirm.key)
    .map(|refused| refused.messages)
    .unwrap_or_default();
  let next = confirm.next.number.get().to_string();
  let key = confirm.detail.task.key().to_string();
  card("task-run-again-confirm", CardSurface::Decision)
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(crate::theme::ExtraSpacing::Tight.role())
        .child(
          Text::new(&ctx.t_args("task.runAgain.title", [("run", next.as_str()), ("task", key.as_str())]))
            .id("task-run-again-title")
            .variant(TaskText::SectionTitle.role()),
        )
        .child(Text::new(&explanation(ctx, confirm)).variant(ExtraTypography::Prose.role())),
    )
    .child(properties(ctx, confirm))
    .with_children(option(ctx, confirm))
    .with_children(errors.iter().map(|error| {
      Text::new(&message_text(ctx, error))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Danger)
        .width(Dimension::Pct(100.0))
    }))
    .child(actions(ctx, confirm, pending))
    .into()
}

fn explanation(ctx: &Ctx, confirm: &Confirmation) -> Arc<str> {
  let next = confirm.next.number.get().to_string();
  let previous = confirm.run.run.number().get().to_string();
  match confirm.next.base.as_ref().and_then(|base| base.branch.as_ref()) {
    Some(branch) => ctx.t_args(
      "task.runAgain.explanation.branch",
      [
        ("run", next.as_str()),
        ("base", branch.as_str()),
        ("previous", previous.as_str()),
      ],
    ),
    None => ctx.t_args(
      "task.runAgain.explanation.head",
      [("run", next.as_str()), ("previous", previous.as_str())],
    ),
  }
}

/// "1.4M tokens · 30 min", one of them "no token limit" / "no time limit", or "No limits".
fn limits_text(ctx: &Ctx, limits: TaskLimits) -> Arc<str> {
  if limits == TaskLimits::NONE {
    return ctx.t("task.runAgain.noLimits");
  }
  let budget = limits.budget().map_or_else(
    || ctx.t("task.runAgain.noBudget"),
    |budget| ctx.t_args("task.runAgain.budgetValue", [("budget", compact_tokens(budget.get()))]),
  );
  let time = limits.max_duration().map_or_else(
    || ctx.t("task.runAgain.noTimeLimit"),
    |limit| {
      ctx.t_args(
        "task.runAgain.timeValue",
        [("minutes", whole_minutes(limit).to_string())],
      )
    },
  );
  ctx.t_args("task.runAgain.limitsValue", [("budget", &*budget), ("time", &*time)])
}

/// What the new run uses.
fn properties(ctx: &Ctx, confirm: &Confirmation) -> Element {
  let (next, run) = (confirm.next, confirm.run);
  let version = next.version.to_string();
  let previous_version = run.version.to_string();
  let previous = run.run.number().get().to_string();
  let steps = next.steps.to_string();
  let base = base(ctx, next);
  let limits = limits_text(ctx, next.limits);
  let pipeline = &*confirm.check.pipeline;
  Column::new()
    .id("task-run-again-properties")
    .width(Dimension::Pct(100.0))
    .child(detail_row(
      "task-run-again-pipeline",
      &ctx.t("task.runAgain.pipeline"),
      &ctx.t_args(
        "task.runAgain.pipelineValue",
        [("pipeline", pipeline), ("version", version.as_str())],
      ),
      DetailValue::Text,
    ))
    .child(detail_row(
      "task-run-again-previous",
      &ctx.t_args("task.runAgain.previous", [("run", previous.as_str())]),
      &ctx.t_args(
        "task.runAgain.pipelineValue",
        [("pipeline", &*run.pipeline), ("version", previous_version.as_str())],
      ),
      DetailValue::Text,
    ))
    .child(detail_row(
      "task-run-again-starts",
      &ctx.t("task.runAgain.startsFrom"),
      &ctx.t_args(
        if next.steps == 1 {
          "task.runAgain.startsFromValue.one"
        } else {
          "task.runAgain.startsFromValue.other"
        },
        [("step", &*next.first_step), ("count", steps.as_str())],
      ),
      DetailValue::Text,
    ))
    .child(detail_row(
      "task-run-again-base",
      &ctx.t("task.runAgain.base"),
      &base,
      DetailValue::Mono,
    ))
    .child(detail_row(
      "task-run-again-branch",
      &ctx.t("task.runAgain.branch"),
      next.branch.as_str(),
      DetailValue::Mono,
    ))
    .child(detail_row(
      "task-run-again-worktree",
      &ctx.t("task.runAgain.worktree"),
      &ctx.t_args("task.runAgain.worktreeValue", [("folder", WORKTREES_DIR)]),
      DetailValue::Text,
    ))
    .child(detail_row(
      "task-run-again-limits",
      &ctx.t("task.runAgain.limits"),
      &limits,
      DetailValue::Text,
    ))
    .into()
}

/// Where the next run starts: the checkout's branch at its `HEAD`, a detached `HEAD`, or read when the run
/// starts when git could not tell.
fn base(ctx: &Ctx, next: &NextRun) -> Arc<str> {
  let Some(base) = &next.base else {
    return ctx.t("task.runAgain.baseUnknown");
  };
  let revision = display::short_revision(base.revision.as_str());
  match &base.branch {
    Some(branch) => ctx.t_args(
      "task.runAgain.baseValue",
      [("branch", branch.as_str()), ("revision", revision)],
    ),
    None => ctx.t_args("task.runAgain.baseDetached", [("revision", revision)]),
  }
}

/// *Remove the worktree of run #N*, while the run before has worktrees, and what removing it does.
fn option(ctx: &Ctx, confirm: &Confirmation) -> Option<Element> {
  if confirm.busy {
    return None;
  }
  let leftovers = confirm.leftovers.filter(|left| !left.worktrees.is_empty())?;
  let run = &confirm.run.run;
  let number = run.number().get().to_string();
  let files: Vec<String> = leftovers
    .worktrees
    .iter()
    .flat_map(|worktree| worktree.uncommitted.iter().cloned())
    .collect();
  let note = match files.len() {
    0 => ctx.t_args("task.runAgain.remove.clean", [("branch", run.branch().as_str())]),
    count => {
      let listed = file_list(&files, &ctx.t("task.listSeparator"));
      let key = if count == 1 {
        "task.runAgain.remove.uncommitted.one"
      } else {
        "task.runAgain.remove.uncommitted.other"
      };
      ctx.t_args(key, [("count", count.to_string().as_str()), ("files", listed.as_str())])
    }
  };
  let remove = confirm.remove.clone();
  let switch = Toggle::new(
    "task-run-again-remove",
    ToggleKind::Switch,
    confirm.remove.get(),
    ctx.t_args("task.runAgain.remove.title", [("run", number.as_str())]),
  )
  .on_change(move |on| remove.set(on));
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::ExtraSpacing::Snug.role())
      .child(switch)
      .child(
        Text::new(&note)
          .id("task-run-again-remove-note")
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      )
      .into(),
  )
}

fn actions(ctx: &Ctx, confirm: &Confirmation, pending: bool) -> Element {
  let close = confirm.open.clone();
  let cancel = ActionButton::new(
    "task-run-again-cancel",
    ButtonKind::Secondary,
    ctx.t("task.runAgain.cancel"),
  )
  .in_form();
  let cancel = if pending {
    cancel.disabled()
  } else {
    cancel.on_click(move || close.set(Confirming(None)))
  };
  let next = confirm.next.number.get().to_string();
  let start = ActionButton::new(
    "task-run-again-start",
    ButtonKind::Primary,
    ctx.t_args("task.runAgain.start", [("run", next.as_str())]),
  )
  .icon(Icon::Play)
  .in_form();
  let start = if pending {
    start.loading()
  } else if confirm.busy {
    start.disabled()
  } else {
    let action = confirm.action.clone();
    let request = Request {
      key: confirm.key,
      task: confirm.detail.task.id(),
      revision: confirm.detail.task.revision(),
      after: confirm.run.id(),
      remove: confirm.leftovers.cloned().filter(|_| confirm.remove.get()),
      steps: confirm
        .run
        .steps
        .iter()
        .map(|step| (Arc::from(step.key.as_str()), step.name.clone()))
        .collect(),
      pipeline: confirm.check.pipeline.clone(),
    };
    start.on_click(move || {
      if !action.state().get_untracked().is_pending() {
        action.run(request.clone());
      }
    })
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Spacer::new().flex(1.0))
    .child(cancel)
    .child(start)
    .into()
}
