//! Sending held feedback posts to their task's agent (`docs/design/orchester-workspace.md`: "A fix respects task
//! concurrency, agent capacity, branch/worktree serialization and the remaining task budget; multiple requests must
//! not create concurrent writers on one PR branch"). A post goes as a person's message to its thread's agent, once
//! the run's state lets it go as a plain message: delivered at the agent's next turn, or as a follow-up of a run
//! that is ready for review. It is held while the run stopped at its limit, an agent waits for the person's answer
//! or a plan for their decision (a message would decide them), a follow-up would need an agent slot none is free
//! for, another fix request of the task still changes its branch, or no agent of the task reads messages any more.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  conversation::{Delivery, Message, MessageBody, Recipient},
  diff::Diff,
  id::{RunId, TaskId},
  pull_request::feedback::{FeedbackKind, FeedbackPost, FeedbackThread, Located, locate, request_text},
  store::{PullRequestDiffStore, RepositoryStore},
  time::Timestamp,
};
use orchester_ipc::WorkSummary;
use orchester_store::ProjectDb;

use super::{FeedbackError, state::Hold};
use crate::{
  runner::Runners,
  sharing::Servers,
  tasks::{
    conversation::{
      Conversation,
      composer::{ComposerMode, Queued},
      follow::{RunnerOf, Sent, tell},
      outgoing::conversation_of,
    },
    detail::{RunView, TaskDetail},
  },
  workspace::OpenProject,
};

/// The project's agent slots and how many work, as its runner last reported them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots {
  pub working: u32,
  pub slots: u32,
}

impl Slots {
  pub fn of(work: &WorkSummary) -> Self {
    Self {
      working: u32::try_from(work.agents_working()).unwrap_or(u32::MAX),
      slots: work.agent_slots.get(),
    }
  }

  const fn full(self) -> bool {
    self.working >= self.slots
  }
}

/// A post sent now, whose run the runner is to be told of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dispatched {
  pub task: TaskId,
  pub run: RunId,
}

/// The task's state as a post's hold is decided against.
pub(super) struct TaskState<'a> {
  pub conversation: &'a Conversation,
  pub run: Option<&'a RunView>,
  pub messages: &'a [Message],
  pub slots: Option<Slots>,
}

/// Why `post` of `thread` is held, given the task's state and the task's other threads (`threads`, the thread
/// itself included); `None`: it goes now.
pub(super) fn hold_of(
  (thread, post): (&FeedbackThread, &FeedbackPost),
  task: &TaskState<'_>,
  threads: &[&FeedbackThread],
) -> Option<Hold> {
  let mode = ComposerMode::of(task.conversation, task.run, Recipient::Agent(thread.recipient()));
  match mode {
    ComposerMode::Closed | ComposerMode::RunAgain => return Some(Hold::Closed(task.conversation.state)),
    ComposerMode::RaiseLimit => return Some(Hold::Limit),
    ComposerMode::Queue(Queued::SharedBudget(_)) => return Some(Hold::SharedLimit),
    ComposerMode::Answer { agent, .. } => return Some(Hold::Question(agent)),
    ComposerMode::PlanChanges { agent, .. } => return Some(Hold::Plan(agent)),
    ComposerMode::FollowUp if task.slots.is_some_and(Slots::full) => {
      let slots = task.slots.expect("checked just above");
      return Some(Hold::NoSlot {
        working: slots.working,
        slots: slots.slots,
      });
    }
    _ => {}
  }
  if post.kind() != FeedbackKind::Fix {
    return None;
  }
  fix_before(post, task.messages, threads).map(Hold::AfterFix)
}

/// The thread of another fix request of the task that changes its branch before `post`: one sent whose turn has
/// not ended, else one held that was written earlier.
fn fix_before(
  post: &FeedbackPost,
  messages: &[Message],
  threads: &[&FeedbackThread],
) -> Option<orchester_core::id::FeedbackId> {
  let fixes = || {
    threads.iter().flat_map(|thread| {
      thread
        .posts()
        .iter()
        .filter(move |other| other.kind() == FeedbackKind::Fix && other.id() != post.id())
        .map(move |other| (thread.id(), other))
    })
  };
  let in_flight = fixes().find(|(_, other)| {
    other.message().is_some_and(|id| {
      messages
        .iter()
        .find(|message| message.id() == id)
        .and_then(Message::delivery)
        .is_some_and(|delivery| matches!(delivery, Delivery::Queued | Delivery::Delivered { .. }))
    })
  });
  in_flight
    .or_else(|| fixes().find(|(_, other)| other.is_held() && other.written_at() < post.written_at()))
    .map(|(thread, _)| thread)
}

