//! Where a feedback thread stands, from its posts and the task's conversation and run: held by Orchester and why
//! (`Hold`), in the conversation and when the agent reads it, taken into the agent's turn, answered, a fix ready to
//! verify with the commit the agent made, not delivered, resolved. A successful send never claims more than the
//! conversation says (`docs/design/orchester-workspace.md`: "a successful send alone must not claim execution").

use std::sync::Arc;

use orchester_core::{
  conversation::{AgentMessage, Delivery, Message, Sender, Undelivered},
  id::{FeedbackId, InvocationId, StepExecutionId},
  pull_request::feedback::{FeedbackKind, FeedbackPost, FeedbackThread},
  revision::Revision,
  run::Checkpoint,
  time::Timestamp,
};

use crate::tasks::conversation::{EntryState, PanelState, Queue};

/// Why Orchester holds a post instead of sending it to the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hold {
  /// Another fix request of the task changes its branch now: thread `0`. One request changes the branch at a
  /// time.
  AfterFix(FeedbackId),
  /// Every agent slot works (`working` of `slots`): a follow-up of the run waits for one.
  NoSlot { working: u32, slots: u32 },
  /// The task's run stopped at its token limit: the post goes once the person raises or removes it.
  Limit,
  /// The run stopped at a budget it shares, the project's daily budget or its group's.
  SharedLimit,
  /// The agent waits for the person's answer to its question in the conversation; a message would answer it.
  Question(Arc<str>),
  /// A plan waits for the person's decision.
  Plan(Arc<str>),
  /// No agent of the task reads messages any more, and a follow-up cannot bring one back.
  Closed(PanelState),
  /// Written, and sent at Orchester's next look at the task's run.
  Next,
}

/// Where a thread stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadState {
  /// Held by Orchester.
  Queued(Hold),
  /// In the task's conversation; when the agent reads it, as the conversation says.
  Delivered(Queue),
  /// The agent took it into its turn at `since`.
  Working {
    since: Timestamp,
  },
  /// The agent's turn that read it ended at `at`: a question answered, or a fix without a commit.
  Answered {
    at: Timestamp,
  },
  /// A fix: the agent's turn ended at `at` with commit `commit`.
  ReadyToVerify {
    at: Timestamp,
    commit: Revision,
  },
  /// The agent's CLI did not take it, or its run ended before any agent read it; it goes no further unless sent
  /// again.
  NotDelivered {
    at: Timestamp,
    why: NotDelivered,
  },
  Resolved {
    at: Timestamp,
  },
  /// Every post was withdrawn before it reached the agent.
  Withdrawn {
    at: Timestamp,
  },
}

/// Why a sent post did not reach its agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotDelivered {
  Cli(Undelivered),
  /// The run it was sent to ended, or the run's worktree is gone, before any agent read it.
  RunEnded,
}

impl ThreadState {
  /// Whether the thread counts as open: everything but resolved and withdrawn.
  pub const fn is_open(&self) -> bool {
    !matches!(self, Self::Resolved { .. } | Self::Withdrawn { .. })
  }
}

/// What happened to the current post, oldest first, as the thread's events list it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadEvent {
  /// Held from `at`, why.
  Queued {
    at: Timestamp,
    hold: Hold,
  },
  /// Put in the task's conversation at `at`.
  Delivered {
    at: Timestamp,
  },
  /// The agent took it into its turn at `at`.
  Working {
    at: Timestamp,
  },
  /// The agent answered at `at`.
  Answered {
    at: Timestamp,
  },
  /// The agent's turn ended at `at` with commit `commit`.
  Committed {
    at: Timestamp,
    commit: Revision,
  },
  NotDelivered {
    at: Timestamp,
    why: NotDelivered,
  },
}

/// A sent post's conversation message and what came of it.
pub(super) struct Outcome<'a> {
  pub message: &'a Message,
  /// The agent's reply at the end of the turn that read it.
  pub reply: Option<&'a Message>,
  /// The commit the turn that read it made: the run's first checkpoint of its execution after it was delivered.
  pub commit: Option<&'a Checkpoint>,
}

/// What the task's conversation and run say of a post.
pub(super) struct Facts<'a> {
  pub messages: &'a [Message],
  pub checkpoints: &'a [Checkpoint],
  /// The conversation's own state of each of its entries.
  pub entry: &'a dyn Fn(&Message) -> Option<EntryState>,
  /// Why a held post is held, when it is.
  pub hold: &'a dyn Fn(&FeedbackThread, &FeedbackPost) -> Hold,
}

