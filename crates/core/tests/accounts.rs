use std::path::{Path, PathBuf};

use orchester_core::{
  account::{
    AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AccountIdentity, AccountState, AgentAccount,
    CheckedProgram, FolderProblem, MoveTo, Removal, RemovalError, account_folder_name, brought_folder_problem,
    same_sign_in,
  },
  command::Command,
  connection::{
    AccountStartProblem, AgentConnection, AgentProvider, ConnectionAccount,
    invocation::{Platform, account_environment, on_account},
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
  },
  id::{AgentAccountId, ConnectionId},
  name::Name,
  time::Timestamp,
};

fn home() -> PathBuf {
  if cfg!(windows) {
    PathBuf::from(r"C:\Users\ana")
  } else {
    PathBuf::from("/Users/ana")
  }
}

fn account(cli: AccountCli, name: &str, folder: AccountFolder) -> AgentAccount {
  AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    Name::new(name).expect("a name"),
    folder,
    Timestamp::from_unix_millis(1),
  )
  .expect("an account")
}

fn created(name: &str) -> AgentAccount {
  let folder = home()
    .join("AppData")
    .join("Local")
    .join("Orchester")
    .join("accounts")
    .join(name);
  account(AccountCli::ClaudeCode, name, AccountFolder::Created(folder))
}

fn signed_in(email: &str) -> AccountCheck {
  AccountCheck {
    at: Timestamp::from_unix_millis(2),
    result: AccountCheckResult::SignedIn {
      program: CheckedProgram {
        path: PathBuf::from("claude"),
        version: "2.1.296".into(),
      },
      identity: AccountIdentity {
        email: Some(email.into()),
        ..AccountIdentity::default()
      },
    },
  }
}

fn signed_out() -> AccountCheck {
  AccountCheck {
    at: Timestamp::from_unix_millis(3),
    result: AccountCheckResult::SignedOut {
      program: CheckedProgram {
        path: PathBuf::from("claude"),
        version: "2.1.296".into(),
      },
      was: None,
    },
  }
}

#[test]
fn accounts_each_cli_has_a_fixed_default_on_its_own_folder() {
  for cli in AccountCli::ALL {
    let default = AgentAccount::default_of(cli);
    assert!(default.is_default());
    assert_eq!(default.id(), cli.default_account());
    assert_eq!(default.folder().path(), None);
    assert_eq!(default.name(), None);
  }
  assert_ne!(
    AccountCli::ClaudeCode.default_account(),
    AccountCli::Codex.default_account()
  );
  assert_eq!(AccountCli::ClaudeCode.folder_variable(), "CLAUDE_CONFIG_DIR");
  assert_eq!(AccountCli::Codex.folder_variable(), "CODEX_HOME");
}

#[test]
fn accounts_an_added_account_needs_an_absolute_folder_of_its_own() {
  let name = Name::new("Work").expect("a name");
  let id = AgentAccountId::generate();
  let own = AgentAccount::new(
    id,
    AccountCli::Codex,
    name.clone(),
    AccountFolder::Own,
    Timestamp::now(),
  );
  assert!(own.is_err());
  let relative = AgentAccount::new(
    id,
    AccountCli::Codex,
    name,
    AccountFolder::Brought(PathBuf::from("relative")),
    Timestamp::now(),
  );
  assert!(relative.is_err());
}

#[test]
fn accounts_a_sign_out_after_a_sign_in_reads_as_expired_and_keeps_who_it_was() {
  let mut work = created("Work");
  assert_eq!(work.state(Platform::Windows), AccountState::NotChecked);
  work.record_check(signed_out());
  assert_eq!(work.state(Platform::Windows), AccountState::NotSignedIn);
  work.record_check(signed_in("ana@example.com"));
  assert_eq!(work.state(Platform::Windows), AccountState::SignedIn);
  work.record_check(signed_out());
  assert_eq!(work.state(Platform::Windows), AccountState::Expired);
  assert_eq!(work.email(), Some("ana@example.com"));
  assert!(!AccountState::Expired.lets_runs_start());
  assert!(AccountState::Unknown.lets_runs_start());
}

#[test]
fn accounts_claude_accounts_other_than_default_are_not_supported_on_macos_yet() {
  let work = created("Work");
  assert_eq!(work.state(Platform::MacOs), AccountState::Unsupported);
  assert_eq!(
    AgentAccount::default_of(AccountCli::ClaudeCode).state(Platform::MacOs),
    AccountState::NotChecked
  );
  let codex = account(
    AccountCli::Codex,
    "Client",
    AccountFolder::Brought(home().join(".codex-client")),
  );
  assert_eq!(codex.state(Platform::MacOs), AccountState::NotChecked);
}

#[test]
fn accounts_two_folders_signed_in_as_one_person_are_the_same_account() {
  let mut work = created("Work");
  let mut freelance = created("Freelance");
  work.record_check(signed_in("ana@example.com"));
  freelance.record_check(signed_in("ana@example.com"));
  let accounts = vec![work.clone(), freelance.clone()];
  assert_eq!(
    same_sign_in(&freelance, &accounts).map(AgentAccount::id),
    Some(work.id())
  );
}

