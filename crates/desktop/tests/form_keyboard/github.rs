//! The GitHub tab of Connections: GitHub rows, the inspector in each state a check can leave
//! (checks made with a replayed `gh`, never the real one), the account panel with its command
//! rows, and the GitHub connection dialog. Copy buttons are not pressed: that would write to
//! the clipboard of the machine running the tests, so the command text is asserted instead.

use std::{collections::HashMap, path::PathBuf};

use lurq::node::element::ElementRef;
use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{GitHubHost, GitHubLogin, LastCheck, check_github},
  id::ConnectionId,
  name::Name,
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  run::BranchName,
  store::{ConnectionStore, RepositoryStore},
  time::Timestamp,
};
use orchester_desktop::connections::{
  data::{ConnectionDraft, save_in},
  github::{GitHubDraft, save_github_in},
};
use orchester_store::ProjectDb;

use crate::harness::Desktop;

pub(crate) const VERSION: &str = include_str!("../../../core/tests/fixtures/github/gh_version.stdout");
pub(crate) const SIGNED_IN: &str = include_str!("../../../core/tests/fixtures/github/gh_auth_status_signed_in.stdout");
const SIGNED_OUT: &str = include_str!("../../../core/tests/fixtures/github/gh_auth_status_not_signed_in.stdout");
/// Signed in as ivanzaida without the `repo` scope; shaped like the captured output.
const NO_REPO_SCOPE: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"ivanzaida","tokenSource":"keyring","scopes":"read:org","gitProtocol":"https"}]}}"#;
const AUTH_STATUS: &str = "auth status --hostname github.com --json hosts";
const INSPECTOR: &str = "connection-inspector";
const PANEL: &str = "github-account-panel";
/// More presses than the screen has stops.
const PRESSES: usize = 40;

/// Answers `gh`'s read-only commands with fixed outputs; anything else was never allowed to run.
pub(crate) struct ReplayGh(HashMap<&'static str, Result<CommandOutput, CommandError>>);

impl ReplayGh {
  pub(crate) fn new(version: Result<&str, CommandError>, auth: Option<&str>) -> Self {
    let mut answers = HashMap::from([("--version", version.map(ok))]);
    if let Some(auth) = auth {
      answers.insert(AUTH_STATUS, Ok(ok(auth)));
    }
    Self(answers)
  }
}

impl CommandRunner for ReplayGh {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let key = command.args().join(" ");
    self
      .0
      .get(key.as_str())
      .cloned()
      .unwrap_or_else(|| panic!("`{key}` is not a read-only gh command of the check"))
  }
}

fn ok(stdout: &str) -> CommandOutput {
  CommandOutput {
    code: Some(0),
    stdout: stdout.to_owned(),
    stderr: String::new(),
  }
}

fn login(value: &str) -> GitHubLogin {
  GitHubLogin::new(value).expect("valid login")
}

fn gh_path(desktop: &Desktop) -> PathBuf {
  desktop.repo().join("gh.exe")
}

/// Stores a GitHub connection whose last check `gh` answered through `runner`. Only a verified
/// program can be saved from the dialog, so the connection is saved with a signed-in check
/// first and the replayed check is recorded over it, as Check again does.
pub(crate) fn github_connection(desktop: &Desktop, runner: &ReplayGh) -> ConnectionId {
  let host = GitHubHost::github_com();
  let signed_in = ReplayGh::new(Ok(VERSION), Some(SIGNED_IN));
  let draft = GitHubDraft {
    id: None,
    name: Name::new("GitHub · github.com").expect("valid name"),
    host: host.clone(),
    executable: gh_path(desktop),
    check: check_github(&gh_path(desktop), &host, &signed_in, Timestamp::now()),
  };
  let id = save_github_in(desktop.repo(), draft).expect("the GitHub connection is saved");
  let mut db = ProjectDb::open(desktop.repo()).expect("project database");
  let mut connection = db.connection(id).expect("read").expect("stored");
  let check = check_github(&gh_path(desktop), &host, runner, Timestamp::now());
  connection
    .record_check(LastCheck::GitHub(check))
    .expect("a GitHub check");
  db.save_connection(&connection).expect("the check is stored");
  id
}

/// Links the project's repository through `connection`, expecting `expected`.
pub(crate) fn link(desktop: &Desktop, connection: ConnectionId, expected: &str) {
  let repo = GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", "kontur").expect("valid repository");
  let view = RepositoryView {
    repo,
    default_branch: BranchName::new("main").expect("valid branch"),
    permission: RepoPermission::Admin,
  };
  let link = RepositoryLink::new(
    connection,
    RemoteName::origin(),
    view,
    login(expected),
    Timestamp::now(),
  );
  let mut db = ProjectDb::open(desktop.repo()).expect("project database");
  db.save_repository_link(&link).expect("the link is saved");
}

fn agent_connection(desktop: &Desktop) {
  use orchester_core::connection::{AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn};
  let draft = ConnectionDraft {
    id: None,
    name: Name::new("Codex · Kontur").expect("valid name"),
    provider: AgentProvider::CodexCli,
    executable: desktop.repo().join("codex.exe"),
    check: ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in: SignIn::SignedOut,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    },
  };
  save_in(desktop.repo(), draft).expect("the agent connection is saved");
}

