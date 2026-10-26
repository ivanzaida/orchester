//! The decision of a task whose latest run is delivered by the project's merge queue (ORC-19,
//! `orchester_workspace::merge_queue`), in place of Ready for review: its pipeline ends in a Deliver step, so the
//! queue merges it and *Merge into main* is not offered. While the run joins the queue, waits its turn or is delivered,
//! the state card says where it stands with the phases of its delivery (`ulEuh`, integrating `RRk9E`, [`card`]);
//! waiting for the person's approval, *Review before merging* with *Merge into main*, *Open changes* and *Reject*
//! (`wN5Bz`, [`side`]); after the Integrate step could not resolve its conflicts, why, the files, what Integrate did
//! and *Resolve myself* or *Reject* (`hKct7`); while the person resolves them in the run's worktree, where and what
//! *I've resolved it* checks (`fAcWt`, [`resolve`]); once merged, where it went and what was cleared (`lvupO`). Each
//! shows its decision first at the top of the Summary, then the phases ([`rows::stages`]) and what changed and its
//! verification with what the delivery added; the run's facts end with its *Delivery* rows ([`details`]). A person's
//! decision changes the queue off the UI thread; the queue's driver goes on from there.

mod card;
mod commits;
pub(super) mod details;
mod integrate;
mod resolve;
mod rows;
mod side;

use std::time::Duration;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, Interval},
    events::is_synthetic_input,
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::RunId,
  merge_queue::{Left, QueueState},
  resource::DeliverMode,
  run::{Answerer, PauseReason, RunStatus},
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_workspace::{
  background,
  display::short_revision,
  merge_queue::{self, QueueRefusal, TaskDelivery},
  messages::Message,
  tasks::{delivery::Delivery, detail::RunView, run::control_message},
};

use self::rows::Stage;
use super::{
  TaskContext, arranged,
  ready::{changed, in_worktree},
};
use crate::{services::Services, ui::button::ActionButton};

/// How often the run's worktree is read again while the person resolves its conflicts there.
const LOOK: Duration = Duration::from_secs(2);

/// Whether the Summary of `task` is its latest run's delivery: the run's pipeline ends in a Deliver step, it passed
/// every step (or works again on its conflicts in its Integrate step) and a person did not take it out of the queue.
/// A run that committed nothing never joins the queue (ORC-93): its Summary is that of a run with nothing to merge.
pub fn shown(task: &TaskContext) -> bool {
  let detail = &task.detail;
  let (Some(run), Some(queue)) = (detail.run.as_ref(), detail.queue.as_ref()) else {
    return false;
  };
  let status = run.run.status();
  match queue.delivery.as_ref().map(|delivery| &delivery.entry.state) {
    Some(QueueState::Removed) => false,
    Some(QueueState::Merged(_)) => true,
    Some(QueueState::Integrating { .. }) => !matches!(
      status,
      RunStatus::Paused(PauseReason::LimitReached(_)) | RunStatus::WaitingForPerson(_)
    ),
    Some(_) => *status == RunStatus::Succeeded,
    None => *status == RunStatus::Succeeded && detail.merge.is_none() && detail.delivery != Delivery::NothingToMerge,
  }
}

/// A person's decision on the run in the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
  /// *Merge into main* on a run waiting for approval.
  Approve,
  /// *Reject*, or *Take out of the queue* after it left it.
  Reject,
  /// *Resolve myself*: the base is merged into the run's worktree for the person.
  ResolveMyself,
  /// *I've resolved it*.
  Resolved,
}

impl DevtoolsInspectable for Act {}

/// A decision on a run, and who pressed it: a person, or an MCP client driving the window.
type Decision = (RunId, Act, Answerer);

/// Why a decision did not happen: what the person reads, and the files *I've resolved it* found conflict markers left
/// in when that was why.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Refused {
  pub message: Message,
  pub markers: Option<Vec<String>>,
}

impl lurq::app::component::DevtoolsInspectable for Refused {}

impl From<Message> for Refused {
  fn from(message: Message) -> Self {
    Self { message, markers: None }
  }
}