#[test]
fn accounts_created_folder_names_follow_the_cli_and_name() {
  assert_eq!(
    account_folder_name("claude", "Freelance", |_| false),
    "claude-freelance"
  );
  assert_eq!(
    account_folder_name("codex", "Kontur team!", |_| false),
    "codex-kontur-team"
  );
  assert_eq!(
    account_folder_name("claude", "Work", |name| name == "claude-work"),
    "claude-work-2"
  );
  assert_eq!(account_folder_name("claude", "***", |_| false), "claude");
}

#[test]
fn accounts_folders_inside_a_repository_or_taken_or_elsewhere_are_refused() {
  let home = home();
  let repository = home.join("projects").join("kontur");
  let inside = repository.join(".claude-ci");
  assert_eq!(
    brought_folder_problem(&inside, Some(&repository), (&[], None), Some(&home), &as_written),
    Some(FolderProblem::InsideRepository(repository.clone()))
  );
  let personal = account(
    AccountCli::ClaudeCode,
    "Personal",
    AccountFolder::Brought(home.join(".claude-personal")),
  );
  assert_eq!(
    brought_folder_problem(
      &home.join(".claude-personal"),
      None,
      (std::slice::from_ref(&personal), None),
      Some(&home),
      &as_written
    ),
    Some(FolderProblem::Taken(personal.id()))
  );
  let default = AgentAccount::default_of(AccountCli::ClaudeCode);
  let own = home.join(".claude");
  assert_eq!(
    brought_folder_problem(
      &own,
      None,
      (std::slice::from_ref(&default), Some(&own)),
      Some(&home),
      &as_written
    ),
    Some(FolderProblem::Taken(default.id()))
  );
  assert_eq!(
    brought_folder_problem(
      &home.join("Documents").join("claude"),
      None,
      (&[], None),
      Some(&home),
      &as_written
    ),
    Some(FolderProblem::OutsideCredentialPlaces)
  );
  assert_eq!(
    brought_folder_problem(Path::new("relative"), None, (&[], None), Some(&home), &as_written),
    Some(FolderProblem::NotAbsolute)
  );
  assert_eq!(
    brought_folder_problem(
      &home.join(".claude-freelance"),
      None,
      (std::slice::from_ref(&personal), None),
      Some(&home),
      &as_written
    ),
    None
  );
}

#[test]
fn accounts_folders_with_dots_or_nested_with_another_accounts_are_refused() {
  let home = home();
  let personal = account(
    AccountCli::ClaudeCode,
    "Personal",
    AccountFolder::Brought(home.join(".claude-personal")),
  );
  let dotted = home.join(".claude-personal").join("..").join("Documents").join("creds");
  assert_eq!(
    brought_folder_problem(&dotted, None, (&[], None), Some(&home), &as_written),
    Some(FolderProblem::NotAbsolute),
    "`..` is refused, not resolved past the rules"
  );
  for nested in [
    home.join(".claude-personal").join("inner"),
    home.join(".CLAUDE-PERSONAL").join("inner"),
  ] {
    let problem = brought_folder_problem(
      &nested,
      None,
      (std::slice::from_ref(&personal), None),
      Some(&home),
      &as_written,
    );
    let case_blind = cfg!(windows) || !nested.to_string_lossy().contains("CLAUDE");
    assert_eq!(
      problem == Some(FolderProblem::Taken(personal.id())),
      case_blind,
      "{nested:?}"
    );
  }
  let created = account(
    AccountCli::ClaudeCode,
    "CI",
    AccountFolder::Brought(home.join(".agents").join("claude-ci")),
  );
  assert_eq!(
    brought_folder_problem(
      &home.join(".agents"),
      None,
      (std::slice::from_ref(&created), None),
      Some(&home),
      &as_written
    ),
    Some(FolderProblem::Taken(created.id())),
    "a folder holding another account's"
  );
}

/// Paths as written: these rules read no file system.
fn as_written(path: &Path) -> std::path::PathBuf {
  path.to_path_buf()
}

#[test]
fn accounts_removal_moves_connections_waits_for_runs_and_keeps_brought_folders() {
  let work = created("Work");
  let personal = account(
    AccountCli::ClaudeCode,
    "Personal",
    AccountFolder::Brought(home().join(".claude-personal")),
  );
  let client = account(
    AccountCli::Codex,
    "Client",
    AccountFolder::Brought(home().join(".codex-client")),
  );
  let default = AgentAccount::default_of(AccountCli::ClaudeCode);
  let accounts = vec![default.clone(), work.clone(), personal.clone(), client.clone()];
  assert_eq!(
    Removal::plan(&default, MoveTo::NoAccount, false, &accounts, false),
    Err(RemovalError::Default)
  );
  assert_eq!(
    Removal::plan(&work, MoveTo::Account(personal.id()), false, &accounts, true),
    Err(RemovalError::InUse)
  );
  assert_eq!(
    Removal::plan(&work, MoveTo::Account(client.id()), false, &accounts, false),
    Err(RemovalError::NotAnotherAccount)
  );
  assert_eq!(
    Removal::plan(&work, MoveTo::Account(work.id()), false, &accounts, false),
    Err(RemovalError::NotAnotherAccount)
  );
  assert_eq!(
    Removal::plan(&personal, MoveTo::NoAccount, true, &accounts, false),
    Err(RemovalError::FolderNotCreated)
  );
  let removal = Removal::plan(&work, MoveTo::Account(default.id()), true, &accounts, false).expect("allowed");
  assert_eq!(removal.trash(), work.folder().path());
  assert_eq!(removal.move_to(), MoveTo::Account(default.id()));
}

