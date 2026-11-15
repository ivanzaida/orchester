//! What uses an account, a sign-in or an API account, across the listed projects (*Used by*), and moving connections
//! off a removed sign-in. Each
//! project's database is opened in turn; a project whose folder is gone is passed over, and its connections follow
//! the removal when it opens again ([`follow_removals`]).

use std::{collections::BTreeSet, path::Path, sync::Arc};

use orchester_core::{
  account::{AgentAccount, ApiAccount},
  connection::{AgentConnection, Connection, ConnectionAccount, ConnectionKind},
  id::{AgentAccountId, ConnectionId, ProjectId, ResourceId},
  name::Name,
  resource::AgentProfile,
  run::StepExecutor,
  store::{ConnectionStore, ResourceStore, TaskStore},
};
use orchester_store::{ConnectionPick, ProjectDb, RegisteredProject, StoreError};

use super::{AccountsError, api_runs_on::on_account};
use crate::agents::data::profiles;

/// A connection that runs on the account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionUse {
  pub project: ProjectId,
  pub project_name: Name,
  pub connection: ConnectionId,
  pub connection_name: Name,
  /// The roles of the agent profiles that run on the connection.
  pub profiles: Vec<Arc<str>>,
}

/// An agent profile that runs on the account on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileUse {
  pub project: ProjectId,
  pub project_name: Name,
  pub profile: ResourceId,
  /// The profile's role; `None` for a team's profile, whose name the team's configuration holds, not the project's
  /// database.
  pub role: Option<Arc<str>>,
}

/// The account whose uses are looked for.
#[derive(Clone, Copy)]
pub(super) enum On<'a> {
  SignIn(&'a AgentAccount),
  Api(&'a ApiAccount),
}

impl On<'_> {
  /// Whether `connection` runs on the account.
  fn connection(self, connection: &Connection) -> bool {
    match self {
      Self::SignIn(account) => runs_on(connection, account),
      Self::Api(account) => {
        AgentConnection::try_from(connection.clone()).is_ok_and(|connection| on_account(&connection, account))
      }
    }
  }

  /// Whether `pick` names the account beside its connection, as a team profile's pick may.
  fn picked(self, pick: &ConnectionPick) -> bool {
    matches!(self, Self::SignIn(account) if pick.account == Some(account.id()))
  }

  /// The sign-in an agent may have started on: its session is kept in the sign-in's folder.
  const fn sign_in(self) -> Option<AgentAccountId> {
    match self {
      Self::SignIn(account) => Some(account.id()),
      Self::Api(_) => None,
    }
  }
}

/// A run on the account: one that has not ended and started an agent on it, or has a step on a connection on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunUse {
  pub project_name: Name,
  /// The task's key and the run's number, as `KON-12 #128`.
  pub label: String,
}

/// Everything on an account across the listed projects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountUses {
  pub connections: Vec<ConnectionUse>,
  /// The agent profiles that run on the account on this computer, a project's own and a team's.
  pub profiles: Vec<ProfileUse>,
  pub running: Vec<RunUse>,
  /// Listed projects that could not be read; what they hold is not counted.
  pub unreadable: Vec<Name>,
}

impl AccountUses {
  /// How many projects the connections are in.
  pub fn project_count(&self) -> usize {
    self
      .connections
      .iter()
      .map(|used| used.project)
      .collect::<BTreeSet<_>>()
      .len()
  }
}

/// Whether `connection` runs on `account`; one on an API provider runs on its key, not on an account.
pub fn runs_on(connection: &Connection, account: &AgentAccount) -> bool {
  match connection.kind() {
    ConnectionKind::Agent(provider) => {
      *provider == account.cli().provider()
        && connection.api_provider().is_none()
        && connection.account().id(*provider) == Some(account.id())
    }
    ConnectionKind::GitHub { .. } => false,
  }
}

pub(super) fn uses(account: On<'_>, projects: &[RegisteredProject]) -> AccountUses {
  let mut found = AccountUses::default();
  for project in projects.iter().filter(|project| !project.folder_missing()) {
    if let Err(error) = project_uses(account, project, &mut found) {
      tracing::warn!(%error, project = %project.repo_root.display(), "cannot read what a project runs on an account");
      found.unreadable.push(project.name.clone());
    }
  }
  found
}

fn project_uses(account: On<'_>, project: &RegisteredProject, found: &mut AccountUses) -> Result<(), AccountsError> {
  let db = ProjectDb::open(&project.repo_root)?;
  let connections: Vec<Connection> = db
    .connections()?
    .into_iter()
    .filter(|connection| account.connection(connection))
    .collect();
  let ids: Vec<ConnectionId> = connections.iter().map(Connection::id).collect();
  // Each profile's role with the connection it runs on here, which a person may have chosen beside its version.
  let mut profiles_here: Vec<(Arc<str>, ConnectionId)> = Vec::new();
  let mut own: Vec<ResourceId> = Vec::new();
  for entry in profiles(&db)? {
    let here = db.connection_here(entry.id, entry.profile.connection())?;
    let role: Arc<str> = Arc::from(entry.profile.role().as_str());
    if ids.contains(&here) {
      found.profiles.push(ProfileUse {
        project: project.id,
        project_name: project.name.clone(),
        profile: entry.id,
        role: Some(role.clone()),
      });
    }
    own.push(entry.id);
    profiles_here.push((role, here));
  }
  // A team's profile is picked for here beside the team's configuration: the pick says what it runs on.
  for profile in db.connection_picks()?.into_keys().filter(|id| !own.contains(id)) {
    let Some(pick) = db.connection_pick(profile)? else {
      continue;
    };
    if ids.contains(&pick.connection) || account.picked(&pick) {
      found.profiles.push(ProfileUse {
        project: project.id,
        project_name: project.name.clone(),
        profile,
        role: None,
      });
    }
  }
  for connection in &connections {
    found.connections.push(ConnectionUse {
      project: project.id,
      project_name: project.name.clone(),
      connection: connection.id(),
      connection_name: connection.name().clone(),
      profiles: profiles_here
        .iter()
        .filter(|(_, here)| *here == connection.id())
        .map(|(role, _)| role.clone())
        .collect(),
    });
  }
  for run in db.unfinished_runs()? {
    // A run that cannot be read may be on the account: the project counts as unreadable.
    let run = run.map_err(|unreadable| unreadable.error)?;
    if run_on_account(&db, &run, account.sign_in(), &ids)? {
      let key = db
        .task(run.task())?
        .map(|task| task.key().to_string())
        .unwrap_or_default();
      found.running.push(RunUse {
        project_name: project.name.clone(),
        // A run's number reads with its `#` already (`#2`).
        label: format!("{key} {}", run.number()),
      });
    }
  }
  Ok(())
}

