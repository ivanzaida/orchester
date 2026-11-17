//! A team task is Done once its run's work is merged here (OA-30), as a local task is: the merge (into the module's
//! branch, or the base for a task in no module) records the Done (`orchester_store::MergedDone`), and this computer,
//! which holds the claim, sets the task's workflow status to Done on the team server through the same patch a person's
//! *Mark as done* sends (decision D4: the holder of the task's claim, or an Admin, may). The server then releases the
//! task's own claim. Sent after the run's reports, so the server no longer sees its work running. The server answers:
//!
//! - the task is Done: sent;
//! - the task was closed or reopened meanwhile, or the patch is refused (`not_holder` once the claim was taken over,
//!   `not_admin`, …): the server's answer stands and is kept with the run, logged and read by the person (the run's
//!   team state, `get_run`), never dropped;
//! - unreachable, or refused for now: tried again at the next round; a revision changed meanwhile is read again then,
//!   and a close refused while a report of the run still waits goes once the report did.
//!
//! The server records the Done as the holder's own, as their *Mark as done* is: a kind of its own for automation would
//! be a change of the contract.

use orchester_api::{
  ErrorCode,
  v2::{AuthorityErrorCode, TaskRead},
};
use orchester_core::{id::RunId, task::organization::WorkflowStatus, time::Timestamp};
use orchester_store::{MergedDone, WorkReportState};
use serde::Serialize;

use super::{Answer, TeamWork, TeamWorkError, answer};
use crate::actions::{Action, ActionError, ActionOutcome, Intent, LifecycleRefusal, Refusal, TaskAction, TeamOutcome};

/// What one round of sending the merged team tasks' Done did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoneSent {
  /// The runs whose task the server has Done now.
  pub sent: Vec<RunId>,
  /// The runs whose task the server did not set Done, with why: kept for the person.
  pub not_sent: Vec<(RunId, String)>,
  /// Whether a Done waits for a server that cannot be reached or refused for now.
  pub waiting: bool,
}

/// What became of one Done.
enum Answered {
  Done,
  NotSent(String),
  /// Sent again at the next round.
  Again,
  /// The server cannot be reached: the rest wait too.
  Waits,
}

impl<R> TeamWork<R> {
  /// Sets the task of each merged team run Done on the server, oldest merge first.
  pub async fn send_merged_done(&self) -> Result<DoneSent, TeamWorkError> {
    let mut round = DoneSent::default();
    let pending = self.db()?.pending_merged_done()?;
    for done in pending {
      match self.send_done(&done).await? {
        Answered::Done => {
          self.db()?.merged_done_sent(done.run, Timestamp::now())?;
          round.sent.push(done.run);
        }
        Answered::NotSent(reason) => {
          tracing::warn!(run = %done.run, task = %done.team_task, %reason, "the team server did not set a merged task Done");
          self.db()?.merged_done_not_sent(done.run, &reason, Timestamp::now())?;
          round.not_sent.push((done.run, reason));
        }
        Answered::Again => {}
        Answered::Waits => {
          round.waiting = true;
          break;
        }
      }
    }
    Ok(round)
  }

  async fn send_done(&self, done: &MergedDone) -> Result<Answered, TeamWorkError> {
    let (_, session) = self.dispatcher.team(&self.db()?)?;
    let read = match session.authority().task(done.team_task).await {
      Ok(read) => read,
      Err(error) => {
        return Ok(match answer(error) {
          Answer::Unreachable => Answered::Waits,
          Answer::Refused(error) => Answered::NotSent(wire_name(error.code())),
        });
      }
    };
    let action = match done_action(&read) {
      Ok(action) => action,
      Err(sent) => return Ok(sent),
    };
    let ActionOutcome::Team(outcome) = self.dispatcher.dispatch(&Intent::new(action)).await? else {
      return Err(ActionError::TeamOnly.into());
    };
    Ok(match outcome {
      TeamOutcome::Completed(_) | TeamOutcome::AcceptedPending { .. } | TeamOutcome::LocalEffectFailed { .. } => {
        Answered::Done
      }
      TeamOutcome::Rejected { retryable: true, .. } | TeamOutcome::AcceptanceUnknown { .. } => Answered::Waits,
      TeamOutcome::Rejected { refusal, .. } => self.refused(done.run, &refusal)?,
    })
  }

  /// What a refusal of the Done comes to: tried again while the task changed meanwhile, or while a report of the run
  /// that would end its work on the server still waits; kept with its code otherwise.
  fn refused(&self, run: RunId, refusal: &Refusal) -> Result<Answered, TeamWorkError> {
    let again = match refusal {
      Refusal::Route {
        code: ErrorCode::RevisionConflict,
        ..
      }
      | Refusal::Superseded => true,
      Refusal::TaskLifecycle(LifecycleRefusal::StopRunFirst) => self
        .db()?
        .work_reports(run)?
        .iter()
        .any(|report| report.state == WorkReportState::Pending),
      _ => false,
    };
    Ok(if again {
      Answered::Again
    } else {
      Answered::NotSent(refusal_name(refusal))
    })
  }
}

/// The patch that sets the task `read` shows Done on the revision read; what the Done comes to without one for a task
/// that is closed already (Done, or Canceled, which stands), or whose organization this version cannot read.
fn done_action(read: &TaskRead) -> Result<Action, Answered> {
  let task = &read.task;
  match task.content.workflow {
    WorkflowStatus::Done => return Err(Answered::Done),
    WorkflowStatus::Canceled => return Err(Answered::NotSent("canceled".to_owned())),
    WorkflowStatus::Backlog | WorkflowStatus::Todo | WorkflowStatus::InProgress | WorkflowStatus::InReview => {}
  }
  let Ok(base) = task.content.organization().core() else {
    // The server's task is not one this version can change; the person closes it.
    return Err(Answered::NotSent("organization_unreadable".to_owned()));
  };
  Ok(Action::Task(TaskAction::Organize {
    task: task.id,
    read: task.revision,
    edited: base.clone().with_status(WorkflowStatus::Done),
    base,
  }))
}

/// The refusal by the server's code for it.
fn refusal_name(refusal: &Refusal) -> String {
  match refusal {
    Refusal::Command(error) => wire_name(error.code()),
    Refusal::NeedsVersion { .. } => wire_name(AuthorityErrorCode::UnsupportedContract),
    Refusal::Route { code, .. } => wire_name(code),
    Refusal::TaskPipeline(_) => wire_name(ErrorCode::Invalid),
    Refusal::TaskLifecycle(LifecycleRefusal::NotHolder) => wire_name(ErrorCode::NotHolder),
    Refusal::TaskLifecycle(LifecycleRefusal::NotAdmin | LifecycleRefusal::NotAdminWithDetails) => {
      wire_name(ErrorCode::NotAdmin)
    }
    Refusal::TaskLifecycle(LifecycleRefusal::StopRunFirst) => "stop_run_first".to_owned(),
    Refusal::TaskLifecycle(LifecycleRefusal::MissingReason) => "missing_reason".to_owned(),
    Refusal::PolicyRevisionConflict | Refusal::Superseded => wire_name(ErrorCode::RevisionConflict),
  }
}

/// `value` by the snake_case name it serializes to.
fn wire_name(value: impl Serialize) -> String {
  match serde_json::to_value(value) {
    Ok(serde_json::Value::String(name)) => name,
    // Codes and statuses are unit variants that serialize to strings; one that did not is told as JSON.
    Ok(other) => other.to_string(),
    Err(error) => error.to_string(),
  }
}
