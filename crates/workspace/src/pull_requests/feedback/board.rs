//! A pull request's agent feedback as its page shows it (`Pull request feedback` `seN1p`): each thread with where it
//! stands, what happened to its current request, its posts and the agent's answers, where its lines are at the pull
//! request's head, and the fix commit or the work under way; the agent the threads go to, with the run and the
//! task's budget; and whether a new thread can be sent, and what sending does. Building it blocks on SQLite and git.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::AgentProvider,
  conversation::{Delivery, Message, Recipient},
  diff::Diff,
  id::{FeedbackId, FeedbackPostId, MessageId, ResourceId, StepExecutionId},
  integration::commit_subject,
  project::TaskKey,
  pull_request::feedback::{FeedbackAnchor, FeedbackKind, FeedbackThread, Located, locate},
  revision::Revision,
  run::Checkpoint,
  time::Timestamp,
};
use orchester_exec::programs::Programs;

use super::{
  dispatch::{Slots, TaskState, hold_of},
  sources::{SourceRef, sources},
  state::{Facts, ThreadEvent, ThreadState, events_of, state_of},
};
use crate::tasks::{
  changes::{Checkpointed, GIT, checkpointed_in},
  conversation::{Conversation, PanelState, composer::ComposerMode},
  detail::{RunView, StepView, TaskDetail},
};

/// The agent feedback of a pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedbackBoard {
  /// The pull request's linked task; `None`: it has none, so no agent can be asked.
  pub task: Option<BoardTask>,
  /// Oldest first.
  pub threads: Vec<ThreadView>,
}

impl FeedbackBoard {
  /// The threads that are open.
  pub fn open(&self) -> impl Iterator<Item = &ThreadView> {
    self.threads.iter().filter(|thread| thread.state.is_open())
  }

  pub fn thread(&self, id: FeedbackId) -> Option<&ThreadView> {
    self.threads.iter().find(|thread| thread.id == id)
  }
}

/// The linked task as feedback reaches it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardTask {
  pub key: TaskKey,
  /// What sending a new thread does.
  pub gate: Gate,
  /// The agents a thread can go to: each agent profile of the task's run, in pipeline order.
  pub targets: Vec<Target>,
  /// The agent a new thread goes to unless the person picks another: the run's agent that writes code.
  pub default: Option<ResourceId>,
  /// The run the feedback goes to and what it spent on it.
  pub run: Option<RunFacts>,
}

impl BoardTask {
  pub fn target(&self, profile: ResourceId) -> Option<&Target> {
    self.targets.iter().find(|target| target.profile == profile)
  }
}

/// An agent of the task's run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
  pub profile: ResourceId,
  pub role: Arc<str>,
  pub provider: Option<AgentProvider>,
  pub model: Option<Arc<str>>,
}

/// What sending a new thread does now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gate {
  /// It goes to the run.
  Send,
  /// The run ended ready for review at `ended_at`: sending follows it up, on its branch, with the agent's
  /// permissions and the task's limits.
  FollowUp { ended_at: Option<Timestamp> },
  /// No agent of the task reads messages any more, and sending cannot bring one back.
  Closed(PanelState),
}

/// The task's latest run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFacts {
  pub number: u32,
  pub branch: Arc<str>,
  /// Counted tokens the agents spent on feedback requests in this run.
  pub feedback_tokens: u64,
  /// Counted tokens the run used of its budget; `None` without one.
  pub budget: Option<Budget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
  pub used: u64,
  pub limit: u64,
}

impl Budget {
  pub const fn left(self) -> u64 {
    self.limit.saturating_sub(self.used)
  }
}

/// One thread as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadView {
  pub id: FeedbackId,
  /// What it asks now.
  pub kind: FeedbackKind,
  pub anchor: FeedbackAnchor,
  /// The revision the anchor was written on.
  pub revision: Revision,
  /// Where its lines are at the pull request's head, when that head is another revision and its diff is read.
  pub at_head: Option<Located>,
  pub state: ThreadState,
  /// What happened to the current request, oldest first.
  pub events: Vec<ThreadEvent>,
  pub posts: Vec<PostView>,
  /// The agent it goes to.
  pub recipient: Arc<str>,
  /// The held post Withdraw request withdraws.
  pub held: Option<FeedbackPostId>,
  /// The current request's message that its agent's CLI did not take, which Retry sends after the turn.
  pub undelivered: Option<MessageId>,
  /// Counted tokens the agent spent on the current request so far.
  pub tokens: Option<u64>,
  /// The agent's work on it, while it works.
  pub working: Option<WorkingView>,
  /// The commit the agent made for the current fix request.
  pub commit: Option<CommitView>,
  /// The places the agent's latest answer points to.
  pub sources: Vec<SourceRef>,
}

