//! *Send to {agent}* (ORC-337, 12c): the answered side question becomes one ordinary message to the run's agent, the
//! question and its answer marked as coming from a side question ([`sent_on_text`]), stored as the composer stores a
//! message the run's state offers then, and followed the same way. It waits for the agent's turn like any message
//! the person sends after the turn; nothing is sent without the person's press, and a question is sent once: its
//! message's key is the question's own, so a retry stores nothing new.

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, RunId, SideQuestionId, TaskId},
  run::{
    Answerer,
    side_question::{SideState, sent_on_text},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  decisions::DecisionKind,
  messages::{self, Message},
  runner::Runners,
  sharing::Servers,
  tasks::{
    conversation::{
      Conversation,
      composer::{ComposerMode, Then},
      follow::{Readiness, RunnerOf, Sent, Stored, follow},
      outgoing::{Context, OutgoingError, Way, decision_kind_of, offer, prepare_in, store_answer_in},
    },
    detail::TaskDetail,
  },
  workspace::OpenProject,
};

/// How a side question's answer can reach the run's agent in `mode`, the composer's mode for the run now: as a
/// message after the agent's turn, or for its next turn; `None` once no agent of the run reads messages (the run
/// ended, or must be continued or run again), when *Send to {agent}* is unavailable. A question or plan that waits
/// is left: the message is an ordinary one, never an answer or a plan's changes.
pub const fn send_way(mode: &ComposerMode) -> Option<Way> {
  match mode {
    ComposerMode::Steer(working) if working.mid_turn => Some(Way::Second),
    ComposerMode::Resume { .. } => Some(Way::Second),
    ComposerMode::Steer(_)
    | ComposerMode::Queue(_)
    | ComposerMode::Answer { .. }
    | ComposerMode::PlanChanges { .. } => Some(Way::Primary),
    ComposerMode::FollowUp
    | ComposerMode::Continue
    | ComposerMode::RaiseLimit
    | ComposerMode::NotStarted
    | ComposerMode::RunAgain
    | ComposerMode::Closed => None,
  }
}

/// The decision sending a side question's answer on `detail`'s latest run takes, read as [`send_on_as`] reads it when
/// it sends (`conversation` is the run's conversation): `None` when it takes none or no agent reads a message. The
/// window reads it as *Send to {agent}* is pressed, so a client's press is refused there; the send checks it again.
pub fn send_decision(detail: &TaskDetail, conversation: &Conversation) -> Option<DecisionKind> {
  let run = detail.run.as_ref();
  let (mut mode, mut recipient) = offer(conversation, run, None);
  if let Some(context) = Context::of(&mode) {
    (mode, recipient) = offer(conversation, run, Some(context));
  }
  let way = send_way(&mode)?;
  decision_kind_of(detail, (&mode, recipient), way)
}

/// A side question's answer stored as a message, and what follows it.
pub struct SentOn {
  pub then: Then,
  pub run: RunId,
  pub key: IdempotencyKey,
  pub pipeline: String,
}

/// Stores the answer of side question `id` of `task` in the project at `root` as one ordinary message to its run's
/// agent, sent by the person, and marks it sent ([`send_on_as`]). Blocks on SQLite.
pub fn send_on_in(root: &Path, (task, id): (TaskId, SideQuestionId)) -> Result<SentOn, Message> {
  send_on_as(root, (task, id), (Answerer::Person, |_| Ok(true)))
}

/// [`send_on_in`], pressed `by`. Refused when it was not answered, was sent already, is about a run that is no longer
/// the task's latest, or no agent of the run reads messages any more. A message that would answer a free-text question
/// an agent waits on is that question's answer (`Prepared::decision_kind`): automation sends it only where `allowed`
/// says automation may decide that kind, and automation's message is recorded as automation's. Blocks on SQLite.
pub fn send_on_as(
  root: &Path,
  (task, id): (TaskId, SideQuestionId),
  (by, allowed): (Answerer, impl FnOnce(DecisionKind) -> Result<bool, StoreError>),
) -> Result<SentOn, Message> {
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let asked = db
    .side_question(id)
    .map_err(|error| messages::store_error(&error))?
    .filter(|asked| asked.task == task)
    .ok_or_else(|| Message::new("sideQuestion.errors.notFound"))?;
  let SideState::Answered { reply, sent, .. } = &asked.state else {
    return Err(Message::new("sideQuestion.errors.notAnswered"));
  };
  if sent.is_some() {
    return Err(Message::new("sideQuestion.errors.alreadySent"));
  }
  let refused = |error: OutgoingError| match error {
    OutgoingError::NotLatestRun(_) => Message::new("sideQuestion.errors.notLatestRun"),
    other => other.message(),
  };
  let mut prepared = prepare_in(root, (task, Some(asked.run)), (false, None)).map_err(refused)?;
  if let Some(context) = Context::of(&prepared.mode) {
    prepared = prepare_in(root, (task, Some(asked.run)), (false, Some(context))).map_err(refused)?;
  }
  let way = send_way(&prepared.mode).ok_or_else(|| Message::new("sideQuestion.errors.runEnded"))?;
  if let Some(kind) = prepared.decision_kind(way).filter(|_| by == Answerer::Automation)
    && !allowed(kind).map_err(|error| messages::store_error(&error))?
  {
    return Err(Message::new("appMcp.errors.automationNotAllowed"));
  }
  let key = IdempotencyKey::from_uuid(id.as_uuid());
  let text = sent_on_text(&asked.question, reply);
  let (then, _) = store_answer_in(root, &prepared, (key, way), (&text, &[], None), by).map_err(refused)?;
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  db.mark_side_question_sent(id, Timestamp::now())
    .map_err(|error| messages::store_error(&error))?;
  let pipeline = prepared
    .detail
    .run
    .as_ref()
    .map(|run| run.pipeline.to_string())
    .unwrap_or_default();
  Ok(SentOn {
    then,
    run: asked.run,
    key,
    pipeline,
  })
}

/// *Send to {agent}*, pressed `by`: stores the answer of side question `id` of `task` as a message ([`send_on_as`],
/// automation's under `allowed`) and lets `project`'s runner deliver it.
pub async fn send_on(
  (runners, servers): (&Runners, &Servers),
  ready: Readiness<'_>,
  project: &OpenProject,
  (task, id): (TaskId, SideQuestionId),
  (by, allowed): (
    Answerer,
    impl FnOnce(DecisionKind) -> Result<bool, StoreError> + Send + 'static,
  ),
) -> Result<Sent, Message> {
  let root = project.repo_root.clone();
  let sent = crate::background::run(move || send_on_as(&root, (task, id), (by, allowed))).await??;
  let stored = Stored {
    root: &project.repo_root,
    task,
    run: sent.run,
    key: sent.key,
    pipeline: &sent.pipeline,
  };
  Ok(
    follow(
      (runners, servers, RunnerOf::Project(project)),
      ready,
      &stored,
      sent.then,
    )
    .await,
  )
}
