//! A turn Orchester started by itself compacts its conversation's session first when the session is nearly full
//! (`docs/plan/orchestrator-autonomy/OA-05-compaction-before-a-turn.md`): a conversation that leads a module takes
//! hundreds of such turns with nobody there to press *Compact*. The compaction is the person's own
//! ([`super::compact`]), run in the turn's place in the conversation before the turn's CLI starts, and kept as the same
//! event; whatever it comes to, the turn runs after it. *Stop response* pressed meanwhile ends the compaction and the
//! turn. A person's own message is never compacted under: they see the meter and decide.

use orchester_core::{
  connection::{AgentConnection, compaction::COMPACT_COMMAND, context_use::ContextUse},
  conversation::MessageBody,
  id::ChatMessageId,
  orchestrator::{CompactionState, OrchestratorSettings, TurnFailure, TurnState},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ChatTurn, ProjectDb};
use tokio::sync::{mpsc, oneshot};

use super::turn::Turn;
use crate::execution::{Scheduler, failed};

/// The share of its window from which a session is compacted before a turn Orchester started by itself: below the
/// share the meter calls near full, so the turn has room to read what it must.
pub(super) const COMPACT_FROM_PERCENT: u64 = 70;

/// A compaction kept as running, which a turn carries out before it starts: its entry, the tokens the session's
/// context held, and what the CLI is told.
pub(super) struct CompactFirst {
  pub(super) seq: i64,
  before: u64,
  command: MessageBody,
}

impl Scheduler {
  /// Keeps a compaction as running for the turn `kept` to carry out first, when the turn is one Orchester started by
  /// itself, the Orchestrator's connection and account resume its conversation's session, and that session filled
  /// [`COMPACT_FROM_PERCENT`] of its window; `None` otherwise, and for a session nothing reported the size of.
  pub(super) fn compact_first(
    &self,
    db: &mut ProjectDb,
    kept: &ChatTurn,
    settings: &OrchestratorSettings,
  ) -> Result<Option<CompactFirst>, Rejection> {
    if !kept.on_its_own || !resumes(db, kept, settings)? {
      return Ok(None);
    }
    let chat = kept.chat.id();
    let reading = db
      .chat_context(chat)
      .map_err(|error| failed("cannot read the conversation's context", &error))?;
    let Some(before) = reading.filter(nearly_full).map(|reading| reading.used()) else {
      return Ok(None);
    };
    let command = MessageBody::new(COMPACT_COMMAND).map_err(|error| failed("cannot write the compaction", &error))?;
    let seq = db
      .begin_chat_compaction(chat, Some(before), Timestamp::now())
      .map_err(|error| failed("cannot keep the compaction", &error))?;
    self.inner.notifier.send(Event::ChatChanged { chat });
    Ok(Some(CompactFirst { seq, before, command }))
  }
}

/// Whether a turn on the Orchestrator's connection and its account resumes the session of `kept`'s conversation: one
/// it does not resume starts afresh, and has nothing to compact.
fn resumes(db: &ProjectDb, kept: &ChatTurn, settings: &OrchestratorSettings) -> Result<bool, Rejection> {
  let connection = db
    .connection(settings.connection())
    .map_err(|error| failed("cannot read the Orchestrator's connection", &error))?
    .and_then(|connection| AgentConnection::try_from(connection).ok());
  // The turn's own start says what is wrong with a connection the Orchestrator cannot run on.
  let account = connection.and_then(|connection| connection.account().id(connection.provider()));
  Ok(account.is_some_and(|account| kept.chat.resumes_on(settings.connection(), account).is_some()))
}

/// Whether `reading` fills its window from [`COMPACT_FROM_PERCENT`] on.
fn nearly_full(reading: &ContextUse) -> bool {
  reading.share().is_some_and(|share| share >= COMPACT_FROM_PERCENT)
}

impl Turn {
  /// Carries out `first` in the turn's conversation, then runs the turn as [`Self::run`] does, on the conversation as
  /// the compaction left it. A stop asked for while it compacts ends the compaction's CLI and the turn, which starts
  /// none.
  pub(super) async fn run_after(
    self,
    first: CompactFirst,
    mut stop: oneshot::Receiver<()>,
    mut steers: mpsc::UnboundedReceiver<ChatMessageId>,
  ) {
    let compaction = self.compaction(first.command);
    let stopped = tokio::select! {
      () = compaction.compact(first.seq, Some(first.before)) => false,
      // Dropping the compaction ends its CLI. A sender that went without asking is taken as a stop too: the receiver
      // is spent, and the turn could not be stopped any more.
      _ = &mut stop => true,
    };
    if !stopped {
      return self.read_again().run(stop, steers).await;
    }
    steers.close();
    self.release_steering();
    self.record(|db| {
      let cut_short = CompactionState::Failed {
        reason: TurnFailure::Interrupted,
      };
      db.end_chat_compaction(first.seq, cut_short, None)?;
      db.end_chat_turn(self.message, &TurnState::Stopped, Timestamp::now())
        .map(drop)
    });
  }

  /// The turn that compacts this turn's conversation with `command`: the same conversation, connection and tools.
  fn compaction(&self, command: MessageBody) -> Self {
    Self {
      root: self.root.clone(),
      project: self.project,
      notifier: self.notifier.clone(),
      computer: self.computer.clone(),
      chat: self.chat.clone(),
      // What it spends is kept under an id of its own, as a person's compaction's is.
      message: ChatMessageId::generate(),
      text: command,
      attached: Vec::new(),
      settings: self.settings.clone(),
      tools: self.tools.clone(),
      time_limit: self.time_limit,
    }
  }

  /// The turn on its conversation as it is kept now: a compaction may leave it another session to resume.
  fn read_again(self) -> Self {
    let read = ProjectDb::open(&self.root).and_then(|db| db.chat(self.chat.id()));
    match read {
      Ok(Some(chat)) => Self { chat, ..self },
      // The turn's own start says what is wrong with the conversation.
      Ok(None) => self,
      Err(error) => {
        tracing::warn!(chat = %self.chat.id(), %error, "cannot read the conversation after its compaction");
        self
      }
    }
  }
}