impl ThreadView {
  /// The current request: the latest post not withdrawn.
  pub fn current(&self) -> &PostView {
    self
      .posts
      .iter()
      .rev()
      .find(|post| !post.withdrawn)
      .unwrap_or(&self.posts[0])
  }

  /// The file it is about; `None` for the whole pull request.
  pub fn path(&self) -> Option<&str> {
    self.anchor.lines().map(|lines| lines.path())
  }
}

/// A person's post and the agent's answer to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostView {
  pub id: FeedbackPostId,
  pub kind: FeedbackKind,
  pub body: Arc<str>,
  /// The pull request's head it was written at.
  pub revision: Revision,
  pub at: Timestamp,
  pub withdrawn: bool,
  pub reply: Option<ReplyView>,
}

/// What the agent answered at the end of the turn that read a post.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplyView {
  pub at: Timestamp,
  pub body: Arc<str>,
}

/// The agent's turn on the current request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingView {
  pub step: Arc<str>,
  pub since: Timestamp,
  /// What the agent last reported of its work, if it did.
  pub progress: Option<Arc<str>>,
}

/// A commit the agent made for a fix request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitView {
  pub revision: Revision,
  pub at: Timestamp,
  /// Its message's first line, when git tells it.
  pub subject: Option<Arc<str>>,
  /// What it changed against the request's revision, when git tells it: files, added and removed lines.
  pub changes: Option<(usize, u32, u32)>,
  /// The pull request's head is this commit: GitHub has it.
  pub on_pull_request: bool,
}

/// What the board is built from besides the stored threads.
pub struct Inputs<'a> {
  pub root: &'a Path,
  pub programs: &'a Programs,
  /// The pull request's head as last read, and its diff when it is read.
  pub head: Option<(&'a Revision, Option<&'a Diff>)>,
  pub slots: Option<Slots>,
  /// The runner that served the project is gone.
  pub lost: bool,
}

/// The board of `threads`, all on one pull request, whose linked task is `detail`'s (`None`: it has none).
pub(super) fn board(
  inputs: &Inputs<'_>,
  detail: Option<(&TaskDetail, &Conversation, &[Checkpoint])>,
  threads: &[FeedbackThread],
) -> FeedbackBoard {
  let Some((detail, conversation, checkpoints)) = detail else {
    return FeedbackBoard {
      task: None,
      threads: threads.iter().map(|thread| bare(inputs, thread)).collect(),
    };
  };
  let run = detail.run.as_ref();
  let all: Vec<&FeedbackThread> = threads.iter().collect();
  let task_state = TaskState {
    conversation,
    run,
    messages: &detail.messages,
    slots: inputs.slots,
  };
  let hold = |thread: &FeedbackThread, post: &orchester_core::pull_request::feedback::FeedbackPost| {
    hold_of((thread, post), &task_state, &all).unwrap_or(super::state::Hold::Next)
  };
  let entry = |message: &Message| {
    conversation
      .entries
      .iter()
      .find(|entry| entry.id == message.id())
      .map(|entry| entry.state.clone())
  };
  let facts = Facts {
    messages: &detail.messages,
    checkpoints,
    entry: &entry,
    hold: &hold,
  };
  let views: Vec<ThreadView> = threads
    .iter()
    .map(|thread| view(inputs, (thread, &facts), run))
    .collect();
  FeedbackBoard {
    task: Some(board_task(detail, conversation, &views)),
    threads: views,
  }
}

