//! A team action as the command its server takes: the server's ids of its targets, the revisions it was decided on and
//! its typed inputs, checked here without any effect. A change of something whose owner is an open decision (a task's
//! pipeline, limits or workflow status) is refused before it is sent, never left out silently.

use orchester_api::{
  Blocking, ModuleFields, TaskContent,
  v2::{DecisionId, ExpectedRevision, ExpectedRevisions, ModuleField, RevisionTarget, Subject, TaskField, TeamCommand},
};
use orchester_core::{
  module::ModuleDetails,
  task::{
    TaskDetails, TaskLimits,
    organization::{TaskOrganization, WorkflowStatus},
  },
};
use orchester_store::ProjectDb;

use super::{
  ActionError,
  action::{Action, AssignmentAction, ClaimAction, DeliveryAction, ModuleAction, PolicyAction, TagAction, TaskAction},
  ownership::{TeamLink, team_authority, team_subject, team_task},
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

  fn revised(command: TeamCommand, target: RevisionTarget, revision: u64) -> Self {
    // Task, module and policy revisions are 1..=i64::MAX by their own types, an expected revision's range.
    let expected = ExpectedRevision::new(target, revision).expect("a revision is in an expected revision's range");
    Self {
      command,
      expected: ExpectedRevisions::new(vec![expected]).expect("one expected revision names its record once"),
    }
  }
}

/// The command of `action` on the team project `link`, with the server's ids of its targets.
pub(super) fn team_request(db: &ProjectDb, link: TeamLink, action: &Action) -> Result<TeamRequest, ActionError> {
  match action {
    Action::Task(action) => task(db, link, action),
    Action::Module(action) => module(action),
    Action::Tag(action) => Ok(tag(action)),
    Action::Claim(action) => claim(db, link, action),
    Action::Assignment(action) => assignment(db, link, *action),
    Action::Policy(action) => Ok(policy(link, *action)),
    Action::Delivery(action) => Ok(delivery(action)),
  }
}

fn task(db: &ProjectDb, link: TeamLink, action: &TaskAction) -> Result<TeamRequest, ActionError> {
  let (task, read, changes) = match action {
    TaskAction::Create { details, organization } => {
      return Ok(TeamRequest::unrevised(TeamCommand::TaskCreate {
        content: content(details, organization)?,
      }));
    }
    TaskAction::Edit {
      task,
      read,
      base,
      edited,
    } => (task, read, detail_changes(base, edited)?),
    TaskAction::Organize {
      task,
      read,
      base,
      edited,
    } => (task, read, organization_changes(base, edited)?),
    TaskAction::Move { task, read, module } => (task, read, vec![TaskField::Module(*module)]),
  };
  let task_id = team_task(db, link, *task)?;
  let command = TeamCommand::TaskPatch { task_id, changes };
  Ok(TeamRequest::revised(command, RevisionTarget::Task(task_id), read.get()))
}

/// A new team task's content. Its pipeline and limits are configuration whose owner is open (D6), and a new task
/// starts in the default workflow status (D4).
fn content(details: &TaskDetails, organization: &TaskOrganization) -> Result<TaskContent, ActionError> {
  ensure_owned_details(details.pipeline.is_some() || details.limits != TaskLimits::NONE)?;
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
  };
  content.set_organization(organization);
  Ok(content)
}

fn ensure_owned_details(configuration_changed: bool) -> Result<(), ActionError> {
  if configuration_changed {
    return Err(ActionError::DecisionRequired(DecisionId::D6));
  }
  Ok(())
}

fn detail_changes(base: &TaskDetails, edited: &TaskDetails) -> Result<Vec<TaskField>, ActionError> {
  ensure_owned_details(base.pipeline != edited.pipeline || base.limits != edited.limits)?;
  let mut changes = Vec::new();
  if base.title != edited.title {
    changes.push(TaskField::Title(edited.title.as_str().to_owned()));
  }
  if base.brief != edited.brief {
    changes.push(TaskField::Brief(edited.brief.clone()));
  }
  if base.acceptance_criteria != edited.acceptance_criteria {
    changes.push(TaskField::Criteria(edited.acceptance_criteria.clone()));
  }
  some(changes)
}