/// Sends every held post of the project at `root` that may go now, oldest first, and returns their runs; `slots`
/// as the runner last reported them, `lost`: the runner that served the project is gone. Blocks on SQLite.
pub fn dispatch_in(root: &Path, slots: Option<Slots>, lost: bool) -> Result<Vec<Dispatched>, FeedbackError> {
  let mut db = ProjectDb::open(root)?;
  let Some(link) = db.repository_link()? else {
    return Ok(Vec::new());
  };
  let mut by_task: BTreeMap<TaskId, Vec<FeedbackThread>> = BTreeMap::new();
  for thread in db.feedback_threads(link.repo(), None)? {
    if thread.resolved_at().is_none() {
      by_task.entry(thread.task()).or_default().push(thread);
    }
  }
  let mut sent = Vec::new();
  for (task, threads) in by_task {
    let held = threads
      .iter()
      .any(|thread| thread.posts().iter().any(FeedbackPost::is_held));
    if !held {
      continue;
    }
    let detail = TaskDetail::load_in(root, task)?;
    sent.extend(dispatch_task(&mut db, (&detail, lost), &threads, slots)?);
  }
  Ok(sent)
}

/// Tells `open`'s runner of each run `sent` put feedback in, starting the runner when it has none. A run the
/// runner does not take keeps its messages queued in the task's conversation, which says why.
pub async fn tell_runner(runners: &Runners, servers: &Servers, open: &OpenProject, sent: &[Dispatched]) {
  for dispatched in sent {
    let told = tell(
      (runners, servers, RunnerOf::Project(open)),
      (&open.repo_root, dispatched.task, dispatched.run),
    )
    .await;
    if let Sent::Queued(why) = told {
      tracing::info!(?why, run = %dispatched.run, "the runner did not take the run's feedback yet");
    }
  }
}

/// Sends the held posts of `threads`, all of `detail`'s task, that may go now.
fn dispatch_task(
  db: &mut ProjectDb,
  (detail, lost): (&TaskDetail, bool),
  threads: &[FeedbackThread],
  slots: Option<Slots>,
) -> Result<Option<Dispatched>, FeedbackError> {
  let Some(run) = detail.run.as_ref() else {
    return Ok(None);
  };
  let conversation = conversation_of(detail, detail.lost_its_runner(!lost));
  let mut messages = detail.messages.clone();
  let mut threads: Vec<FeedbackThread> = threads.to_vec();
  let mut held: Vec<(usize, FeedbackPost)> = threads
    .iter()
    .enumerate()
    .flat_map(|(at, thread)| {
      thread
        .posts()
        .iter()
        .filter(|post| post.is_held())
        .map(move |post| (at, post.clone()))
    })
    .collect();
  held.sort_by_key(|(_, post)| post.written_at());
  let mut any = false;
  for (at, post) in held {
    let task = TaskState {
      conversation: &conversation,
      run: Some(run),
      messages: &messages,
      slots,
    };
    let others: Vec<&FeedbackThread> = threads.iter().collect();
    if hold_of((&threads[at], &post), &task, &others).is_some() {
      continue;
    }
    let text = request_text(&threads[at], &post, moved(db, &threads[at], &post)?);
    let body = MessageBody::new(&text).expect("a request names what it is about");
    let message = Message::from_person(
      detail.task.id(),
      Some(run.id()),
      Recipient::Agent(threads[at].recipient()),
      body,
      Timestamp::now(),
    );
    if db.send_feedback_post(post.id(), &message)? {
      threads[at]
        .sent(post.id(), message.id())
        .expect("a post held in storage is held here");
      messages.push(message);
      any = true;
    }
  }
  Ok(any.then(|| Dispatched {
    task: detail.task.id(),
    run: run.id(),
  }))
}

/// Where `thread`'s lines are at `post`'s revision, when they moved and the pull request's diff of that revision is
/// stored.
fn moved(
  db: &ProjectDb,
  thread: &FeedbackThread,
  post: &FeedbackPost,
) -> Result<Option<orchester_core::pull_request::feedback::LineSpan>, FeedbackError> {
  let Some(lines) = thread.anchor().lines() else {
    return Ok(None);
  };
  if post.revision() == thread.revision() {
    return Ok(None);
  }
  let Some(stored) = db.latest_pull_request_diff(thread.repo(), thread.number())? else {
    return Ok(None);
  };
  if stored.head != *post.revision() {
    return Ok(None);
  }
  let Ok(diff) = Diff::parse(&stored.text) else {
    return Ok(None);
  };
  let file = diff.files().iter().find(|file| file.path() == lines.path());
  Ok(match locate(lines, file) {
    Located::At(span) => Some(span),
    Located::Outdated => None,
  })
}