/// A thread of a pull request without a linked task: its posts only.
fn bare(inputs: &Inputs<'_>, thread: &FeedbackThread) -> ThreadView {
  let state = match thread.resolved_at() {
    Some(at) => ThreadState::Resolved { at },
    None => ThreadState::Queued(super::state::Hold::Closed(PanelState::Ended)),
  };
  ThreadView {
    id: thread.id(),
    kind: thread.kind(),
    anchor: thread.anchor().clone(),
    revision: thread.revision().clone(),
    at_head: at_head(inputs, thread),
    state,
    events: Vec::new(),
    posts: thread.posts().iter().map(|post| post_view(post, None)).collect(),
    recipient: Arc::from(""),
    held: None,
    undelivered: None,
    tokens: None,
    working: None,
    commit: None,
    sources: Vec::new(),
  }
}

fn post_view(post: &orchester_core::pull_request::feedback::FeedbackPost, reply: Option<&Message>) -> PostView {
  PostView {
    id: post.id(),
    kind: post.kind(),
    body: Arc::from(post.body().as_str()),
    revision: post.revision().clone(),
    at: post.written_at(),
    withdrawn: post.withdrawn_at().is_some(),
    reply: reply.map(|reply| ReplyView {
      at: reply.created_at(),
      body: Arc::from(reply.body().as_str()),
    }),
  }
}

fn view(inputs: &Inputs<'_>, (thread, facts): (&FeedbackThread, &Facts<'_>), run: Option<&RunView>) -> ThreadView {
  let state = state_of(thread, facts);
  let events = events_of(thread, &state, facts);
  let current = thread.current();
  let outcome = facts.outcome(current);
  let posts: Vec<PostView> = thread
    .posts()
    .iter()
    .map(|post| post_view(post, facts.outcome(post).and_then(|outcome| outcome.reply)))
    .collect();
  let sources = posts
    .iter()
    .rev()
    .find_map(|post| post.reply.as_ref())
    .map(|reply| sources(&reply.body))
    .unwrap_or_default();
  let delivered = outcome.as_ref().and_then(|outcome| delivered(outcome.message));
  let recipient = run
    .and_then(|run| role_of(run, thread.recipient()))
    .unwrap_or_else(|| Arc::from(""));
  ThreadView {
    id: thread.id(),
    kind: thread.kind(),
    anchor: thread.anchor().clone(),
    revision: thread.revision().clone(),
    at_head: at_head(inputs, thread),
    events,
    posts,
    recipient,
    held: current.is_held().then(|| current.id()),
    undelivered: match &state {
      ThreadState::NotDelivered {
        why: super::state::NotDelivered::Cli(_),
        ..
      } => current.message(),
      _ => None,
    },
    tokens: run
      .zip(delivered)
      .map(|(run, (execution, at))| tokens_since(run, execution, at)),
    working: match (&state, run.zip(delivered)) {
      (ThreadState::Working { since }, Some((run, (execution, _)))) => working(run, execution, *since),
      _ => None,
    },
    commit: match &state {
      ThreadState::ReadyToVerify { at, commit } => Some(commit_view(inputs, (commit, *at), current.revision())),
      _ => None,
    },
    sources,
    state,
  }
}

/// Where `thread`'s lines are at the pull request's head, when it is another revision than the thread's and its
/// diff is read.
fn at_head(inputs: &Inputs<'_>, thread: &FeedbackThread) -> Option<Located> {
  let lines = thread.anchor().lines()?;
  let (head, diff) = inputs.head?;
  if head == thread.revision() {
    return None;
  }
  let file = diff?.files().iter().find(|file| file.path() == lines.path());
  Some(locate(lines, file))
}

/// The execution that took `message` and when.
fn delivered(message: &Message) -> Option<(StepExecutionId, Timestamp)> {
  match message.delivery()? {
    Delivery::Delivered { execution, at, .. }
    | Delivery::Acknowledged {
      execution,
      delivered_at: at,
      ..
    } => Some((execution, at)),
    Delivery::Queued | Delivery::NotDelivered { .. } => None,
  }
}

/// Counted tokens of `execution`'s invocations that started at `at` or later: the turn that took the request and
/// the ones after it.
fn tokens_since(run: &RunView, execution: StepExecutionId, at: Timestamp) -> u64 {
  run
    .invocations_of(execution)
    .filter(|invocation| invocation.ended_at.is_none_or(|ended| ended >= at))
    .map(|invocation| invocation.usage.counted())
    .sum()
}

