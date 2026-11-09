//! What the Orchestrator chat does not do itself (`docs/design/orchestrator-toolset.md`, decision 5). While the
//! project's switch *Let the Orchestrator drive* is on, the chat acts in every turn as a client of the app does: a
//! merge, a start, a pause, a resume, a release of new starts and a publication are carried out, not proposed; only in
//! a turn a day's report started (ORC-47), or a subscription that ended by itself to have its summary written
//! (ORC-764), the chat changes nothing at all: every change is refused (`report_turn`). While it is off, in a turn a
//! wait or a run's end started by itself ([`super::tools::ChatScope::on_its_own`]) nobody asked for anything, so a
//! merge, a start, a pause, a release of new starts and a publication are only proposed, and in a report's turn every
//! change a card carries is proposed and any other is refused; the changes a person alone applies are always cards
//! ([`needs_person`]). A run stopped at a limit, canceled, or interrupted with its worktree gone or by the runner
//! failing to record it for any reason but the database staying busy is resumed or continued by the person only
//! (`continue_run` continues one stopped at a limit); and one a crash, the runner going away, *Quit now* or the
//! database staying busy (ORC-75) interrupted is continued from the chat only while it drives: in a turn nobody asked
//! for, only the run whose interruption started the turn, once, not when it was interrupted again soon after the time
//! before, and not while the database is under contention.

use orchester_core::{
  id::RunId,
  orchestrator::{
    AgentWait, ChatEntry, ChatResume, Continuing, EntryKind, PersonsStop, RunEnd, WaitKind,
    change::Publishing,
    resume::{AGAIN_WITHIN, CONTENTION_BRAKE},
  },
  run::InterruptCause,
  store::WorktreeStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{
  change::control::{PAUSE_RUN, RELEASE_QUEUE, RESUME_RUN},
  repository::INIT_REPOSITORY,
  tools::{ANSWER_QUESTION, CANCEL_RUN, CONTINUE_RUN, ChatScope, MERGE_RUN, RUN_TASK, SEND_MESSAGE},
};
use crate::{
  app_mcp::{ToolError, blocking, find_run},
  messages::Message,
  tasks::changes::run_work_present,
};

/// Whether the chat's call of `name` in `scope` is only proposed as a change because nobody asked for anything in its
/// turn, while it does not `drive`; a start is then proposed as a start ([`super::tools`]), and a resume as
/// [`resume_as`] tells. In a turn a day's report started, every change a card carries is proposed (not while it
/// drives), and any other is refused (`report_turn`): a task, a draft, a copy of a pipeline or a hold of new starts,
/// and every change while it drives.
pub(super) async fn only_proposed(scope: &ChatScope, name: &str, drives: bool) -> Result<bool, ToolError> {
  if !scope.on_its_own {
    return Ok(false);
  }
  // A start is proposed by `run_task` itself in any turn nobody asked for, while the chat does not drive.
  if (name != RUN_TASK || drives) && in_report_turn(scope).await? {
    return if !drives && card_carries(name) {
      Ok(true)
    } else {
      Err(ToolError::refused(
        "report_turn",
        vec![Message::new("orchestrator.tools.reportChangesNothing")],
      ))
    };
  }
  Ok(!drives && (matches!(name, MERGE_RUN | PAUSE_RUN | RELEASE_QUEUE) || Publishing::of_tool(name).is_some()))
}

/// Whether the turn under way in `scope`'s conversation is the one a day's report, or a subscription that ended by
/// itself to have its summary written (ORC-764), started by itself: either changes nothing.
pub(super) async fn in_report_turn(scope: &ChatScope) -> Result<bool, ToolError> {
  if !scope.on_its_own {
    return Ok(false);
  }
  let (root, chat) = (scope.root.clone(), scope.chat);
  blocking(move || Ok(thinking_report(&ProjectDb::open(&root)?.chat_entries(chat)?))).await
}

/// Whether a change of the chat's tool `name` is always a card while the chat does not drive, in any turn: the person
/// applies it ([`super::change::subject`]).
pub(super) fn needs_person(name: &str) -> bool {
  matches!(
    name,
    MERGE_RUN | CANCEL_RUN | CONTINUE_RUN | ANSWER_QUESTION | SEND_MESSAGE | INIT_REPOSITORY
  ) || Publishing::of_tool(name).is_some()
}

/// Whether the person applies a change of the chat's tool `name` from a card ([`super::change::subject`]).
fn card_carries(name: &str) -> bool {
  matches!(
    name,
    MERGE_RUN
      | CANCEL_RUN
      | CONTINUE_RUN
      | ANSWER_QUESTION
      | SEND_MESSAGE
      | PAUSE_RUN
      | RESUME_RUN
      | RELEASE_QUEUE
      | INIT_REPOSITORY
  ) || Publishing::of_tool(name).is_some()
}

/// Whether the turn that thinks now among a conversation's `entries` is one that changes nothing: a day's report's,
/// or a subscription's end's.
fn thinking_report(entries: &[ChatEntry]) -> bool {
  entries
    .iter()
    .rev()
    .find(|entry| entry.kind.turn().is_some_and(|(_, state)| state.is_thinking()))
    .is_some_and(|entry| entry.kind.changes_nothing())
}

/// What the chat's `resume_run` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Resume {
  /// It proposes the resume to the person: nobody asked for it in the turn.
  Propose,
  /// It resumes the run, or continues it from its last checkpoint, now.
  Now(RunId),
}

