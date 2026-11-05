//! A team action the team server takes as a command (`POST …/commands`, with a receipt): task creation, claims,
//! assignment, a module's branch, work start and a module's delivery authorized or requested, with the server's ids
//! of its targets and the revisions it was decided on, checked here without any effect. A new task's pipeline, limits
//! and workflow status are configuration whose owner is open (D4/D6): such a task is refused before it is sent, never
//! sent without them.

use orchester_api::{
  TaskContent,
  v2::{AuthorityRef, DecisionId, ExpectedRevision, ExpectedRevisions, RevisionTarget, Subject, TeamCommand},
};
use orchester_core::{
  id::ModuleId,
  run::BranchName,
  sharing::TaskRevision,
  task::{
    TaskDetails, TaskLimits,
    organization::{TaskOrganization, WorkflowStatus},
  },
};
use orchester_store::ProjectDb;

use super::{
  ActionError,
  action::{AssignmentAction, ClaimAction, DeliveryAction, DeliveryStep, WorkAction},
  ownership::{TeamLink, team_authority, team_subject},
};

pub(super) struct TeamRequest {
  pub(super) command: TeamCommand,
  pub(super) expected: ExpectedRevisions,
}

impl TeamRequest {
  fn unrevised(command: TeamCommand) -> Self {
    Self {
      command,
      expected: ExpectedRevisions::default(),
    }
  }

  /// `command` decided on the records `read` at their revisions; each record is named once.
  fn revised(command: TeamCommand, read: &[(RevisionTarget, TaskRevision)]) -> Self {
    let expected = read
      .iter()
      // Task and module revisions are 1..=i64::MAX by their own type, an expected revision's range.
      .map(|(target, revision)| {
        ExpectedRevision::new(*target, revision.get()).expect("a revision is in an expected revision's range")
      })
      .collect();
    Self {
      command,
      expected: ExpectedRevisions::new(expected).expect("each record is read once"),
    }
  }
}

/// `task.create`: one task, in the default workflow status, without pipeline or limits.
pub(super) fn create_task(details: &TaskDetails, organization: &TaskOrganization) -> Result<TeamRequest, ActionError> {
  if details.pipeline.is_some() || details.limits != TaskLimits::NONE {
    return Err(ActionError::DecisionRequired(DecisionId::D6));
  }
  if organization.status() != WorkflowStatus::default() {
    return Err(ActionError::DecisionRequired(DecisionId::D4));
  }
  let mut content = TaskContent {
    title: details.title.as_str().to_owned(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: None,
    limits: None,
    workflow: WorkflowStatus::default(),
    priority: organization.priority(),
    tags: Vec::new(),
    blocking: Vec::new(),
    blocked_reason: None,
    module: None,
    release: None,
  };
  content.set_organization(organization);
  Ok(TeamRequest::unrevised(TeamCommand::TaskCreate { content }))
}

/// `module.branch.set`, on the module's revision.
pub(super) fn module_branch(module: ModuleId, read: TaskRevision, branch: &BranchName) -> TeamRequest {
  let command = TeamCommand::ModuleBranchSet {
    module_id: module,
    branch: branch.clone(),
  };
  TeamRequest::revised(command, &[(RevisionTarget::Module(module), read)])
}

/// `claim.*`: claims fence themselves by their ids, so no revision is expected.
pub(super) fn claim(db: &ProjectDb, link: TeamLink, action: &ClaimAction) -> Result<TeamRequest, ActionError> {
  let command = match action {
    ClaimAction::Acquire { subject } => TeamCommand::ClaimAcquire {
      subject: team_subject(db, link, *subject)?,
    },
    ClaimAction::TakeOver { subject, observed } => TeamCommand::ClaimTakeOver {
      subject: team_subject(db, link, *subject)?,
      observed_claim_id: *observed,
    },
    ClaimAction::Release { authority } => TeamCommand::ClaimRelease {
      authority: team_authority(db, link, *authority)?,
    },
  };
  Ok(TeamRequest::unrevised(command))
}

/// `assignment.set`, on the subject's revision.
pub(super) fn assignment(db: &ProjectDb, link: TeamLink, action: AssignmentAction) -> Result<TeamRequest, ActionError> {
  let subject = team_subject(db, link, action.subject)?;
  let target = match subject {
    Subject::Task { id } => RevisionTarget::Task(id),
    Subject::Module { id } => RevisionTarget::Module(id),
  };
  let command = TeamCommand::AssignmentSet {
    subject,
    user_id: action.user,
  };
  Ok(TeamRequest::revised(command, &[(target, action.read)]))
}

/// `work.start` of a task, on the task's and its module's revisions, which the server needs both of. A module's own
/// work waits for its execution bundle (D3).
pub(super) fn work(db: &ProjectDb, link: TeamLink, action: &WorkAction) -> Result<TeamRequest, ActionError> {
  let authority = team_authority(db, link, action.authority)?;
  let Subject::Task { id: task } = authority.subject else {
    return Err(ActionError::DecisionRequired(DecisionId::D3));
  };
  let command = TeamCommand::WorkStart {
    authority,
    base_commit: action.base_commit.clone(),
  };
  let read = [
    (RevisionTarget::Task(task), action.read),
    (RevisionTarget::Module(action.module), action.module_read),
  ];
  Ok(TeamRequest::revised(command, &read))
}

/// `delivery.authorize` or `delivery.request` of a module's branch; a delivery is always a module's, and binds to the
/// module's claim, not to a work.
pub(super) fn delivery(action: &DeliveryAction) -> TeamRequest {
  let subject = Subject::Module { id: action.module };
  let (branch, source_commit, base_commit) = (
    action.branch.clone(),
    action.source_commit.clone(),
    action.base_commit.clone(),
  );
  TeamRequest::unrevised(match action.step {
    DeliveryStep::Authorize => TeamCommand::DeliveryAuthorize {
      subject,
      claim_id: action.claim,
      branch,
      source_commit,
      base_commit,
    },
    DeliveryStep::Request { authorization } => TeamCommand::DeliveryRequest {
      authority: AuthorityRef {
        subject,
        claim_id: action.claim,
        work_id: None,
      },
      authorization_operation_id: authorization,
      branch,
      source_commit,
      base_commit,
    },
  })
}