fn working(run: &RunView, execution: StepExecutionId, since: Timestamp) -> Option<WorkingView> {
  let step = run.step_of(execution)?;
  Some(WorkingView {
    step: step.name.clone(),
    since,
    progress: run.reported(execution).map(Arc::from),
  })
}

fn commit_view(inputs: &Inputs<'_>, (revision, at): (&Revision, Timestamp), from: &Revision) -> CommitView {
  let runner = inputs.programs.runner();
  let subject = commit_subject(&runner, Path::new(GIT), (inputs.root, revision))
    .inspect_err(|error| tracing::warn!(%error, %revision, "cannot read the subject of a fix commit"))
    .ok()
    .filter(|subject| !subject.is_empty())
    .map(Arc::from);
  let changes = match checkpointed_in(inputs.root, inputs.programs, from, Some(revision)) {
    Ok(Checkpointed::Read { changes, .. }) => Some((
      changes.diff.files().len(),
      changes.diff.additions(),
      changes.diff.deletions(),
    )),
    Ok(Checkpointed::TooLarge { size, .. }) => Some((
      usize::try_from(size.files).unwrap_or(usize::MAX),
      size.additions,
      size.deletions,
    )),
    Ok(Checkpointed::NoCheckpoint) => None,
    Err(error) => {
      tracing::warn!(%error, %revision, "cannot read what a fix commit changed");
      None
    }
  };
  CommitView {
    revision: revision.clone(),
    at,
    subject,
    changes,
    on_pull_request: inputs.head.is_some_and(|(head, _)| head == revision),
  }
}

/// The role of agent profile `profile` among the run's steps.
fn role_of(run: &RunView, profile: ResourceId) -> Option<Arc<str>> {
  step_of(run, profile).and_then(|step| step.agent.clone())
}

fn step_of(run: &RunView, profile: ResourceId) -> Option<&StepView> {
  run.steps.iter().filter(|step| !step.tool_step).find(|step| {
    matches!(
      run.run.snapshot().step(&step.key).map(|pinned| pinned.executor()),
      Some(orchester_core::run::StepExecutor::Agent { profile: pinned, .. }) if pinned.resource == profile
    )
  })
}

fn board_task(detail: &TaskDetail, conversation: &Conversation, views: &[ThreadView]) -> BoardTask {
  let run = detail.run.as_ref();
  let targets: Vec<Target> = conversation
    .recipients
    .iter()
    .filter_map(|choice| match choice.recipient {
      Recipient::Agent(profile) => {
        let step = run.and_then(|run| step_of(run, profile));
        Some(Target {
          profile,
          role: choice.agent.clone()?,
          provider: step.and_then(|step| step.provider),
          model: step.and_then(|step| step.model.clone()),
        })
      }
      Recipient::Orchestrator => None,
    })
    .collect();
  let default = run
    .and_then(|run| {
      run
        .steps
        .iter()
        .filter(|step| !step.tool_step && step.writes())
        .find_map(|step| targets.iter().find(|target| Some(&target.role) == step.agent.as_ref()))
    })
    .or_else(|| targets.first())
    .map(|target| target.profile);
  BoardTask {
    key: detail.task.key().clone(),
    gate: gate(detail, conversation, default),
    targets,
    default,
    run: run.map(|run| RunFacts {
      number: run.run.number().get(),
      branch: Arc::from(run.run.branch().as_str()),
      feedback_tokens: views.iter().filter_map(|view| view.tokens).sum(),
      budget: run.run.budget().map(|budget| Budget {
        used: run.run.used().get(),
        limit: budget.get(),
      }),
    }),
  }
}

/// What sending a new thread to `profile` does now.
fn gate(detail: &TaskDetail, conversation: &Conversation, profile: Option<ResourceId>) -> Gate {
  let run = detail.run.as_ref();
  let recipient = profile.map_or(Recipient::Orchestrator, Recipient::Agent);
  match ComposerMode::of(conversation, run, recipient) {
    ComposerMode::Closed | ComposerMode::RunAgain => Gate::Closed(conversation.state),
    ComposerMode::FollowUp => Gate::FollowUp {
      ended_at: run.and_then(|run| run.finished_at),
    },
    _ => Gate::Send,
  }
}