/// The organization fields that changed; the workflow status is a lifecycle transition, never a patch (D4).
fn organization_changes(base: &TaskOrganization, edited: &TaskOrganization) -> Result<Vec<TaskField>, ActionError> {
  if base.status() != edited.status() {
    return Err(ActionError::DecisionRequired(DecisionId::D4));
  }
  let mut changes = Vec::new();
  if base.priority() != edited.priority() {
    changes.push(TaskField::Priority(edited.priority()));
  }
  if base.tags() != edited.tags() {
    changes.push(TaskField::Tags(edited.tags().to_vec()));
  }
  if base.dependencies() != edited.dependencies() {
    let blocking = edited
      .dependencies()
      .iter()
      .map(|link| Blocking {
        task: link.task,
        gate: link.gate,
      })
      .collect();
    changes.push(TaskField::Blocking(blocking));
  }
  if base.blocked_reason() != edited.blocked_reason() {
    changes.push(TaskField::BlockedReason(edited.blocked_reason().map(str::to_owned)));
  }
  some(changes)
}

fn some<T>(changes: Vec<T>) -> Result<Vec<T>, ActionError> {
  if changes.is_empty() {
    return Err(ActionError::NothingToChange);
  }
  Ok(changes)
}

fn module(action: &ModuleAction) -> Result<TeamRequest, ActionError> {
  let (module_id, read, command) = match action {
    ModuleAction::Create { details } => {
      return Ok(TeamRequest::unrevised(TeamCommand::ModuleCreate {
        content: ModuleFields::from_core(details),
      }));
    }
    ModuleAction::Edit {
      module,
      read,
      base,
      edited,
    } => {
      let changes = module_changes(base, edited)?;
      (
        *module,
        read,
        TeamCommand::ModulePatch {
          module_id: *module,
          changes,
        },
      )
    }
    ModuleAction::AssignBranch { module, read, branch } => (
      *module,
      read,
      TeamCommand::ModuleBranch {
        module_id: *module,
        branch: branch.clone(),
      },
    ),
  };
  Ok(TeamRequest::revised(
    command,
    RevisionTarget::Module(module_id),
    read.get(),
  ))
}

fn module_changes(base: &ModuleDetails, edited: &ModuleDetails) -> Result<Vec<ModuleField>, ActionError> {
  let (base, edited) = (ModuleFields::from_core(base), ModuleFields::from_core(edited));
  let mut changes = Vec::new();
  if base.name != edited.name {
    changes.push(ModuleField::Name(edited.name));
  }
  if base.description != edited.description {
    changes.push(ModuleField::Description(edited.description));
  }
  if base.start != edited.start {
    changes.push(ModuleField::Start(edited.start));
  }
  if base.target != edited.target {
    changes.push(ModuleField::Target(edited.target));
  }
  if base.state != edited.state {
    changes.push(ModuleField::State(edited.state));
  }
  some(changes)
}

fn tag(action: &TagAction) -> TeamRequest {
  TeamRequest::unrevised(match action {
    TagAction::Create { name } => TeamCommand::TagCreate {
      name: name.as_str().to_owned(),
    },
    TagAction::Rename { tag, name } => TeamCommand::TagRename {
      tag_id: *tag,
      name: name.as_str().to_owned(),
    },
    TagAction::Merge { tag, into } => TeamCommand::TagMerge {
      tag_id: *tag,
      into: *into,
    },
    TagAction::Delete { tag } => TeamCommand::TagDelete { tag_id: *tag },
  })
}

fn claim(db: &ProjectDb, link: TeamLink, action: &ClaimAction) -> Result<TeamRequest, ActionError> {
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

fn assignment(db: &ProjectDb, link: TeamLink, action: AssignmentAction) -> Result<TeamRequest, ActionError> {
  let subject = team_subject(db, link, action.subject)?;
  let target = match subject {
    Subject::Task { id } => RevisionTarget::Task(id),
    Subject::Module { id } => RevisionTarget::Module(id),
  };
  let command = TeamCommand::AssignmentSet {
    subject,
    user_id: action.user,
  };
  Ok(TeamRequest::revised(command, target, action.read.get()))
}

/// The policy's revision is the workspace's: the expected `project` revision.
fn policy(link: TeamLink, action: PolicyAction) -> TeamRequest {
  let command = TeamCommand::PolicySet {
    automatic_merge: action.automatic_merge,
  };
  TeamRequest::revised(command, RevisionTarget::Project(link.project), action.read.get())
}

fn delivery(action: &DeliveryAction) -> TeamRequest {
  TeamRequest::unrevised(TeamCommand::DeliveryAuthorize {
    work_id: action.work,
    claim_id: action.claim,
    source_commit: action.source_commit.clone(),
    base_commit: action.base_commit.clone(),
    bundle: action.bundle.clone(),
  })
}