/// The person's decisions on the run, the one last pressed and when, and why the last one did not happen.
#[derive(Clone)]
pub(super) struct Acts {
  action: FutureAction<Decision, Act, Refused>,
  pressed: Signal<Option<(Act, Timestamp)>>,
  run: RunId,
}

impl Acts {
  /// Why the last decision did not happen.
  pub(super) fn refused(&self) -> Option<Refused> {
    let state = self.action.state().get();
    if state.is_pending() { None } else { state.error }
  }

  /// The decision under way and when it was pressed.
  pub(super) fn under_way(&self) -> Option<(Act, Timestamp)> {
    self
      .action
      .state()
      .get()
      .is_pending()
      .then(|| self.pressed.get())
      .flatten()
  }

  /// The decision last pressed and when.
  pub(super) fn last(&self) -> Option<(Act, Timestamp)> {
    self.pressed.get()
  }

  /// `button` making `act`: under way while it is, waiting while another decision is.
  pub(super) fn button(&self, button: ActionButton, act: Act) -> ActionButton {
    if self.action.state().get().is_pending() {
      return if self.pressed.get().map(|(pressed, _)| pressed) == Some(act) {
        button.loading()
      } else {
        button.disabled()
      };
    }
    let (action, pressed, run) = (self.action.clone(), self.pressed.clone(), self.run);
    button.on_click(move || {
      // A second press while the first is under way would replace it (lurq's `FutureAction::run`).
      if !action.state().get_untracked().is_pending() {
        let by = if is_synthetic_input() {
          Answerer::Automation
        } else {
          Answerer::Person
        };
        pressed.set(Some((act, Timestamp::now())));
        action.run((run, act, by));
      }
    })
  }
}

pub struct DeliveryDecision {
  /// Counts while the person resolves conflicts in the run's worktree, which is read again then.
  look: Signal<u64>,
  looking: Interval,
  /// *Back to the decision* from the view of the worktree the person resolves in.
  back: Signal<bool>,
  /// The decision last pressed and when, for its button.
  pressed: Signal<Option<(Act, Timestamp)>>,
  action: FutureAction<Decision, Act, Refused>,
}

impl Component for DeliveryDecision {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    let look = ctx.signal(0_u64);
    let next = look.clone();
    let looking = ctx.create_interval(LOOK, move || next.update(|tick| *tick += 1));
    let props = ctx.props::<TaskContext>().clone();
    let action = decide(ctx, &props);
    Self {
      look,
      looking,
      back: ctx.signal(false),
      pressed: ctx.signal(None),
      action,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    // Read on every render, before the parts only some states build: lurq keeps futures by their order. The run's
    // worktree and commits are read again when the queue changes too: *Resolve myself* merges the base into the
    // worktree, and a sync adds its merge commit.
    let read = changed::read(ctx, &props);
    let changes = crate::project::merge_queue::changes(ctx, &props.project.open.repo_root);
    let commits = commits::read(ctx, &props, changes);
    let (Some(run), Some(queue)) = (props.detail.run.as_ref(), props.detail.queue.as_ref()) else {
      return Column::new().into();
    };
    let worktree = in_worktree(ctx, &props, (run.id(), self.look.get() + changes));
    let in_merge = worktree.as_ref().and_then(|worktree| worktree.merge.clone());
    let acts = Acts {
      action: self.action.clone(),
      pressed: self.pressed.clone(),
      run: run.id(),
    };
    let state = queue.delivery.as_ref().map(|delivery| delivery.entry.state.clone());
    let resolving = matches!(state, Some(QueueState::Left(Left::Conflicts { .. }))) && in_merge.is_some();
    if resolving {
      self.looking.start();
    } else {
      self.looking.stop();
    }
    let view = View {
      task: &props,
      run,
      queue,
      read: read.as_ref(),
      commits: commits.as_ref(),
      acts: &acts,
    };
    let (parts, lead) = match &state {
      Some(QueueState::Merged(delivered)) => (view.merged(ctx, delivered), None),
      Some(QueueState::WaitingForApproval) if queue.deliver.mode() == DeliverMode::ApproveToMerge => {
        (view.review(ctx), Some(side::review(ctx, &view)))
      }
      Some(QueueState::Left(Left::Conflicts { files })) => match in_merge.as_ref() {
        Some(resolution) if !self.back.get() => (resolve::resolving(ctx, &view, (files, resolution), &self.back), None),
        _ => {
          let back = self.back.clone();
          (
            integrate::failed(ctx, &view, files),
            Some(side::conflicts(ctx, &view, in_merge.is_some(), &back)),
          )
        }
      },
      Some(QueueState::Left(left)) => (view.left(ctx, left), Some(side::left(ctx, &view, left))),
      _ => (view.delivering(ctx), None),
    };
    // The decision the person makes comes first: what it rests on follows it.
    arranged(lead.into_iter().chain(parts).collect())
  }
}

