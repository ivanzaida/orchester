//! What a local agent profile runs on on this computer (`docs/design/agent-accounts.md`, "Agents run on accounts"):
//! a person chooses an account, the project gets its connection on it when it has none, and the profile's version
//! stays as it is. No test starts a real CLI: the program set for the CLI is a file that is not there, and the
//! search path is passed in.
//!
//! `cargo test -p orchester-workspace --test runs_on runs_on`

use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  account::AccountCli,
  connection::{AgentConnection, AgentProvider, ConnectionAccount, ConnectionStatus},
  id::{AgentAccountId, ConnectionId, ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  accounts::{NewAccount, NewFolder, RunsOnError},
  connections::data::agent_connections,
  project_setup::NewProject,
  workspace::Workspace,
};
use tempfile::TempDir;

struct Fixture {
  data: TempDir,
  repo: TempDir,
  workspace: Workspace,
  /// The connection the profile's version names: Claude Code on its Default account.
  own: ConnectionId,
  profile: ResourceId,
}

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

/// A local project whose Builder is published on a Claude Code connection, and the program of Claude Code set to a
/// file that is not there.
fn fixture() -> Fixture {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&data.path().join("accounts"));
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  workspace
    .create(NewProject {
      project: Project::new(
        ProjectId::generate(),
        ProjectKey::new("KON").expect("valid key"),
        name("Kontur"),
      ),
      repo_root: repo.path().to_path_buf(),
    })
    .expect("created");
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Claude"),
    AgentProvider::ClaudeCodeCli,
    repo.path().join("missing-claude.exe"),
    Timestamp::from_unix_millis(1),
  )
  .expect("absolute");
  let mut project = ProjectDb::open(repo.path()).expect("open");
  project.create_connection(&connection).expect("created");
  let body = AgentProfile::new(
    name("Builder"),
    "Implements the approved plan.".into(),
    PromptText::new("You implement {{task.title}}.").expect("valid prompt"),
    connection.id(),
    ModelId::new("claude-opus").expect("valid model"),
    ProviderSettings::new(BTreeMap::new()).expect("valid settings"),
  );
  let (resource, _) = project
    .create_published(
      Slug::new("builder").expect("a slug"),
      name("Builder"),
      &body,
      Timestamp::from_unix_millis(2),
    )
    .expect("published");
  workspace
    .set_agent_program(AccountCli::ClaudeCode, &data.path().join("no-claude-here.exe"))
    .expect("set");
  Fixture {
    data,
    repo,
    workspace,
    own: connection.id(),
    profile: resource.id(),
  }
}

fn account(workspace: &Workspace, cli: AccountCli, called: &str) -> AgentAccountId {
  workspace
    .add_agent_account(NewAccount {
      cli,
      name: name(called),
      folder: NewFolder::Create,
    })
    .expect("added")
    .id()
}

fn connections(repo: &Path) -> Vec<AgentConnection> {
  agent_connections(ProjectDb::open(repo).expect("open").connections().expect("read"))
}

/// The connection the profile runs on here, and the number of its latest version.
fn runs_on(fixture: &Fixture) -> (ConnectionId, u32) {
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  let version = db
    .latest_version::<AgentProfile>(fixture.profile)
    .expect("read")
    .expect("published");
  let here = db
    .profile_here(fixture.profile, version.body().clone())
    .expect("read")
    .connection();
  (here, version.number().get())
}

fn nothing_installed(_: AccountCli) -> Option<PathBuf> {
  None
}

#[test]
fn runs_on_an_account_the_project_has_no_connection_on_makes_one_and_keeps_the_version() {
  let fixture = fixture();
  let work = account(&fixture.workspace, AccountCli::ClaudeCode, "Work");

  fixture
    .workspace
    .run_profile_on_account_with(fixture.repo.path(), fixture.profile, work, nothing_installed)
    .expect("chosen");

  let listed = connections(fixture.repo.path());
  assert_eq!(listed.len(), 2, "the profile's own and the one on Work");
  let made = listed.iter().find(|found| found.id() != fixture.own).expect("made");
  assert_eq!(made.name().as_str(), "Claude Code · Work");
  assert_eq!(made.provider(), AgentProvider::ClaudeCodeCli);
  assert!(matches!(made.account(), ConnectionAccount::Account { id, .. } if *id == work));
  assert_eq!(
    made.executable(),
    Some(fixture.data.path().join("no-claude-here.exe").as_path()),
    "the program set for the CLI on this computer"
  );
  assert_eq!(
    made.status(),
    ConnectionStatus::ExecutableMissing,
    "checked when it was made"
  );
  assert_eq!(runs_on(&fixture), (made.id(), 1), "no new version");

  fixture
    .workspace
    .run_profile_on_account_with(fixture.repo.path(), fixture.profile, work, nothing_installed)
    .expect("chosen again");
  assert_eq!(
    connections(fixture.repo.path()).len(),
    2,
    "the connection on Work is there already"
  );
}