fn open_connections(desktop: &mut Desktop) {
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for(INSPECTOR);
}

pub(crate) fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content().is_some_and(|content| content.contains(text))
    || element.children().into_iter().any(|child| contains_text(child, text))
}

pub(crate) fn shows(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  let element = desktop
    .tree
    .get_element_by_id(id)
    .unwrap_or_else(|| panic!("{id} is shown"));
  contains_text(element, text)
}

fn shown(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

#[test]
fn github_tab_lists_only_github_rows_and_the_signed_in_account() {
  let mut desktop = Desktop::with_project();
  let github = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  agent_connection(&desktop);
  open_connections(&mut desktop);
  let row = format!("connection-{github}");
  // All connections: GitHub first, then the agent provider.
  assert!(shows(&mut desktop, &row, "gh · github.com · signed in as ivanzaida"));
  assert!(shows(&mut desktop, "connection-list", "Codex · Kontur"));
  desktop.click("connections-tab-github");
  assert!(shows(&mut desktop, &row, "0 repos"));
  assert!(shows(&mut desktop, &row, "Signed in"));
  assert!(!shows(&mut desktop, "connection-list", "Codex · Kontur"));
  desktop.click("connections-tab-providers");
  assert!(!shown(&mut desktop, &row));
}

#[test]
fn github_inspector_signed_in_shows_account_scopes_and_no_link_section() {
  let mut desktop = Desktop::with_project();
  github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  open_connections(&mut desktop);
  for text in [
    "github.com",
    "gh 2.97.0",
    "ivanzaida",
    "gist, read:org, repo, workflow",
    "Signed in",
  ] {
    assert!(shows(&mut desktop, INSPECTOR, text), "{text}");
  }
  // Without a repository link there is no expected account.
  assert!(!shows(&mut desktop, INSPECTOR, "Expected account"));
  assert!(shown(&mut desktop, "github-change-account"));
  assert!(!shown(&mut desktop, "github-command-login"));
}

#[test]
fn github_inspector_not_signed_in_offers_the_login_command() {
  let mut desktop = Desktop::with_project();
  github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_OUT)));
  open_connections(&mut desktop);
  assert!(shows(&mut desktop, INSPECTOR, "Not signed in"));
  assert!(shows(
    &mut desktop,
    "github-command-login",
    "gh auth login --hostname github.com"
  ));
  assert!(shown(&mut desktop, "github-command-login-copy"));
}

#[test]
fn github_inspector_gh_not_found_offers_choosing_the_program() {
  let mut desktop = Desktop::with_project();
  github_connection(&desktop, &ReplayGh::new(Err(CommandError::NotFound), None));
  open_connections(&mut desktop);
  assert!(shows(&mut desktop, INSPECTOR, "gh not found"));
  assert!(shown(&mut desktop, "github-choose-program"));
  assert!(!shown(&mut desktop, "github-change-account"));
}

#[test]
fn github_inspector_unrecognized_output() {
  let mut desktop = Desktop::with_project();
  github_connection(&desktop, &ReplayGh::new(Ok("git version 2.46.0"), None));
  open_connections(&mut desktop);
  assert!(shows(&mut desktop, INSPECTOR, "Unrecognized"));
  assert!(shows(
    &mut desktop,
    "github-state-hint",
    "did not answer as the GitHub CLI"
  ));
}

#[test]
fn github_inspector_missing_scope_offers_the_refresh_command() {
  let mut desktop = Desktop::with_project();
  github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(NO_REPO_SCOPE)));
  open_connections(&mut desktop);
  assert!(shows(&mut desktop, INSPECTOR, "Missing scope: repo"));
  assert!(shows(&mut desktop, INSPECTOR, "read:org"));
  assert!(shows(
    &mut desktop,
    "github-command-refresh",
    "gh auth refresh -h github.com -s repo"
  ));
}

#[test]
fn github_inspector_wrong_account_shows_what_the_project_expects() {
  let mut desktop = Desktop::with_project();
  let github = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  link(&desktop, github, "ivan-work");
  open_connections(&mut desktop);
  assert!(shows(&mut desktop, INSPECTOR, "Wrong account"));
  assert!(shows(&mut desktop, INSPECTOR, "Expected account"));
  assert!(shows(&mut desktop, INSPECTOR, "ivanzaida / kontur"));
  assert!(shows(
    &mut desktop,
    "github-command-switch",
    "gh auth switch -h github.com -u ivan-work"
  ));
  assert!(shows(
    &mut desktop,
    "github-use-account",
    "Use this account (ivanzaida)"
  ));
  assert!(shows(&mut desktop, &format!("connection-{github}"), "1 repo"));
}

