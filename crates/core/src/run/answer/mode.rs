//! Answering a tool request by the run's approval mode (`docs/design/approval-modes.md`), and how each request
//! records it. Autopilot answers by what the request touches ([`RootsCheck`]): inside the run's roots it is approved at
//! once by the rule, without the reviewer and without a card; outside them (the main checkout too), a push, or a change
//! to a ref other than the run's branch comes to the person with that reason; so does, as `notAllowed`, what the
//! step's own permissions forbid ([`StepLimit`]); what Orchester could not place (a call of another MCP server's tool
//! too) is one question to the reviewer, whether it touches anything outside the roots or the step's limits, whose
//! "no" approves it. Manual approves what the profile's permissions allow, asks the reviewer about the rest, and brings
//! what the reviewer denies or is unsure about to the person with the reviewer's reason.
//!
//! The reviewer's verdict is approve or ask: an ask on Manual's question reads as a denial, on Autopilot's question
//! (where it means "yes, or not sure") as doubt, and a review that gave no verdict as doubt too.

use serde::{Deserialize, Serialize};

use super::{ApprovalSubject, DecidedBy, ToolReach};
use crate::{
  orchester_tools::SERVER,
  permission::roots::{RootsCheck, RunRoots, ToolRequest},
  run::review::{Review, ReviewHold, StepLimit},
};

/// Who answered a request by the run's approval mode, as each request records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerKind {
  /// Autopilot's rule: everything the request touches lies in the run's roots.
  Autopilot,
  /// What the profile's permissions allow: its allowed commands, file, network and MCP permissions.
  Profile,
  /// The reviewer, with its reason.
  Reviewer,
  /// A person, automation in their place, or a rule a person made.
  Person,
}

impl AnswerKind {
  /// The kind of an answer `decided_by` gave, where nothing tells it more precisely.
  pub const fn of(decided_by: DecidedBy) -> Self {
    match decided_by {
      DecidedBy::AutoApproved => Self::Autopilot,
      DecidedBy::Permissions => Self::Profile,
      DecidedBy::Reviewer => Self::Reviewer,
      DecidedBy::Person
      | DecidedBy::PersonSimilar
      | DecidedBy::PersonAlways
      | DecidedBy::PersonRun
      | DecidedBy::PersonDeny
      | DecidedBy::SimilarRule
      | DecidedBy::AlwaysAllowed
      | DecidedBy::Automation
      | DecidedBy::AutomationDeny => Self::Person,
    }
  }
}

/// How a request was answered, and the reviewer's reason when it answered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestAnswer {
  pub kind: AnswerKind,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

impl RequestAnswer {
  pub const fn by(kind: AnswerKind) -> Self {
    Self { kind, reason: None }
  }

  /// The reviewer's approval, for `reason`.
  pub fn reviewer(reason: &str) -> Self {
    Self {
      kind: AnswerKind::Reviewer,
      reason: Some(reason.to_owned()),
    }
  }
}

/// Why a request came to the person, as its card says it (`task.request.reason.*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PersonReason {
  /// It touches a path outside the worktree, its Sources and the agent's own folders.
  Outside,
  /// It is not what the profile's permissions allow, and no reviewer could approve it.
  NotAllowed,
  /// The reviewer was not sure, or gave no verdict.
  ReviewerUnsure,
  /// The reviewer denied it.
  ReviewerDenied,
  /// It pushes to a remote.
  Push,
  /// It changes a ref other than the run's branch.
  Refs,
}

impl PersonReason {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Outside => "outside",
      Self::NotAllowed => "notAllowed",
      Self::ReviewerUnsure => "reviewerUnsure",
      Self::ReviewerDenied => "reviewerDenied",
      Self::Push => "push",
      Self::Refs => "refs",
    }
  }
}

/// A request that came to the person: why, and what its card names with the reason — the path outside the roots, the
/// ref, or the reviewer's own reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameToPerson {
  pub reason: PersonReason,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub detail: Option<String>,
}

impl CameToPerson {
  pub const fn because(reason: PersonReason) -> Self {
    Self { reason, detail: None }
  }

  fn with(reason: PersonReason, detail: &str) -> Self {
    Self {
      reason,
      detail: Some(detail.to_owned()),
    }
  }

