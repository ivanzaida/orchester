//! The computer's resources' answers (`orchester_core::computer`, `docs/architecture/remote-runners.md`, rule 4):
//! every connection, what it has on the computer and why the computer could not answer survive serialization as they
//! were, a serialized connection is read back through the checks of a stored one, and the CLI's config folder and its
//! account's start problem are read as the runner read them before the interface.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::AccountCli,
  computer::{CliHomes, ComputerConnection, ComputerError},
  connection::{
    AccountStartProblem, AgentConnection, AgentEndpoint, AgentProvider, ApiEndpoint, ApiProvider, BaseUrl, CheckResult,
    Connection, ConnectionAccount, ConnectionCheck, ConnectionKind, EndpointPreset, GitHubCheck, GitHubCheckResult,
    GitHubHost, LastCheck,
  },
  id::{AgentAccountId, ConnectionId, SecretId},
  name::Name,
  resource::ModelId,
  time::Timestamp,
};

fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
  let text = serde_json::to_string(value).expect("the answer serializes");
  serde_json::from_str(&text).expect("the answer reads back")
}

fn program(name: &str) -> PathBuf {
  std::env::temp_dir().join(name)
}

fn cli(provider: AgentProvider) -> AgentConnection {
  AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude Code").expect("valid name"),
    provider,
    program("claude.exe"),
    Timestamp::from_unix_millis(1_700_000_000_000),
  )
  .expect("an absolute program fits an agent CLI")
}

/// A Claude Code connection on an account whose folder is `folder`, checked, with a model requirement recorded.
fn on_account(folder: &Path) -> Connection {
  let mut connection = cli(AgentProvider::ClaudeCodeCli);
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::from_unix_millis(1_700_000_100_000),
      result: CheckResult::NotRecognized,
    })
    .expect("a program check fits an agent CLI");
  connection.record_model_requirement(&ModelId::new("claude-opus-5-5").expect("valid model"), "3.1.0");
  connection
    .set_account(ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: folder.to_path_buf(),
    })
    .expect("an agent CLI runs on an account");
  connection.into_connection()
}

/// The homes of a computer whose environment names `claude` as Claude Code's folder, and no Codex one.
fn homes(claude: Option<&str>, home: &Path) -> CliHomes {
  CliHomes::new(
    |variable| {
      claude
        .filter(|_| variable == AccountCli::ClaudeCode.folder_variable())
        .map(PathBuf::from)
    },
    Some(home.to_path_buf()),
  )
}

#[test]
fn computer_connections_of_every_kind_round_trip_as_they_were() {
  let account = tempfile::tempdir().expect("a scratch folder");
  let mut on_provider = cli(AgentProvider::CodexCli);
  let key = SecretId::generate();
  let base_url = BaseUrl::new("https://api.deepseek.com/anthropic").expect("valid base URL");
  let provider = ApiProvider::new(base_url, key, vec![ModelId::new("deepseek-v4").expect("valid model")])
    .expect("a valid API provider");
  on_provider
    .set_api_provider(Some(provider))
    .expect("a Codex connection runs on an API provider");
  let native = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Anthropic API").expect("valid name"),
    AgentProvider::AnthropicApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.anthropic.com").expect("valid base URL"),
      SecretId::generate(),
    ),
    Timestamp::from_unix_millis(1_700_000_000_000),
  )
  .expect("an official endpoint fits a native agent");
  let github = Connection::from_stored(
    ConnectionId::generate(),
    Name::new("GitHub").expect("valid name"),
    ConnectionKind::GitHub {
      host: GitHubHost::new("github.com").expect("valid host"),
    },
    AgentEndpoint::Program(program("gh.exe")),
    Some(LastCheck::GitHub(GitHubCheck {
      at: Timestamp::from_unix_millis(1_700_000_200_000),
      result: GitHubCheckResult::ExecutableMissing,
    })),
    Timestamp::from_unix_millis(1_700_000_000_000),
  )
  .expect("a GitHub check fits a GitHub connection");
  let connections = [
    on_account(account.path()),
    on_account(&account.path().join("gone")),
    on_provider.into_connection(),
    native.into_connection(),
    github,
  ];
  let homes = homes(None, account.path());
  for connection in connections {
    assert_eq!(round_trip(&connection), connection);
    let answer = ComputerConnection::read(connection, Path::is_dir, &homes);
    assert_eq!(round_trip(&answer), answer);
  }
}