/// What `resume_run` with `arguments` does in `scope` while the chat does or does not `drive`; refused (`person_only`)
/// for a run that is the person's to resume or continue, and in a turn nobody asked for, (`interrupted_again`) for one
/// interrupted again soon and (`storage_contention`) for one the database staying busy interrupted while it is under
/// contention.
pub(super) async fn resume_as(scope: &ChatScope, arguments: &JsonObject, drives: bool) -> Result<Resume, ToolError> {
  let named = arguments
    .get("run")
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments("run: required".to_owned()))?
    .to_owned();
  let (root, chat) = (scope.root.clone(), scope.chat);
  let read = blocking(move || {
    let db = ProjectDb::open(&root)?;
    let run = find_run(&db, &named)?;
    let present = db.worktree_removal(run.id())?.is_none() && run_work_present(&db, &root, &run);
    let started_by = thinking_wait(&db.chat_entries(chat)?);
    let resume = ChatResume::of(run.status(), db.runner_failure(run.id())?, present);
    let continuing = match resume {
      ChatResume::Interrupted(cause) => Some(continuing_of(&db, run.id(), cause)?),
      ChatResume::Paused | ChatResume::PersonOnly(_) | ChatResume::Other => None,
    };
    Ok(Read {
      run: run.id(),
      number: run.number().get(),
      resume,
      continuing,
      started_by,
    })
  })
  .await?;
  read.decide(scope.on_its_own, drives)
}

/// What a turn nobody asked for does about `run`, which `cause` interrupted and the chat may continue: once, unless
/// it was interrupted again soon; and for the database staying busy, not while more runs of the project than
/// [`CONTENTION_BRAKE`] were so within [`AGAIN_WITHIN`].
pub(super) fn continuing_of(db: &ProjectDb, run: RunId, cause: InterruptCause) -> Result<Continuing, StoreError> {
  let interruptions = db.run_interruptions(run)?;
  if cause != InterruptCause::RunnerFailed {
    return Ok(Continuing::after(&interruptions));
  }
  let window = i64::try_from(AGAIN_WITHIN.as_millis()).expect("ten minutes of milliseconds fit in i64");
  let since = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - window);
  Ok(Continuing::after_contention(
    &interruptions,
    db.contended_runs_since(since)?,
  ))
}

/// What a resume reads of its run: its id and number, what the chat may do with it, what a turn nobody asked for does
/// about it (`None` when the chat may not continue it), and the wait whose turn is the one under way in the chat's
/// conversation.
struct Read {
  run: RunId,
  number: u32,
  resume: ChatResume,
  continuing: Option<Continuing>,
  started_by: Option<AgentWait>,
}

impl Read {
  fn decide(&self, on_its_own: bool, drives: bool) -> Result<Resume, ToolError> {
    match self.resume {
      // A limit is decided with `continue_run`, which the chat takes while it drives.
      ChatResume::PersonOnly(PersonsStop::Limit(_)) if drives => {
        Err(self.refused("use_continue_run", "orchestrator.tools.resumeUseContinue"))
      }
      ChatResume::PersonOnly(stop) => Err(self.refused("person_only", stopped_key(stop))),
      ChatResume::Paused | ChatResume::Other if on_its_own && !drives => Ok(Resume::Propose),
      ChatResume::Paused | ChatResume::Other => Ok(Resume::Now(self.run)),
      ChatResume::Interrupted(_) if !drives => Err(self.refused("person_only", "orchestrator.tools.resumePersonOnly")),
      ChatResume::Interrupted(_) if !on_its_own => Ok(Resume::Now(self.run)),
      // A turn nobody asked for continues only the run whose interruption started it.
      ChatResume::Interrupted(_) if !self.started_by_its_interruption() => {
        Err(self.refused("person_only", "orchestrator.tools.resumePersonOnly"))
      }
      ChatResume::Interrupted(_) => match self.continuing {
        Some(Continuing::Continues) | None => Ok(Resume::Now(self.run)),
        Some(Continuing::InterruptedAgain) => Err(ToolError::refused(
          "interrupted_again",
          vec![
            Message::new("orchestrator.tools.continueAgain")
              .with("run", self.number.to_string())
              .with("minutes", (AGAIN_WITHIN.as_secs() / 60).to_string()),
          ],
        )),
        Some(Continuing::StorageContention) => Err(ToolError::refused(
          "storage_contention",
          vec![
            Message::new("orchestrator.tools.continueContention")
              .with("run", self.number.to_string())
              .with("runs", CONTENTION_BRAKE.to_string())
              .with("minutes", (AGAIN_WITHIN.as_secs() / 60).to_string()),
          ],
        )),
      },
    }
  }

  /// Whether the turn under way is the one this run's interruption started by itself.
  fn started_by_its_interruption(&self) -> bool {
    self
      .started_by
      .as_ref()
      .is_some_and(|wait| wait.run == self.run && wait.kind == WaitKind::Ended(RunEnd::Interrupted))
  }

  fn refused(&self, code: &str, key: &'static str) -> ToolError {
    ToolError::refused(code, vec![Message::new(key).with("run", self.number.to_string())])
  }
}

/// The words the chat's agent is told why a run stopped so is the person's.
const fn stopped_key(stop: PersonsStop) -> &'static str {
  match stop {
    PersonsStop::Limit(_) => "orchestrator.tools.resumePersonOnly",
    PersonsStop::RunnerFailed => "orchestrator.tools.continueRunnerFailed",
    PersonsStop::Canceled => "orchestrator.tools.resumeCanceled",
    PersonsStop::WorktreeGone => "orchestrator.tools.continueWorktreeGone",
  }
}

/// The wait among a conversation's `entries` whose turn thinks now: the turn under way, when a wait started it.
pub(super) fn thinking_wait(entries: &[ChatEntry]) -> Option<AgentWait> {
  entries.iter().rev().find_map(|entry| match &entry.kind {
    EntryKind::Wait { wait, turn: Some(turn) } if turn.state.is_thinking() => Some(wait.clone()),
    _ => None,
  })
}