#[test]
fn runs_on_an_account_the_project_has_a_connection_on_takes_that_one() {
  let fixture = fixture();
  let work = account(&fixture.workspace, AccountCli::ClaudeCode, "Work");
  fixture
    .workspace
    .run_profile_on_account_with(fixture.repo.path(), fixture.profile, work, nothing_installed)
    .expect("chosen");

  fixture
    .workspace
    .run_profile_on_account_with(
      fixture.repo.path(),
      fixture.profile,
      AccountCli::ClaudeCode.default_account(),
      nothing_installed,
    )
    .expect("back on Default");

  assert_eq!(
    connections(fixture.repo.path()).len(),
    2,
    "nothing was made for Default"
  );
  assert_eq!(runs_on(&fixture), (fixture.own, 1));
}

#[test]
fn runs_on_an_account_of_another_cli_is_refused_and_makes_nothing() {
  let fixture = fixture();
  let refused = fixture.workspace.run_profile_on_account_with(
    fixture.repo.path(),
    fixture.profile,
    AccountCli::Codex.default_account(),
    nothing_installed,
  );
  assert!(matches!(refused, Err(RunsOnError::AnotherCli)), "{refused:?}");
  assert_eq!(connections(fixture.repo.path()).len(), 1);
  assert_eq!(runs_on(&fixture), (fixture.own, 1));

  let unknown = fixture.workspace.run_profile_on_account_with(
    fixture.repo.path(),
    fixture.profile,
    AgentAccountId::generate(),
    nothing_installed,
  );
  assert!(matches!(unknown, Err(RunsOnError::AccountNotFound(_))), "{unknown:?}");

  let no_profile = fixture.workspace.run_profile_on_account_with(
    fixture.repo.path(),
    ResourceId::generate(),
    AccountCli::ClaudeCode.default_account(),
    nothing_installed,
  );
  assert!(
    matches!(no_profile, Err(RunsOnError::ProfileNotFound(_))),
    "{no_profile:?}"
  );
}

#[test]
fn runs_on_needs_the_program_of_the_cli_only_to_make_a_connection() {
  let fixture = fixture();
  fixture
    .workspace
    .clear_agent_program(AccountCli::ClaudeCode)
    .expect("cleared");
  let work = account(&fixture.workspace, AccountCli::ClaudeCode, "Work");

  let missing =
    fixture
      .workspace
      .run_profile_on_account_with(fixture.repo.path(), fixture.profile, work, nothing_installed);
  assert!(
    matches!(missing, Err(RunsOnError::CliMissing(AccountCli::ClaudeCode))),
    "{missing:?}"
  );
  assert_eq!(connections(fixture.repo.path()).len(), 1);
  assert_eq!(runs_on(&fixture), (fixture.own, 1), "nothing was chosen");

  let found = fixture.data.path().join("found-on-the-path.exe");
  fixture
    .workspace
    .run_profile_on_account_with(fixture.repo.path(), fixture.profile, work, |_| Some(found.clone()))
    .expect("chosen");
  let listed = connections(fixture.repo.path());
  let made = listed.iter().find(|found| found.id() != fixture.own).expect("made");
  assert_eq!(made.executable(), Some(found.as_path()));
}

#[test]
fn runs_on_a_name_the_project_already_has_is_numbered() {
  let fixture = fixture();
  let work = account(&fixture.workspace, AccountCli::ClaudeCode, "Work");
  let namesake = AgentConnection::new(
    ConnectionId::generate(),
    name("Claude Code · Work"),
    AgentProvider::CodexCli,
    fixture.repo.path().join("codex.exe"),
    Timestamp::from_unix_millis(3),
  )
  .expect("absolute");
  ProjectDb::open(fixture.repo.path())
    .expect("open")
    .create_connection(&namesake)
    .expect("created");

  fixture
    .workspace
    .run_profile_on_account_with(fixture.repo.path(), fixture.profile, work, nothing_installed)
    .expect("chosen");

  let (here, _) = runs_on(&fixture);
  let listed = connections(fixture.repo.path());
  let made = listed.iter().find(|found| found.id() == here).expect("made");
  assert_eq!(made.name().as_str(), "Claude Code · Work (2)");
}

/// A profile's form reads an account's models from the project's connection on it, so the connection is made before
/// anything runs on it; what the profile runs on stays as it was.
#[test]
fn runs_on_connecting_an_account_makes_its_connection_once_and_moves_no_profile() {
  let fixture = fixture();
  let codex = account(&fixture.workspace, AccountCli::Codex, "Work");
  let found = fixture.data.path().join("codex.exe");
  let installed = |_: AccountCli| Some(found.clone());

  let made = fixture
    .workspace
    .connect_account_with(fixture.repo.path(), codex, installed)
    .expect("connected");

  let listed = connections(fixture.repo.path());
  assert_eq!(listed.len(), 2, "the profile's own and the one on Codex's Work");
  let connection = listed.iter().find(|listed| listed.id() == made).expect("made");
  assert_eq!(connection.provider(), AgentProvider::CodexCli);
  assert!(matches!(connection.account(), ConnectionAccount::Account { id, .. } if *id == codex));
  assert_eq!(
    runs_on(&fixture),
    (fixture.own, 1),
    "the profile runs on what it ran on"
  );

  let again = fixture
    .workspace
    .connect_account_with(fixture.repo.path(), codex, nothing_installed)
    .expect("there already");
  assert_eq!(again, made);
  assert_eq!(connections(fixture.repo.path()).len(), 2);
}