#[test]
fn accounts_a_connection_runs_its_cli_with_the_accounts_folder_variable() {
  let work = created("Work");
  let account = ConnectionAccount::of(&work);
  let folder = work.folder().path().expect("a folder");
  let environment = vec![
    ("PATH".to_owned(), "bin".to_owned()),
    ("claude_config_dir".to_owned(), "elsewhere".to_owned()),
  ];
  let set = account_environment(environment.clone(), &account, AgentProvider::ClaudeCodeCli).expect("unicode");
  assert_eq!(
    set,
    vec![
      ("PATH".to_owned(), "bin".to_owned()),
      ("CLAUDE_CONFIG_DIR".to_owned(), folder.display().to_string()),
    ]
  );
  let unchanged = account_environment(
    environment.clone(),
    &ConnectionAccount::Default,
    AgentProvider::ClaudeCodeCli,
  );
  assert_eq!(unchanged.expect("unicode"), environment);
  let command = on_account(
    Command::new(Path::new("codex"), ["app-server"]).with_env("CODEX_HOME", "project's"),
    &ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: home().join(".codex-client"),
    },
    AgentProvider::CodexCli,
  )
  .expect("unicode");
  assert_eq!(
    command.env(),
    [(
      "CODEX_HOME".to_owned(),
      home().join(".codex-client").display().to_string()
    )]
  );
}

#[test]
fn accounts_a_run_does_not_start_without_an_account_or_its_folder_or_on_an_unsupported_one() {
  let work = ConnectionAccount::of(&created("Work"));
  assert_eq!(
    ConnectionAccount::Unassigned.start_problem(AgentProvider::ClaudeCodeCli, Platform::Windows, |_| true),
    Some(AccountStartProblem::NoAccount)
  );
  assert!(matches!(
    work.start_problem(AgentProvider::ClaudeCodeCli, Platform::Windows, |_| false),
    Some(AccountStartProblem::FolderMissing { .. })
  ));
  assert_eq!(
    work.start_problem(AgentProvider::ClaudeCodeCli, Platform::MacOs, |_| true),
    Some(AccountStartProblem::Unsupported)
  );
  assert_eq!(
    work.start_problem(AgentProvider::ClaudeCodeCli, Platform::Windows, |_| true),
    None
  );
  assert_eq!(
    ConnectionAccount::Default.start_problem(AgentProvider::CodexCli, Platform::MacOs, |_| false),
    None
  );
}

#[test]
fn accounts_choosing_another_account_drops_the_connections_check_and_refuses_another_cli() {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude").expect("a name"),
    AgentProvider::ClaudeCodeCli,
    home().join("claude.exe"),
    Timestamp::now(),
  )
  .expect("a connection");
  let client = account(
    AccountCli::Codex,
    "Client",
    AccountFolder::Brought(home().join(".codex-client")),
  );
  let mut plain = (*connection).clone();
  assert!(plain.choose_account(&client).is_err());
  assert!(plain.choose_account(&created("Work")).is_ok());
  assert!(matches!(plain.account(), ConnectionAccount::Account { .. }));
  connection
    .set_account(ConnectionAccount::Unassigned)
    .expect("an agent CLI");
  assert_eq!(connection.account(), &ConnectionAccount::Unassigned);
}

fn reading(account: Option<AgentAccountId>, at: i64, used: f64) -> LimitReading {
  LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    account,
    at: Timestamp::from_unix_millis(at),
    limits: PlanLimits {
      windows: vec![LimitWindow::new(300, Some(used), None, LimitState::Normal, false).expect("a window")],
      plan: None,
      credits: None,
      blocked: None,
    },
  }
}

#[test]
fn accounts_plan_readings_are_kept_per_account() {
  let work = AgentAccountId::generate();
  let mut readings = PlanReadings::default();
  assert!(readings.observe(reading(Some(work), 10, 42.0)));
  assert!(readings.observe(reading(None, 20, 8.0)));
  assert_eq!(readings.all().len(), 2, "Work and Default each keep their own");
  let default = AccountCli::ClaudeCode.default_account();
  assert_eq!(
    readings
      .of_account(default)
      .and_then(|reading| reading.limits.windows[0].percent()),
    Some(8.0)
  );
  assert_eq!(
    readings
      .of_account(work)
      .and_then(|reading| reading.limits.windows[0].percent()),
    Some(42.0)
  );
  assert_eq!(
    readings.get(AgentProvider::ClaudeCodeCli).map(|reading| reading.at),
    Some(Timestamp::from_unix_millis(20)),
    "a view per CLI reads the newest"
  );
}