#[test]
fn github_account_panel_lists_the_commands_and_traps_tab() {
  let mut desktop = Desktop::with_project();
  let github = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  link(&desktop, github, "ivanzaida");
  open_connections(&mut desktop);
  desktop.click("github-change-account");
  desktop.wait_for(PANEL);
  assert_eq!(desktop.focused().as_deref(), Some(PANEL));
  assert!(shows(&mut desktop, PANEL, "Kontur expects"));
  assert!(shows(
    &mut desktop,
    "github-command-login",
    "gh auth login --hostname github.com"
  ));
  assert!(shows(
    &mut desktop,
    "github-command-switch",
    "gh auth switch -h github.com -u ivanzaida"
  ));
  assert!(shows(
    &mut desktop,
    "github-command-refresh",
    "gh auth refresh -h github.com -s repo"
  ));
  let controls = [
    "github-command-login-copy",
    "github-command-switch-copy",
    "github-command-refresh-copy",
    "github-account-close",
    "github-account-check",
  ];
  let stops = desktop.stops(PRESSES, false);
  for control in controls {
    assert!(
      stops.iter().any(|stop| stop.as_deref() == Some(control)),
      "Tab reaches {control}"
    );
  }
  for stop in stops.into_iter().flatten() {
    assert!(controls.contains(&stop.as_str()), "Tab left the panel for {stop}");
  }
  desktop.click("github-account-close");
  assert!(!shown(&mut desktop, PANEL));
}

/// Only form controls take the focus: the rows and the inspector's actions and Copy are
/// pointer-only.
#[test]
fn github_rows_and_inspector_actions_are_not_tab_stops() {
  let mut desktop = Desktop::with_project();
  let github = github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_OUT)));
  open_connections(&mut desktop);
  let row = format!("connection-{github}");
  let stops = desktop.stops(PRESSES, false);
  for control in [
    row.as_str(),
    "github-command-login-copy",
    "test-connection",
    "github-change-account",
    "edit-connection",
  ] {
    assert!(
      !stops.iter().any(|stop| stop.as_deref() == Some(control)),
      "Tab stops at {control}"
    );
  }
  // They still act on a click.
  desktop.click("github-change-account");
  desktop.wait_for(PANEL);
}

#[test]
fn github_dialog_opens_from_the_github_tab_with_a_suggested_name() {
  let mut desktop = Desktop::with_project();
  github_connection(&desktop, &ReplayGh::new(Ok(VERSION), Some(SIGNED_IN)));
  open_connections(&mut desktop);
  desktop.click("connections-tab-github");
  desktop.click("add-connection");
  desktop.wait_for("connection-dialog");
  assert!(shows(&mut desktop, "connection-dialog", "Add GitHub connection"));
  // The stored connection has the plain name, so the suggestion is numbered.
  assert_eq!(desktop.value("connection-name"), "GitHub · github.com 2");
  assert!(!shown(&mut desktop, "github-other-host"));
  assert!(shows(&mut desktop, "connection-dialog", "Uses gh’s own sign-in"));
  // The Save button stays disabled until gh has been checked, and Enter saves nothing.
  desktop
    .tree
    .get_element_by_id_mut("connection-name")
    .expect("connection name")
    .focus();
  desktop.enter();
  assert!(shown(&mut desktop, "connection-dialog"));
}

/// Other host… is chosen with the keyboard; the host it asks for is validated as a host name.
#[test]
fn github_dialog_other_host_is_validated() {
  let mut desktop = Desktop::with_project();
  open_connections_empty(&mut desktop);
  desktop.click("connections-tab-github");
  desktop.click("add-connection");
  desktop.wait_for("connection-dialog");
  desktop.tab_to("connection-name", PRESSES);
  // Connection type, then Host: selects without an id of their own.
  assert_eq!(desktop.stops(2, false), [None, None]);
  desktop.key("ArrowDown", "ArrowDown");
  desktop.key("ArrowDown", "ArrowDown");
  desktop.enter();
  desktop.wait_for("github-other-host");
  desktop.click("github-other-host");
  desktop.type_text("https://ghe");
  assert!(shows(&mut desktop, "connection-dialog", "without https://"));
  for _ in 0.."https://ghe".len() {
    desktop.key("Backspace", "Backspace");
  }
  desktop.type_text("ghe.example.com");
  assert!(!shows(&mut desktop, "connection-dialog", "without https://"));
  // The suggested name follows a valid host.
  assert_eq!(desktop.value("connection-name"), "GitHub · ghe.example.com");
}

#[test]
fn github_dialog_traps_tab() {
  let mut desktop = Desktop::with_project();
  open_connections_empty(&mut desktop);
  desktop.click("connections-tab-github");
  desktop.click("add-connection");
  desktop.wait_for("connection-dialog");
  let controls = [
    "connection-dialog-close",
    "connection-name",
    "connection-program",
    "connection-recheck",
    "connection-cancel",
    "connection-save",
  ];
  // Selects have no id of their own and show up as unnamed stops.
  for stop in desktop.stops(PRESSES, false).into_iter().flatten() {
    assert!(controls.contains(&stop.as_str()), "Tab left the dialog for {stop}");
  }
}

/// Connections with no connection yet: the tab's empty state.
fn open_connections_empty(desktop: &mut Desktop) {
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for("add-connection");
}