/// Whether `run`, which has not ended, is on the account: an agent of it started on the sign-in `sign_in` (its
/// session is kept in the sign-in's folder), or a step runs on a connection on it (`connections`): one a person chose
/// for this run, else the one its profile runs on here.
fn run_on_account(
  db: &ProjectDb,
  run: &orchester_core::run::Run,
  sign_in: Option<AgentAccountId>,
  connections: &[ConnectionId],
) -> Result<bool, StoreError> {
  if let Some(sign_in) = sign_in
    && db
      .invocation_accounts(run.id())?
      .iter()
      .any(|(_, started_on)| *started_on == sign_in)
  {
    return Ok(true);
  }
  for step in run.snapshot().steps() {
    let StepExecutor::Agent { profile, .. } = step.executor() else {
      continue;
    };
    let connection = match db.step_connection(run.id(), step.key())? {
      Some(chosen) => Some(chosen),
      None => db
        .version::<AgentProfile>(profile.resource, profile.number)?
        .map(|version| db.connection_here(profile.resource, version.body().connection()))
        .transpose()?,
    };
    if connection.is_some_and(|connection| connections.contains(&connection)) {
      return Ok(true);
    }
  }
  Ok(false)
}

/// Moves every connection on `account` in the listed projects to `target`.
pub(super) fn move_connections(
  account: &AgentAccount,
  target: &ConnectionAccount,
  projects: &[RegisteredProject],
) -> Result<(), AccountsError> {
  for project in projects.iter().filter(|project| !project.folder_missing()) {
    let mut db = match ProjectDb::open(&project.repo_root) {
      Ok(db) => db,
      // Its connections follow when it opens again (`follow_accounts`).
      Err(error) => {
        tracing::warn!(%error, project = %project.repo_root.display(), "a project's connections move when it opens");
        continue;
      }
    };
    for mut connection in db.connections()? {
      if runs_on(&connection, account) {
        follow(&mut connection, target.clone())?;
        db.save_connection(&connection)?;
      }
    }
  }
  Ok(())
}

/// Moves `connection` to `target`, another account or the same one in another folder, and checks it there at once,
/// as choosing an account in Edit connection does: the folder may be signed out, or signed in to someone else, and a
/// check made in the old folder would let a run start that the CLI then refuses.
///
/// A connection left on no account is not checked: without a folder the CLI would run on Default's and could read
/// "Connected" for a connection no run can start on. It says *No account* until a person chooses one.
fn follow(connection: &mut Connection, target: ConnectionAccount) -> Result<(), AccountsError> {
  let unassigned = target == ConnectionAccount::Unassigned;
  connection.set_account(target).map_err(AccountsError::Connection)?;
  if !unassigned && let Some(check) = crate::connections::probe::recheck(connection) {
    connection
      .record_check(check)
      .expect("the check was made for the connection's own kind");
  }
  Ok(())
}

/// Brings the connections of the project at `repo_root` up to date: one still on a `removed` account (with where
/// its connections went) moves to that account, through removals made since, or to no account; one on a listed
/// account whose folder moved takes the folder it has now.
pub(super) fn follow_accounts(
  repo_root: &Path,
  removed: &[(AgentAccountId, Option<AgentAccountId>)],
  accounts: &[AgentAccount],
) -> Result<(), AccountsError> {
  let mut db = ProjectDb::open(repo_root)?;
  for mut connection in db.connections()? {
    let ConnectionAccount::Account { id, .. } = connection.account() else {
      continue;
    };
    let target = if removed.iter().any(|(gone, _)| gone == id) {
      destination(*id, removed, accounts)
    } else {
      match accounts.iter().find(|account| account.id() == *id) {
        Some(account) => ConnectionAccount::of(account),
        None => continue,
      }
    };
    if target == *connection.account() {
      continue;
    }
    follow(&mut connection, target)?;
    db.save_connection(&connection)?;
  }
  Ok(())
}

/// Where the connections of removed account `id` end up, following each removal's choice to a listed account.
fn destination(
  mut id: AgentAccountId,
  removed: &[(AgentAccountId, Option<AgentAccountId>)],
  accounts: &[AgentAccount],
) -> ConnectionAccount {
  for _ in 0..=removed.len() {
    if let Some(account) = accounts.iter().find(|account| account.id() == id) {
      return ConnectionAccount::of(account);
    }
    match removed.iter().find(|(gone, _)| *gone == id) {
      Some((_, Some(next))) => id = *next,
      _ => break,
    }
  }
  ConnectionAccount::Unassigned
}
