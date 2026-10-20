//! What the Orchestrator chat does not do itself (`docs/design/orchestrator-toolset.md`, decision 5), so that it never
//! takes up its own doing in a loop and never decides what is the person's: in a turn a wait or a run's end started by
//! itself ([`super::tools::ChatScope::on_its_own`]) nobody asked for anything, so a merge, a start, a pause, a resume,
//! a release of new starts and a publication are only proposed, whatever the person's switches and auto mode; a
//! message that moves a run none of whose agents waits or works (a follow-up, a continuation, a resume, the task run
//! again) is always proposed; a run stopped at a limit, canceled, or interrupted with its worktree gone or by the
//! runner failing to record it is resumed or continued by the person only; and one a crash, the runner going away or
//! *Quit now* interrupted is continued from the chat only under *Let the Orchestrator continue interrupted runs*: in a
//! turn nobody asked for, only the run whose interruption started the turn, once, and not when it was interrupted
//! again soon after the time before.

use orchester_core::{
  id::RunId,
  orchestrator::{
    AgentWait, ChatEntry, ChatResume, Continuing, EntryKind, PersonsStop, RunEnd, WaitKind, change::Publishing,
    resume::AGAIN_WITHIN,
  },
  store::WorktreeStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{
  change::control::{PAUSE_RUN, RELEASE_QUEUE},
  tools::{ChatScope, MERGE_RUN},
};
use crate::{
  app_mcp::{ToolError, blocking, find_run},
  messages::Message,
  tasks::changes::run_worktree,
};

/// Whether the chat's call of `name` in `scope` is only proposed as a change because nobody asked for anything in its
/// turn; a start is then proposed as a start ([`super::tools`]), and a resume as [`resume_as`] tells.
pub(super) fn proposed_on_its_own(scope: &ChatScope, name: &str) -> bool {
  scope.on_its_own && (matches!(name, MERGE_RUN | PAUSE_RUN | RELEASE_QUEUE) || Publishing::of_tool(name).is_some())
}

/// What the chat's `resume_run` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Resume {
  /// It proposes the resume to the person: nobody asked for it in the turn.
  Propose,
  /// It resumes the run, or continues it from its last checkpoint, now.
  Now(RunId),
}

/// The person's switches a resume reads: *Let the Orchestrator continue interrupted runs* and *answer agents*.
#[derive(Clone, Copy, Debug)]
pub(super) struct ResumeSwitches {
  pub(super) continues: bool,
  pub(super) answers: bool,
}

/// What `resume_run` with `arguments` does in `scope` under `switches`; refused (`person_only`) for a run that is the
/// person's to resume or continue, and (`interrupted_again`) for one interrupted again soon in a turn nobody asked for.
pub(super) async fn resume_as(
  scope: &ChatScope,
  arguments: &JsonObject,
  switches: ResumeSwitches,
) -> Result<Resume, ToolError> {
  let named = arguments
    .get("run")
    .and_then(Value::as_str)
    .ok_or_else(|| ToolError::InvalidArguments("run: required".to_owned()))?
    .to_owned();
  let (root, chat) = (scope.root.clone(), scope.chat);
  let read = blocking(move || {
    let db = ProjectDb::open(&root)?;
    let run = find_run(&db, &named)?;
    let present = db.worktree_removal(run.id())?.is_none() && run_worktree(&root, run.id()).is_dir();
    let started_by = thinking_wait(&db.chat_entries(chat)?);
    Ok(Read {
      run: run.id(),
      number: run.number().get(),
      resume: ChatResume::of(run.status(), present),
      interruptions: db.run_interruptions(run.id())?,
      started_by,
    })
  })
  .await?;
  read.decide(scope.on_its_own, switches)
}

/// What a resume reads of its run: its id and number, what the chat may do with it, when it was interrupted, and the
/// wait whose turn is the one under way in the chat's conversation.
struct Read {
  run: RunId,
  number: u32,
  resume: ChatResume,
  interruptions: Vec<Timestamp>,
  started_by: Option<AgentWait>,
}

impl Read {
  fn decide(&self, on_its_own: bool, switches: ResumeSwitches) -> Result<Resume, ToolError> {
    match self.resume {
      ChatResume::PersonOnly(stop) => Err(self.refused("person_only", stopped_key(stop))),
      ChatResume::Paused | ChatResume::Other if on_its_own => Ok(Resume::Propose),
      ChatResume::Paused | ChatResume::Other => Ok(Resume::Now(self.run)),
      ChatResume::Interrupted(_) if !switches.continues => {
        Err(self.refused("person_only", "orchestrator.tools.resumePersonOnly"))
      }
      ChatResume::Interrupted(_) if !on_its_own => Ok(Resume::Now(self.run)),
      // A turn nobody asked for continues only the run whose interruption started it.
      ChatResume::Interrupted(_) if !switches.answers || !self.started_by_its_interruption() => {
        Err(self.refused("person_only", "orchestrator.tools.resumePersonOnly"))
      }
      ChatResume::Interrupted(_) => match Continuing::after(&self.interruptions) {
        Continuing::Continues => Ok(Resume::Now(self.run)),
        Continuing::InterruptedAgain => Err(ToolError::refused(
          "interrupted_again",
          vec![
            Message::new("orchestrator.tools.continueAgain")
              .with("run", self.number.to_string())
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
fn thinking_wait(entries: &[ChatEntry]) -> Option<AgentWait> {
  entries.iter().rev().find_map(|entry| match &entry.kind {
    EntryKind::Wait { wait, turn: Some(turn) } if turn.state.is_thinking() => Some(wait.clone()),
    _ => None,
  })
}
