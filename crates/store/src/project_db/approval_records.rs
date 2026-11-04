//! What the runner records of agents' tool requests, as values that can be sent (`docs/architecture/remote-runners.md`,
//! rule 2): a request recorded, why it came to the person, auto mode's review of it, its answer claimed with how the
//! run's mode answered it, the rule that did and a denial's words, its delivery, and its end — interrupted, with the
//! `approval_interruptions` an approved call left uncertain. Each is the change [`ProjectDb::write_approval`] applies
//! with the statement and transaction its direct write uses, so a request is still claimed once and its state only
//! moves on from where the statement expects it.

use orchester_core::{
  id::{InvocationId, RunId},
  run::{
    ApprovalRuleId, CameToPerson, DecidedBy, DenialNote, PendingToolApproval, RequestAnswer, ToolApprovalDecision,
    review::Review,
  },
  time::Timestamp,
};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::StoreError;

/// A change to the tool requests of one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
  run: RunId,
  change: ApprovalChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum ApprovalChange {
  Requested {
    approval: Box<PendingToolApproval>,
  },
  CameToPerson {
    id: String,
    came: Option<CameToPerson>,
  },
  Reviewing {
    id: String,
    on: bool,
  },
  Reviewed {
    id: String,
    review: Review,
  },
  Claimed {
    id: String,
    decided_by: DecidedBy,
    at: Timestamp,
  },
  Answer {
    id: String,
    answer: RequestAnswer,
  },
  Rule {
    id: String,
    rule: ApprovalRuleId,
  },
  DenialNote {
    id: String,
    note: DenialNote,
  },
  Delivered {
    id: String,
    decision: ToolApprovalDecision,
  },
  Interrupted {
    invocation: InvocationId,
  },
  Ended {
    id: String,
  },
}

impl ApprovalRecord {
  const fn new(run: RunId, change: ApprovalChange) -> Self {
    Self { run, change }
  }

  /// `approval`, pending ([`ProjectDb::request_tool_approval`]).
  pub fn requested(approval: &PendingToolApproval) -> Self {
    let approval = Box::new(approval.clone());
    Self::new(approval.run, ApprovalChange::Requested { approval })
  }

  /// Why waiting request `id` of `run` came to the person, or `None` once it no longer waits for them
  /// ([`ProjectDb::record_came_to_person`]).
  pub fn came_to_person(run: RunId, id: &str, came: Option<&CameToPerson>) -> Self {
    let (id, came) = (id.to_owned(), came.cloned());
    Self::new(run, ApprovalChange::CameToPerson { id, came })
  }

  /// Waiting request `id` of `run` under auto mode's review (`on`) or no longer ([`ProjectDb::set_reviewing`]).
  pub fn reviewing(run: RunId, id: &str, on: bool) -> Self {
    let id = id.to_owned();
    Self::new(run, ApprovalChange::Reviewing { id, on })
  }

  /// What auto mode did with waiting request `id` of `run` ([`ProjectDb::record_review`]).
  pub fn reviewed(run: RunId, id: &str, review: &Review) -> Self {
    let (id, review) = (id.to_owned(), review.clone());
    Self::new(run, ApprovalChange::Reviewed { id, review })
  }

  /// Request `id` of `run` taken from the waiting as `decided_by` answers it at `at`
  /// ([`ProjectDb::claim_tool_approval`]).
  pub fn claimed(run: RunId, id: &str, decided_by: DecidedBy, at: Timestamp) -> Self {
    let id = id.to_owned();
    Self::new(run, ApprovalChange::Claimed { id, decided_by, at })
  }

  /// How the run's mode answered request `id` of `run` ([`ProjectDb::record_request_answer`]).
  pub fn answer(run: RunId, id: &str, answer: &RequestAnswer) -> Self {
    let (id, answer) = (id.to_owned(), answer.clone());
    Self::new(run, ApprovalChange::Answer { id, answer })
  }

  /// Rule `rule` of `run` answered request `id` ([`ProjectDb::rule_answered`]).
  pub fn rule(run: RunId, id: &str, rule: ApprovalRuleId) -> Self {
    let id = id.to_owned();
    Self::new(run, ApprovalChange::Rule { id, rule })
  }

  /// The words a denial of request `id` of `run` told the agent ([`ProjectDb::record_denial_note`]).
  pub fn denial_note(run: RunId, id: &str, note: &DenialNote) -> Self {
    let (id, note) = (id.to_owned(), note.clone());
    Self::new(run, ApprovalChange::DenialNote { id, note })
  }

  /// The claimed answer `decision` to request `id` of `run` reached its agent ([`ProjectDb::delivered_tool_approval`]).
  pub fn delivered(run: RunId, id: &str, decision: ToolApprovalDecision) -> Self {
    let id = id.to_owned();
    Self::new(run, ApprovalChange::Delivered { id, decision })
  }

  /// The agent of `invocation` in `run` is gone: its requests not yet delivered are interrupted
  /// ([`ProjectDb::interrupt_tool_approvals`]).
  pub const fn interrupted(run: RunId, invocation: InvocationId) -> Self {
    Self::new(run, ApprovalChange::Interrupted { invocation })
  }

  /// Request `id` of `run`, ended if it still waits ([`ProjectDb::end_waiting_tool_approval`]).
  pub fn ended(run: RunId, id: &str) -> Self {
    let id = id.to_owned();
    Self::new(run, ApprovalChange::Ended { id })
  }
}

impl ProjectDb {
  /// Applies `record` as its direct write does: whether it changed the requests it names, which a claim, a delivery
  /// or an end of a request no longer where its statement expects it does not. The other changes always do.
  pub fn write_approval(&mut self, record: &ApprovalRecord) -> Result<bool, StoreError> {
    let run = record.run;
    let always = |()| true;
    match &record.change {
      ApprovalChange::Requested { approval } => self.request_tool_approval(approval).map(always),
      ApprovalChange::CameToPerson { id, came } => self.record_came_to_person(run, id, came.as_ref()).map(always),
      ApprovalChange::Reviewing { id, on } => self.set_reviewing(run, id, *on).map(always),
      ApprovalChange::Reviewed { id, review } => self.record_review(run, id, review).map(always),
      ApprovalChange::Claimed { id, decided_by, at } => self.claim_tool_approval(run, id, *decided_by, *at),
      ApprovalChange::Answer { id, answer } => self.record_request_answer(run, id, answer).map(always),
      ApprovalChange::Rule { id, rule } => self.rule_answered(run, *rule, id).map(always),
      ApprovalChange::DenialNote { id, note } => self.record_denial_note(run, id, note).map(always),
      ApprovalChange::Delivered { id, decision } => self.delivered_tool_approval(run, id, *decision),
      ApprovalChange::Interrupted { invocation } => self.interrupt_tool_approvals(run, *invocation),
      ApprovalChange::Ended { id } => self.end_waiting_tool_approval(run, id),
    }
  }
}