  /// Why a request a hard rule kept from the reviewer comes to the person.
  pub const fn held(hold: ReviewHold) -> Self {
    Self::because(match hold {
      ReviewHold::Push => PersonReason::Push,
      ReviewHold::OutsideFolders => PersonReason::Outside,
      ReviewHold::Secrets
      | ReviewHold::Unreadable
      | ReviewHold::Destructive
      | ReviewHold::ReadOnlyStep
      | ReviewHold::NetworkOff
      | ReviewHold::PermissionsUnknown
      | ReviewHold::TooLong
      | ReviewHold::Escalation
      | ReviewHold::ProjectProgram
      | ReviewHold::Browser => PersonReason::NotAllowed,
    })
  }
}

/// What the reviewer is asked about a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReviewQuestion {
  /// Manual's: whether to approve it ([`crate::run::review::REVIEW_INSTRUCTIONS`]).
  Request,
  /// Autopilot's: whether it touches anything outside the run's roots ([`crate::run::review::RootsQuestion`]).
  Roots,
}

/// What a mode does with a request next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModeStep {
  Approve(RequestAnswer),
  Review(ReviewQuestion),
  Person(CameToPerson),
}

/// What `subject` touches, compared with `roots`: a command by its parts, a call of Orchester's tools that reads
/// outside the worktree by the folder it reads in, any other call of Orchester's tools as inside. A call of another
/// MCP server's tool cannot be placed: its arguments may name any path, and its tool may act on a remote service.
pub fn autopilot_check(roots: &RunRoots, subject: &ApprovalSubject) -> RootsCheck {
  match subject {
    ApprovalSubject::Command(command) => roots.check(&ToolRequest::Command(command)),
    ApprovalSubject::McpTool {
      reach: ToolReach::Folder { folder, .. },
      ..
    } => roots.check(&ToolRequest::Files(std::slice::from_ref(folder))),
    ApprovalSubject::McpTool { server, .. } if server == SERVER => roots.check(&ToolRequest::McpTool { paths: &[] }),
    ApprovalSubject::McpTool { .. } => RootsCheck::Unknown,
  }
}

/// Autopilot's step for a request that `check` placed, within or beyond the step's own `limit`: what leaves the roots
/// comes to the person with why; what the step forbids as `notAllowed`; what Orchester could not place, or read in a
/// limited step, is the reviewer's question.
pub fn autopilot_step(check: &RootsCheck, limit: StepLimit) -> ModeStep {
  match (check, limit) {
    (RootsCheck::Outside { path } | RootsCheck::MainCheckout { path }, _) => {
      ModeStep::Person(CameToPerson::with(PersonReason::Outside, &path.display().to_string()))
    }
    (RootsCheck::Push, _) => ModeStep::Person(CameToPerson::because(PersonReason::Push)),
    (RootsCheck::OtherRef { name }, _) => ModeStep::Person(CameToPerson::with(PersonReason::Refs, name)),
    (_, StepLimit::Held(hold)) => ModeStep::Person(CameToPerson::held(hold)),
    (RootsCheck::Unknown, _) | (_, StepLimit::Unread) => ModeStep::Review(ReviewQuestion::Roots),
    (RootsCheck::Inside, StepLimit::Within) => ModeStep::Approve(RequestAnswer::by(AnswerKind::Autopilot)),
  }
}

/// What the reviewer's `review` of a request it was asked `question` about brings: its approval, or the person with
/// why.
pub fn after_review(question: ReviewQuestion, review: &Review) -> Result<RequestAnswer, CameToPerson> {
  match (review, question) {
    (Review::Approved { reason, .. }, _) => Ok(RequestAnswer::reviewer(reason)),
    (Review::Asked { reason, .. }, ReviewQuestion::Request) => {
      Err(CameToPerson::with(PersonReason::ReviewerDenied, reason))
    }
    (Review::Asked { reason, .. }, ReviewQuestion::Roots) => {
      Err(CameToPerson::with(PersonReason::ReviewerUnsure, reason))
    }
    (Review::Held { hold }, _) => Err(CameToPerson::held(*hold)),
    (Review::Failed { .. }, _) => Err(CameToPerson::because(PersonReason::ReviewerUnsure)),
  }
}

/// Why a request comes to the person when the project has no reviewer to ask `question`: Manual's is not what the
/// profile allows; Autopilot's could not be placed.
pub const fn without_reviewer(question: ReviewQuestion) -> CameToPerson {
  CameToPerson::because(match question {
    ReviewQuestion::Request => PersonReason::NotAllowed,
    ReviewQuestion::Roots => PersonReason::ReviewerUnsure,
  })
}

/// How one request of a run was answered and whether it came to the person, as recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerRecord {
  /// The request's id.
  pub request: String,
  /// `None` while it waits, and for a request answered before answers were recorded so.
  pub answer: Option<RequestAnswer>,
  pub came_to_person: Option<CameToPerson>,
}
