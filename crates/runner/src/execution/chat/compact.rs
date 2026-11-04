//! A person's *Compact* of a conversation's session (ORC-309, `docs/design/context-meter.md`): the CLI the Orchestrator
//! runs on summarizes the session it resumes and goes on from the summary. Not while a turn of the conversation is
//! under way: the desktop offers it between turns, and the runner refuses it then ([`Rejection::ChatBusy`]); a message
//! waits until it ended, as a compaction holds the conversation as a turn does. It is kept as an event of the
//! conversation, running, then done with the size after it, or failed with why; what it spent counts toward the daily
//! budget as a turn's does.
//!
//! - Claude Code: a turn of its own whose prompt is `/compact` ([`ClaudeChat::compaction_command`]). It is done once
//!   the CLI printed its compaction boundary; Claude Code reports no size after it, so the session's reading is
//!   forgotten until the next turn reports one, unless a message of the turn reported one lower than before
//!   ([`compacted_to`]): the compaction's own request, which reports the size before, is never taken as after.
//! - Codex: App Server compacts the thread it resumes (`thread/compact/start`) in a turn of its own, and reports the
//!   thread's size after it.

use std::{sync::Arc, time::Duration};

use orchester_core::{
  command::redact_secrets,
  connection::{
    AgentProvider,
    app_server::Update,
    compaction::{COMPACT_COMMAND, claude_boundary, compacted_to},
    context_use::ContextUse,
    stream::{AgentEvent, ClaudeStream},
  },
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId},
  orchestrator::{CompactionState, TurnFailure},
  time::Timestamp,
};
use orchester_ipc::{ChatTools, Event, Rejection};