/// What every view of the delivery reads.
pub(super) struct View<'a> {
  pub task: &'a TaskContext,
  pub run: &'a RunView,
  pub queue: &'a TaskDelivery,
  pub read: Option<&'a Result<orchester_workspace::tasks::changes::Checkpointed, Message>>,
  pub commits: Option<&'a Result<commits::Commits, Message>>,
  pub acts: &'a Acts,
}

impl View<'_> {
  /// The integrator profile's name, or how the words name it without one.
  fn integrator(&self, ctx: &Ctx) -> String {
    self
      .queue
      .integrator
      .clone()
      .unwrap_or_else(|| ctx.t("delivery.task.theIntegrator").to_string())
  }

  /// What the delivery adds to the run's verification.
  fn delivered(&self, ctx: &Ctx) -> Vec<Element> {
    let integrator = self.integrator(ctx);
    self.queue.delivery.as_ref().map_or_else(Vec::new, |delivery| {
      rows::delivery_checks(ctx, delivery, (card::base(self.run), &integrator))
    })
  }

  /// What changed and its verification, `head` at the verification's end.
  fn columns(&self, ctx: &mut Ctx, head: Option<std::sync::Arc<str>>) -> Element {
    let delivered = self.delivered(ctx);
    changed::columns(ctx, self.task, self.read, (delivered, head))
  }

  fn stages(&self, ctx: &Ctx, stage: Stage) -> Element {
    rows::stages(ctx, self.run, self.queue.delivery.as_ref(), stage)
  }

  /// Joining, waiting its turn or being delivered.
  fn delivering(&self, ctx: &mut Ctx) -> Vec<Element> {
    let card = card::state_card(ctx, self.task, (self.run, self.queue));
    let conflicts = match self.queue.delivery.as_ref().map(|delivery| &delivery.entry.state) {
      Some(QueueState::Integrating { files, .. }) => Some(integrate::conflicting(ctx, files, None)),
      _ => None,
    };
    let stage = if matches!(
      self.queue.delivery.as_ref().map(|delivery| &delivery.entry.state),
      Some(QueueState::Blocked(_))
    ) {
      Stage::Waiting
    } else {
      Stage::Working
    };
    let mut parts = vec![card, self.stages(ctx, stage)];
    parts.extend(conflicts);
    parts.push(self.columns(ctx, None));
    parts
  }

  /// *Review before merging*: its changes and commits beside its checks.
  fn review(&self, ctx: &mut Ctx) -> Vec<Element> {
    let size = changed::size(ctx, self.read);
    let checked = self
      .queue
      .delivery
      .as_ref()
      .and_then(|delivery| delivery.entry.checked.as_ref())
      .map(|checked| {
        ctx.t_args(
          "delivery.task.review.onBase",
          [
            ("base", card::base(self.run).to_owned()),
            ("revision", short_revision(checked.base.as_str()).to_owned()),
          ],
        )
      });
    let delivered = self.delivered(ctx);
    let commits = commits::section(ctx, self.commits);
    vec![
      card::review_head(ctx, (self.run, self.queue), size),
      self.stages(ctx, Stage::Waiting),
      changed::review_columns(ctx, self.task, (self.read, commits), (delivered, checked)),
    ]
  }

  /// Merged by the queue.
  fn merged(&self, ctx: &mut Ctx, delivered: &orchester_core::merge_queue::Delivered) -> Vec<Element> {
    let notice = self
      .queue
      .delivery
      .as_ref()
      .map(|delivery| card::merged(ctx, self.run, (delivery, delivered)));
    let key = if delivered.kind == orchester_core::integration::MergeKind::Squash {
      "delivery.task.merged.squashCommit"
    } else {
      "delivery.task.merged.mergeCommitHead"
    };
    let head = ctx.t_args(key, [("revision", short_revision(delivered.revision.as_str()))]);
    notice
      .into_iter()
      .chain([self.stages(ctx, Stage::Done), self.columns(ctx, Some(head))])
      .collect()
  }

  /// Left the queue for a reason other than conflicts.
  fn left(&self, ctx: &mut Ctx, left: &Left) -> Vec<Element> {
    let base = card::base(self.run);
    let notice = crate::ui::notice::Notice::new(
      "task-notice",
      crate::ui::notice::NoticeTone::Warning,
      ctx.t_args("delivery.task.left.title", [("base", base)]),
    )
    .icon(crate::ui::icon::Icon::TriangleAlert)
    .body(crate::screens::merge_queue::words::left_reason(ctx, left));
    vec![notice.into(), self.stages(ctx, Stage::Waiting), self.columns(ctx, None)]
  }
}