#[test]
fn computer_errors_round_trip_with_what_the_read_said() {
  let read = std::io::Error::other("database is locked");
  let error = ComputerError::new(&read);
  assert_eq!(error.to_string(), "database is locked");
  assert_eq!(round_trip(&error), error);
}

#[test]
fn computer_connection_that_does_not_fit_its_kind_is_refused_when_read_back() {
  let connection = cli(AgentProvider::ClaudeCodeCli).into_connection();
  let mut value = serde_json::to_value(&connection).expect("the connection serializes");
  value["endpoint"] = serde_json::json!({ "program": "claude.exe" });
  assert!(
    serde_json::from_value::<Connection>(value).is_err(),
    "a relative program is refused as it is for a stored connection"
  );
}

#[test]
fn computer_reads_an_accounts_folder_as_the_clis_config_folder_and_says_when_it_is_gone() {
  let account = tempfile::tempdir().expect("a scratch folder");
  let homes = homes(Some("C:/elsewhere"), account.path());

  let here = ComputerConnection::read(on_account(account.path()), Path::is_dir, &homes);
  assert_eq!(here.here().account_problem(), None);
  assert_eq!(here.here().config_folder(), Some(account.path()));

  let gone = account.path().join("gone");
  let missing = ComputerConnection::read(on_account(&gone), Path::is_dir, &homes);
  assert_eq!(
    missing.here().account_problem(),
    Some(&AccountStartProblem::FolderMissing { folder: gone.clone() })
  );
  assert_eq!(missing.here().config_folder(), Some(gone.as_path()));
}

#[test]
fn computer_reads_a_default_accounts_config_folder_from_the_environment_else_the_home_folder() {
  let home = tempfile::tempdir().expect("a scratch folder");
  let claude = cli(AgentProvider::ClaudeCodeCli).into_connection();
  let codex = cli(AgentProvider::CodexCli).into_connection();

  let named = homes(Some("C:/claude-config"), home.path());
  let read = ComputerConnection::read(claude.clone(), |_| false, &named);
  assert_eq!(read.here().account_problem(), None);
  assert_eq!(read.here().config_folder(), Some(Path::new("C:/claude-config")));
  let read = ComputerConnection::read(codex.clone(), |_| false, &named);
  assert_eq!(read.here().config_folder(), Some(home.path().join(".codex").as_path()));

  let unnamed = homes(None, home.path());
  let read = ComputerConnection::read(claude, |_| false, &unnamed);
  assert_eq!(read.here().config_folder(), Some(home.path().join(".claude").as_path()));

  let nowhere = CliHomes::new(|_| None, None);
  let read = ComputerConnection::read(codex, |_| false, &nowhere);
  assert_eq!(read.here().config_folder(), None);
  assert_eq!(nowhere.home(), None);
}

#[test]
fn computer_gives_a_connection_that_runs_no_cli_no_config_folder() {
  let home = tempfile::tempdir().expect("a scratch folder");
  let homes = homes(Some("C:/claude-config"), home.path());
  let native = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("OpenAI API").expect("valid name"),
    AgentProvider::OpenAiApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.openai.com/v1").expect("valid base URL"),
      SecretId::generate(),
    ),
    Timestamp::from_unix_millis(1_700_000_000_000),
  )
  .expect("an official endpoint fits a native agent");
  let read = ComputerConnection::read(native.into_connection(), |_| false, &homes);
  assert_eq!(read.here().config_folder(), None);
  assert_eq!(read.here().account_problem(), None);
  let (connection, _) = read.into_parts();
  assert_eq!(connection.kind(), &ConnectionKind::Agent(AgentProvider::OpenAiApi));
}