impl Facts<'_> {
  /// The message `post` was sent as, with the reply and the commit of the turn that read it.
  pub(super) fn outcome(&self, post: &FeedbackPost) -> Option<Outcome<'_>> {
    let message = self
      .messages
      .iter()
      .find(|message| Some(message.id()) == post.message())?;
    let read = read_by(message);
    let reply = read.and_then(|(execution, invocation)| {
      self.messages.iter().find(|candidate| {
        matches!(
          candidate.sender(),
          Sender::Agent { execution: by, invocation: at, kind: AgentMessage::Reply }
            if *by == execution && *at == invocation
        )
      })
    });
    let commit = delivered_at(message).and_then(|(execution, at)| {
      self
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.execution() == execution && checkpoint.created_at() >= at)
    });
    Some(Outcome { message, reply, commit })
  }
}

/// The execution and invocation that read `message`, once one did.
fn read_by(message: &Message) -> Option<(StepExecutionId, InvocationId)> {
  match message.delivery()? {
    Delivery::Acknowledged {
      execution, invocation, ..
    } => Some((execution, invocation)),
    _ => None,
  }
}

fn delivered_at(message: &Message) -> Option<(StepExecutionId, Timestamp)> {
  match message.delivery()? {
    Delivery::Delivered { execution, at, .. }
    | Delivery::Acknowledged {
      execution,
      delivered_at: at,
      ..
    } => Some((execution, at)),
    _ => None,
  }
}

/// Where `thread` stands.
pub(super) fn state_of(thread: &FeedbackThread, facts: &Facts<'_>) -> ThreadState {
  if let Some(at) = thread.resolved_at() {
    return ThreadState::Resolved { at };
  }
  if thread.is_withdrawn() {
    let at = thread
      .posts()
      .iter()
      .filter_map(FeedbackPost::withdrawn_at)
      .max()
      .unwrap_or_else(|| thread.created_at());
    return ThreadState::Withdrawn { at };
  }
  let post = thread.current();
  let Some(outcome) = facts.outcome(post) else {
    return ThreadState::Queued((facts.hold)(thread, post));
  };
  sent_state(post.kind(), &outcome, facts)
}

/// Where a sent post stands.
fn sent_state(kind: FeedbackKind, outcome: &Outcome<'_>, facts: &Facts<'_>) -> ThreadState {
  match outcome.message.delivery() {
    Some(Delivery::Delivered { at, .. }) => ThreadState::Working { since: at },
    Some(Delivery::Acknowledged { at, .. }) => match (kind, outcome.commit) {
      (FeedbackKind::Fix, Some(commit)) => ThreadState::ReadyToVerify {
        at,
        commit: commit.revision().clone(),
      },
      _ => ThreadState::Answered { at },
    },
    Some(Delivery::NotDelivered { at, reason, .. }) => ThreadState::NotDelivered {
      at,
      why: NotDelivered::Cli(reason),
    },
    Some(Delivery::Queued) | None => match (facts.entry)(outcome.message) {
      Some(EntryState::Queued(Queue::RunEnded | Queue::WorktreeRemoved)) => ThreadState::NotDelivered {
        at: outcome.message.created_at(),
        why: NotDelivered::RunEnded,
      },
      Some(EntryState::Queued(queue)) => ThreadState::Delivered(queue),
      _ => ThreadState::Delivered(Queue::NextCheckpoint),
    },
  }
}

/// What happened to `thread`'s current post, oldest first.
pub(super) fn events_of(thread: &FeedbackThread, state: &ThreadState, facts: &Facts<'_>) -> Vec<ThreadEvent> {
  let post = thread.current();
  let Some(outcome) = facts.outcome(post) else {
    return match state {
      ThreadState::Queued(hold) => vec![ThreadEvent::Queued {
        at: post.written_at(),
        hold: hold.clone(),
      }],
      _ => Vec::new(),
    };
  };
  let mut events = Vec::new();
  let sent_at = outcome.message.created_at();
  // A post sent at once never waited: it shows no Queued event.
  if sent_at.unix_millis() - post.written_at().unix_millis() > HELD_AFTER_MILLIS {
    events.push(ThreadEvent::Queued {
      at: post.written_at(),
      hold: Hold::Next,
    });
  }
  events.push(ThreadEvent::Delivered { at: sent_at });
  if let Some((_, at)) = delivered_at(outcome.message) {
    events.push(ThreadEvent::Working { at });
  }
  match state {
    ThreadState::Answered { at } => events.push(ThreadEvent::Answered { at: *at }),
    ThreadState::ReadyToVerify { at, commit } => events.push(ThreadEvent::Committed {
      at: *at,
      commit: commit.clone(),
    }),
    ThreadState::NotDelivered { at, why } => events.push(ThreadEvent::NotDelivered { at: *at, why: *why }),
    _ => {}
  }
  events
}

/// How long after it was written a post may be sent and still count as sent at once: Orchester sends a post as it
/// stores it, within one look at the task.
const HELD_AFTER_MILLIS: i64 = 5_000;