/// Makes the person's decisions on the queue of `task`'s project off the UI thread; *I've resolved it* also sends the
/// run's work back to its Integrate step through the project's runner, then puts the run back in the queue. Only a
/// person approves a merge: the press of an MCP client driving the window is refused by the queue's rule.
fn decide(ctx: &mut Ctx, task: &TaskContext) -> FutureAction<Decision, Act, Refused> {
  let services = Services::of(ctx);
  let (runners, programs, project) = (services.runners, services.programs, task.project.open.clone());
  ctx.future_action(move |(run, act, by): Decision| {
    let (runners, programs, project) = (runners.clone(), programs.clone(), project.clone());
    async move {
      let root = project.repo_root.clone();
      let made = background::run(move || made(&root, &programs, (run, act), by))
        .await?
        .map_err(|refusal| refused(&refusal))?;
      let Made::Resolved(command) = made else {
        return Ok(act);
      };
      let sent = match command {
        Some(command) => runners.control_run_in(&project, command).await.map_err(|error| {
          tracing::warn!(%error, "the runner did not take the resolved run back");
          control_message(&error)
        }),
        None => Ok(()),
      };
      let root = project.repo_root.clone();
      background::run(move || merge_queue::requeue(&root, run, sent, Timestamp::now()))
        .await?
        .map_err(|refusal| refused(&refusal))?;
      Ok(act)
    }
  })
}

fn refused(refusal: &QueueRefusal) -> Refused {
  tracing::info!(code = refusal.code(), "a decision on the merge queue did not happen");
  Refused {
    message: refusal.message(),
    markers: refusal.markers_left().map(<[String]>::to_vec),
  }
}

/// What a decision left to do once it was made.
enum Made {
  Done,
  /// The person resolved the conflicts: the command sends the run's work back to its Integrate step, if it has one,
  /// and the run goes back in the queue after it.
  Resolved(Option<Command>),
}

/// Makes `act` on `run`, pressed by `by`, in the queue of the project at `root`. *Resolve myself* where git merges the
/// base without a conflict now goes on as *I've resolved it* at once: nothing is left for the person.
fn made(
  root: &std::path::Path,
  programs: &orchester_exec::programs::Programs,
  (run, act): (RunId, Act),
  by: Answerer,
) -> Result<Made, QueueRefusal> {
  let now = Timestamp::now();
  match act {
    Act::Approve => merge_queue::approve(root, run, now, by).map(|()| Made::Done),
    Act::Reject => merge_queue::reject(root, run, now).map(|()| Made::Done),
    Act::ResolveMyself => {
      if merge_queue::resolve_myself(root, programs, run)?.is_empty() {
        merge_queue::resolved(root, programs, run).map(Made::Resolved)
      } else {
        Ok(Made::Done)
      }
    }
    Act::Resolved => merge_queue::resolved(root, programs, run).map(Made::Resolved),
  }
}