use super::{
  Running, Work,
  codex::turn_failure,
  turn::{Prepared, Seen, Turn, cannot},
};
use crate::{
  execution::{Scheduler, failed, idle::check_daily_budget, step::app_server::AppServerSession},
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

/// How a compaction ended, and the session's reading after it.
type Compacted = (CompactionState, Option<ContextUse>);

impl Scheduler {
  /// Compacts the session of conversation `chat` on the Orchestrator's connection, with the domain tools at `tools`.
  pub async fn compact_chat(&self, chat: ChatId, tools: ChatTools) -> Result<(), Rejection> {
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    let mut db = self.open()?;
    let kept = db
      .chat(chat)
      .map_err(|error| failed("cannot read the conversation", &error))?
      .ok_or(Rejection::NothingToCompact)?;
    let settings = db
      .orchestrator_settings()
      .map_err(|error| failed("cannot read the Orchestrator's settings", &error))?
      .ok_or(Rejection::NoOrchestrator)?;
    if kept
      .session()
      .is_none_or(|session| session.connection != settings.connection())
    {
      return Err(Rejection::NothingToCompact);
    }
    check_daily_budget(&db)?;
    let earlier = db
      .chat_context(chat)
      .map_err(|error| failed("cannot read the conversation's context", &error))?;
    let mut running = self.inner.chats.running.lock();
    running.retain(|_, work| !work.task.is_finished());
    if running.contains_key(&chat) {
      return Err(Rejection::ChatBusy);
    }
    let before = earlier.map(|reading| reading.used());
    let seq = db
      .begin_chat_compaction(chat, before, Timestamp::now())
      .map_err(|error| failed("cannot keep the compaction", &error))?;
    self.inner.notifier.send(Event::ChatChanged { chat });
    let turn = Turn {
      root: self.inner.project_root.clone(),
      project: db.project().id(),
      notifier: self.inner.notifier.clone(),
      vault: Arc::clone(&self.inner.vault),
      chat: kept,
      // What it spends is kept under an id of its own, as a turn's under its message.
      message: ChatMessageId::generate(),
      text: MessageBody::new(COMPACT_COMMAND).map_err(|error| failed("cannot write the compaction", &error))?,
      attached: Vec::new(),
      settings,
      tools,
      time_limit: self.inner.chats.time_limit(),
    };
    let task = tokio::spawn(turn.compact(seq, before));
    running.insert(
      chat,
      Running {
        work: Work::Compaction(seq),
        stop: None,
        steers: None,
        task,
      },
    );
    Ok(())
  }
}

impl Turn {
  /// Compacts the session, its context holding `before` tokens, and records how compaction `seq` ended.
  async fn compact(self, seq: i64, before: Option<u64>) {
    let (state, reading) = match self.prepare(true) {
      Ok(prepared) if prepared.resumed.is_none() => (
        CompactionState::Failed {
          reason: TurnFailure::Failed {
            reason: "the conversation has no session the Orchestrator's account can resume".to_owned(),
          },
        },
        None,
      ),
      Ok(prepared) if prepared.connection.provider() == AgentProvider::CodexCli => {
        self.compact_codex(prepared, before).await
      }
      Ok(prepared) => self.compact_claude(&prepared, before).await,
      Err(reason) => (CompactionState::Failed { reason }, None),
    };
    if let CompactionState::Failed { reason } = &state {
      tracing::warn!(chat = %self.chat.id(), ?reason, "the Orchestrator's session was not compacted");
    }
    self.record(|db| db.end_chat_compaction(seq, state, reading.as_ref()).map(drop));
  }

  /// Keeps what a compaction's CLI reported that a compaction keeps: the session, what it spent and how it failed;
  /// never a reply, as the conversation says nothing new.
  fn keep_compacting(&self, event: AgentEvent, prepared: &Prepared, seen: &mut Seen) {
    match event {
      AgentEvent::SessionStarted { .. }
      | AgentEvent::TurnCompleted { .. }
      | AgentEvent::NotSignedIn { .. }
      | AgentEvent::TurnFailed { .. }
      | AgentEvent::Error { .. }
      | AgentEvent::Raw { .. } => self.keep(event, prepared.account, seen),
      _ => {}
    }
  }

  /// Claude Code's `/compact` in the session it resumes.
  async fn compact_claude(&self, prepared: &Prepared, before: Option<u64>) -> Compacted {
    // Its files live as long as the CLI.
    let started = match self.start_claude(prepared, true) {
      Ok(started) => started,
      Err(reason) => return failed_with(reason),
    };
    let mut process = match Supervised::start(&started.command) {
      Ok(process) => process,
      Err(error) => return failed_with(cannot(error.to_string())),
    };
    let mut seen = Seen::default();
    let mut stream = ClaudeStream::chat();
    let mut boundary = false;
    let limit = tokio::time::sleep(self.time_limit);
    tokio::pin!(limit);
    let exit = loop {
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { stream: from, line }) => {
            boundary |= claude_boundary(&line).is_some();
            for mut event in stream.parse(from, &line) {
              event.redact(&|text| redact_secrets(text, &started.secrets));
              self.keep_compacting(event, prepared, &mut seen);
            }
            if stream.results() > 0 {
              process.close_input();
            }
          }
          Some(ProcessEvent::Exited(exit)) => break exit,
          None => break ProcessExit::WaitFailed { reason: "the CLI's output ended".to_owned() },
        },
        () = &mut limit, if seen.timed_out.is_none() => {
          seen.timed_out = Some(TurnFailure::TimeLimit { minutes: self.time_limit.as_secs() / 60 });
          process.terminate();
        }
      }
    };
    let reading = stream
      .context()
      .filter(|reading| compacted_to(before, None, Some(*reading)).is_some());
    ended(
      seen,
      boundary,
      &exit,
      reading,
      "Claude Code ended without compacting the session",
    )
  }

  /// Codex App Server's `thread/compact/start` on the thread it resumes.
  async fn compact_codex(&self, prepared: Prepared, before: Option<u64>) -> Compacted {
    let mut started = match self.start_codex(prepared, true).await {
      Ok(started) => started,
      Err(reason) => return failed_with(reason),
    };
    started.setup.compact = true;
    let account = started.account;
    let mut process = match Supervised::start(&started.command) {
      Ok(process) => process,
      Err(error) => return failed_with(cannot(error.to_string())),
    };
    let mut session = AppServerSession::new(started.setup);
    if let Err(error) = session.start(&process).await {
      return failed_with(cannot(format!("cannot initialize Codex App Server: {error}")));
    }
    let mut seen = Seen::default();
    let mut completed = false;
    let mut beat = tokio::time::interval(Duration::from_millis(100));
    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let limit = tokio::time::sleep(self.time_limit);
    tokio::pin!(limit);
    let exit = loop {
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { .. }) if seen.finished => {}
          Some(ProcessEvent::Output { stream, line }) => {
            let over = match session.receive(stream, &line, &process).await {
              Ok(updates) => self.compaction_updates(updates, account, (&mut seen, &mut completed)),
              Err(failure) => {
                seen.failure.get_or_insert(turn_failure(failure));
                true
              }
            };
            if over {
              seen.finished = true;
              process.terminate();
            }
          }
          Some(ProcessEvent::Exited(exit)) => break exit,
          None => break ProcessExit::WaitFailed { reason: "the CLI's output ended".to_owned() },
        },
        // App Server's MCP servers are checked as for a turn before it compacts.
        _ = beat.tick(), if !seen.finished && seen.timed_out.is_none() => {
          if let Err(failure) = session.poll_servers(&process).await {
            seen.failure.get_or_insert(turn_failure(failure));
            seen.finished = true;
            process.terminate();
          }
        }
        () = &mut limit, if seen.timed_out.is_none() && !seen.finished => {
          seen.timed_out = Some(TurnFailure::TimeLimit { minutes: self.time_limit.as_secs() / 60 });
          process.terminate();
        }
      }
    };
    let reading = session
      .context()
      .filter(|reading| compacted_to(before, None, Some(*reading)).is_some());
    ended(
      seen,
      completed,
      &exit,
      reading,
      "Codex ended without compacting the session",
    )
  }

  /// Keeps what App Server said of the compaction's turn; whether it is over. Codex asks nothing in it: a decision it
  /// asks ends the compaction as failed.
  fn compaction_updates(
    &self,
    updates: Vec<Update>,
    account: AgentAccountId,
    (seen, completed): (&mut Seen, &mut bool),
  ) -> bool {
    let mut over = false;
    for update in updates {
      match update {
        Update::Activity(event) => match event {
          AgentEvent::SessionStarted { .. }
          | AgentEvent::TurnCompleted { .. }
          | AgentEvent::TurnFailed { .. }
          | AgentEvent::Error { .. } => self.keep(event, account, seen),
          _ => {}
        },
        Update::TurnEnded { completed: done, .. } => {
          *completed = done;
          over = true;
        }
        Update::Approval(_) | Update::Declined(_) | Update::QuestionRefused { .. } | Update::Question(_) => {
          seen.failure.get_or_insert(TurnFailure::Failed {
            reason: "Codex asked a decision while it compacted the session".to_owned(),
          });
          over = true;
        }
        Update::InputTaken(_) | Update::Failure(_) => {}
      }
    }
    over
  }
}

/// How a compaction ended: as the turn of its CLI did, done once the CLI `compacted`, the session then with
/// `reading`; `unfinished` says why a CLI that ended on its own did not.
fn ended(seen: Seen, compacted: bool, exit: &ProcessExit, reading: Option<ContextUse>, unfinished: &str) -> Compacted {
  if let Some(failure) = seen.timed_out.or(seen.failure) {
    return failed_with(failure);
  }
  if compacted {
    return (
      CompactionState::Done {
        after: reading.map(|reading| reading.used()),
      },
      reading,
    );
  }
  let reason = match (seen.stderr, exit) {
    (Some(said), _) => said,
    (None, ProcessExit::WaitFailed { reason }) => reason.clone(),
    (None, _) => unfinished.to_owned(),
  };
  failed_with(TurnFailure::Failed { reason })
}

const fn failed_with(reason: TurnFailure) -> Compacted {
  (CompactionState::Failed { reason }, None)
}
