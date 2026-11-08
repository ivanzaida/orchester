//! The one entry point of the actions on a project's tasks, modules, tags, claims, assignment, workspace policy, work
//! and delivery (team authority P2, `docs/architecture/team-authority-plan.md` §4, "Action architecture"). The
//! [`Dispatcher`] takes a typed [`Intent`], finds who owns its project and target from the project database
//! ([`ownership`]), checks its inputs without any effect and classifies it ([`ActionClass`]). A local project's action
//! runs as it always did ([`local`]). A team project's goes to its server by the server's own contract ([`route`]):
//! either as a command with its operation id, the revisions it was decided on and its typed inputs ([`command`]),
//! recorded before it is sent so that a lost response is looked up or sent again under the same id ([`team`]), with
//! only the local effects its receipt names ([`effects`]); or, for a definition, on that definition's admin-checked
//! route under the revision read ([`definition`]). The outcome says which happened ([`TeamOutcome`]). A refused team
//! action never becomes a local change. The domain MCP tools and the Orchestrator chat, which calls them, act on a team
//! project through it (P7, `crate::app_mcp`); the desktop's controls move onto it in P6.

mod action;
mod command;
mod definition;
mod effects;
mod error;
mod local;
mod outcome;
mod ownership;
mod route;
mod team;

use std::path::PathBuf;

use orchester_core::id::OperationId;
use orchester_store::{ProjectDb, StoreError};

pub use self::{
  action::{
    Action, ActionClass, AssignmentAction, ClaimAction, DeliveryAction, DeliveryStep, Intent, ModuleAction,
    PolicyAction, TagAction, TaskAction, WorkAction,
  },
  error::ActionError,
  outcome::{
    Accepted, ActionOutcome, Current, Definition, LocalResult, PipelineRefusal, Receipt, Refusal, TeamOutcome,
  },
};
use self::{
  ownership::{ProjectOwner, project_owner},
  route::{TeamRoute, team_route},
  team::Operations,
};
pub(crate) use self::{
  ownership::{TeamLink, team_task},
  team::retryable,
};
use crate::{sharing::Servers, tasks::writer::TaskWriter, team_reads::TeamSession};

/// The actions of the project at `root`. A team project's go to its server in `team`, the session the person is
/// signed in with; without one they are refused. Every call blocks on the project database: run it on a worker of
/// the application's runtime.
pub struct Dispatcher {
  root: PathBuf,
  servers: Servers,
  team: Option<TeamSession>,
}

impl Dispatcher {
  pub const fn new(root: PathBuf, servers: Servers, team: Option<TeamSession>) -> Self {
    Self { root, servers, team }
  }

  /// How `action` would be carried out on this project; a team-only action on a local project is
  /// [`ActionError::TeamOnly`].
  pub fn classify(&self, action: &Action) -> Result<ActionClass, ActionError> {
    let db = ProjectDb::open(&self.root)?;
    let team = matches!(project_owner(&db, self.team.as_ref())?, ProjectOwner::Team(_));
    if !team && action.team_only() {
      return Err(ActionError::TeamOnly);
    }
    Ok(action.class(team))
  }

  /// Carries out `intent`: on this computer for a local project, on the team server for a team project. Sending the
  /// same intent again is safe: a team operation recorded before is looked up and carried on, not sent anew, and a
  /// definition edit is decided on the revision read or keyed by the intent.
  pub async fn dispatch(&self, intent: &Intent) -> Result<ActionOutcome, ActionError> {
    let mut db = ProjectDb::open(&self.root)?;
    let ProjectOwner::Team(link) = project_owner(&db, self.team.as_ref())? else {
      if intent.action().team_only() {
        return Err(ActionError::TeamOnly);
      }
      drop(db);
      let mut writer = TaskWriter::open(&self.root, &self.servers)?;
      return local::run(&mut writer, intent.operation(), intent.action()).map(ActionOutcome::Local);
    };
    let session = self.session()?;
    let outcome = match team_route(&db, link, intent.operation(), intent.action())? {
      TeamRoute::Command(request) => {
        let mut operations = Operations {
          db: &mut db,
          session,
          link,
        };
        operations.dispatch(intent.operation(), request).await?
      }
      TeamRoute::Definition(edit) => {
        drop(db);
        definition::carry_out(session, intent.operation(), &edit).await?
      }
    };
    Ok(ActionOutcome::Team(outcome))
  }

  /// Carries on with the recorded team operation `operation` from where it stands, e.g. after a restart: looked up on
  /// the server and sent again only if the server has no outcome of it, or its receipt's local effects made again.
  pub async fn recover(&self, operation: OperationId) -> Result<TeamOutcome, ActionError> {
    let mut db = ProjectDb::open(&self.root)?;
    let ProjectOwner::Team(link) = project_owner(&db, self.team.as_ref())? else {
      return Err(ActionError::TeamOnly);
    };
    let record = db
      .team_operation(operation)?
      .ok_or(StoreError::OperationNotFound(operation))?;
    let mut operations = Operations {
      db: &mut db,
      session: self.session()?,
      link,
    };
    operations.resume(record).await
  }

  /// The team operations of the server project linked now whose outcome or local effects are still open, oldest
  /// first: what [`Self::recover`] carries on with.
  pub fn open_operations(&self) -> Result<Vec<OperationId>, ActionError> {
    let db = ProjectDb::open(&self.root)?;
    Ok(
      db.open_team_operations()?
        .into_iter()
        .map(|operation| operation.id)
        .collect(),
    )
  }

  pub(crate) const fn servers(&self) -> &Servers {
    &self.servers
  }

  /// The project's folder.
  pub(crate) fn root(&self) -> &std::path::Path {
    &self.root
  }

  /// The server project the project is linked to and the session of it, checked as every team action checks them; a
  /// local project is [`ActionError::TeamOnly`].
  pub(crate) fn team(&self, db: &ProjectDb) -> Result<(TeamLink, &TeamSession), ActionError> {
    match project_owner(db, self.team.as_ref())? {
      ProjectOwner::Team(link) => Ok((link, self.session()?)),
      ProjectOwner::Local => Err(ActionError::TeamOnly),
    }
  }

  fn session(&self) -> Result<&TeamSession, ActionError> {
    self.team.as_ref().ok_or(ActionError::NoTeamSession)
  }
}
