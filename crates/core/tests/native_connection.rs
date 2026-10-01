//! Native agent connections (`docs/architecture/native-agent.md` §2.1, §7) before native agents run:
//! their providers, endpoints and base URL rules, and that nothing checks, enforces or runs them yet.

use std::path::PathBuf;

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentConnection, AgentEndpoint, AgentProvider, AgentRuntime, ApiEndpoint, BaseUrl, CheckResult, Connection,
    ConnectionCheck, ConnectionError, ConnectionKind, ConnectionStatus, EndpointError, EndpointPreset, LastCheck,
    WireApi, check, invocation::Platform, parse_version,
  },
  id::{ConnectionId, SecretId},
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, PermissionProblem, Support, Unavailable, support},
  },
  time::Timestamp,
};

fn api(preset: EndpointPreset, url: &str) -> ApiEndpoint {
  ApiEndpoint::new(preset, BaseUrl::new(url).expect("valid base URL"), SecretId::generate())
}

fn native(provider: AgentProvider, endpoint: ApiEndpoint) -> Result<AgentConnection, ConnectionError> {
  AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Claude API · Kontur").expect("valid name"),
    provider,
    endpoint,
    Timestamp::from_unix_millis(1_000),
  )
}

fn program() -> PathBuf {
  std::env::temp_dir().join("claude")
}

/// Fails the test if anything is run.
struct NothingRuns;

impl CommandRunner for NothingRuns {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    panic!("`{}` ran for a native connection", command.args().join(" "))
  }
}

#[test]
fn native_providers_run_over_a_wire_format_and_run_no_agent_yet() {
  assert_eq!(
    AgentProvider::AnthropicApi.runtime(),
    AgentRuntime::Native(WireApi::AnthropicMessages)
  );
  assert_eq!(
    AgentProvider::OpenAiApi.runtime(),
    AgentRuntime::Native(WireApi::OpenAiResponses)
  );
  assert_eq!(
    AgentProvider::CodexCli.runtime(),
    AgentRuntime::Cli { program: "codex" }
  );
  assert_eq!(AgentProvider::ClaudeCodeCli.program_name(), Some("claude"));
  assert_eq!(AgentProvider::OpenAiApi.program_name(), None);
  for provider in [
    AgentProvider::CodexCli,
    AgentProvider::ClaudeCodeCli,
    AgentProvider::AnthropicApi,
    AgentProvider::OpenAiApi,
  ] {
    assert_eq!(provider.runs_agents(), !provider.is_native());
    assert_eq!(AgentProvider::CLI.contains(&provider), !provider.is_native());
  }
  assert_eq!(
    serde_json::to_string(&AgentProvider::OpenAiApi).expect("serializes"),
    r#""openai_api""#
  );
  assert_eq!(
    serde_json::to_string(&AgentProvider::AnthropicApi).expect("serializes"),
    r#""anthropic_api""#
  );
}

#[test]
fn native_base_urls_are_https_or_this_computer_without_credentials_or_query() {
  let url = BaseUrl::new(" https://api.deepseek.com/ ").expect("valid");
  assert_eq!(url.as_str(), "https://api.deepseek.com");
  assert_eq!(
    BaseUrl::new("https://gateway.example.dev:8443/v1")
      .expect("valid")
      .as_str(),
    "https://gateway.example.dev:8443/v1"
  );
  for local in [
    "http://localhost:11434/v1",
    "http://127.0.0.1:8080",
    "http://[::1]:8080/",
  ] {
    assert!(BaseUrl::new(local).is_ok(), "{local} is this computer");
  }
  let refused = [
    ("", EndpointError::BlankUrl),
    ("http://api.example.dev", EndpointError::UrlScheme),
    ("http://localhost.example.dev", EndpointError::UrlScheme),
    ("ftp://api.example.dev", EndpointError::UrlScheme),
    ("https://", EndpointError::NoHost),
    ("https://:443/v1", EndpointError::NoHost),
    ("https://user:key@api.example.dev", EndpointError::UrlCredentials),
    ("https://api.example.dev/v1?key=1", EndpointError::UrlQuery),
    ("https://api.example.dev/v1#models", EndpointError::UrlQuery),
    ("https://api.example.dev/a b", EndpointError::Whitespace),
  ];
  for (url, error) in refused {
    assert_eq!(BaseUrl::new(url), Err(error), "{url}");
  }
}

#[test]
fn a_native_connection_is_reached_at_its_api_and_never_at_a_program() {
  let endpoint = api(EndpointPreset::Official, "https://api.anthropic.com");
  let connection = native(AgentProvider::AnthropicApi, endpoint.clone()).expect("native connection");
  assert_eq!(connection.endpoint(), &AgentEndpoint::Api(endpoint.clone()));
  assert_eq!(connection.executable(), None);
  assert_eq!(connection.status(), ConnectionStatus::NotChecked);
  let mut connection = connection;
  assert!(matches!(
    connection.set_executable(program()),
    Err(ConnectionError::EndpointMismatch(_))
  ));
  assert_eq!(connection.endpoint(), &AgentEndpoint::Api(endpoint.clone()));
  // A native provider needs an API endpoint, and a CLI or GitHub a program.
  assert!(matches!(
    AgentConnection::new(
      ConnectionId::generate(),
      Name::new("Anthropic").expect("valid name"),
      AgentProvider::AnthropicApi,
      program(),
      Timestamp::from_unix_millis(1_000),
    ),
    Err(ConnectionError::EndpointMismatch(_))
  ));
  assert!(matches!(
    native(AgentProvider::ClaudeCodeCli, endpoint),
    Err(ConnectionError::EndpointMismatch(_))
  ));
  assert!(matches!(
    Connection::new(
      ConnectionId::generate(),
      Name::new("Codex").expect("valid name"),
      ConnectionKind::Agent(AgentProvider::CodexCli),
      AgentEndpoint::Program(PathBuf::from("codex")),
      Timestamp::from_unix_millis(1_000),
    ),
    Err(ConnectionError::RelativeExecutable(_))
  ));
}

#[test]
fn a_native_connection_refuses_an_agent_check() {
  let endpoint = api(EndpointPreset::Official, "https://api.openai.com/v1");
  let mut connection = native(AgentProvider::OpenAiApi, endpoint.clone()).expect("native connection");
  let agent_check = ConnectionCheck {
    at: Timestamp::from_unix_millis(2_000),
    result: CheckResult::ExecutableMissing,
  };
  assert!(matches!(
    connection.record_check(agent_check.clone()),
    Err(ConnectionError::CheckKindMismatch(_))
  ));
  assert_eq!(connection.last_check(), None);
  assert_eq!(connection.status(), ConnectionStatus::NotChecked);
  // A stored check of a native connection does not load either.
  assert!(matches!(
    Connection::from_stored(
      ConnectionId::generate(),
      Name::new("OpenAI").expect("valid name"),
      ConnectionKind::Agent(AgentProvider::OpenAiApi),
      AgentEndpoint::Api(endpoint),
      Some(LastCheck::Agent(agent_check)),
      Timestamp::from_unix_millis(1_000),
    ),
    Err(ConnectionError::CheckKindMismatch(_))
  ));
}

#[test]
fn native_deepseek_preset_is_offered_on_the_openai_api_only() {
  let deepseek = api(EndpointPreset::DeepSeek, "https://api.deepseek.com");
  assert_eq!(
    native(AgentProvider::AnthropicApi, deepseek.clone()),
    Err(ConnectionError::PresetNotOffered)
  );
  let connection = native(AgentProvider::OpenAiApi, deepseek).expect("DeepSeek on the OpenAI API");
  assert_eq!(
    connection.endpoint().api().map(ApiEndpoint::preset),
    Some(EndpointPreset::DeepSeek)
  );
  // The Anthropic route of a compatible server is reachable through Custom.
  assert!(
    native(
      AgentProvider::AnthropicApi,
      api(EndpointPreset::Custom, "https://api.deepseek.com/anthropic")
    )
    .is_ok()
  );
}

#[test]
fn nothing_is_enforced_checked_or_run_for_a_native_provider() {
  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::On,
  );
  for provider in [AgentProvider::AnthropicApi, AgentProvider::OpenAiApi] {
    for platform in [Platform::Windows, Platform::MacOs] {
      for option in [
        PermissionOption::ReadOnly,
        PermissionOption::EditWorktree,
        PermissionOption::AskBeforeCommands,
        PermissionOption::AllowedCommands,
        PermissionOption::AnyCommand,
        PermissionOption::NetworkOff,
        PermissionOption::NetworkOn,
      ] {
        assert_eq!(
          support(provider, platform, option),
          Support::Unavailable(Unavailable::NativeNotAvailable)
        );
      }
      assert_eq!(
        permissions.enforced_on(provider, platform),
        Err(PermissionProblem::NativeNotAvailable)
      );
    }
    let checked = check(
      provider,
      &program(),
      &NothingRuns,
      &[],
      Timestamp::from_unix_millis(2_000),
    );
    assert_eq!(checked, None, "a native provider has no CLI to check");
    let output = CommandOutput {
      code: Some(0),
      stdout: "2.1.89 (Claude Code)".into(),
      stderr: String::new(),
    };
    assert_eq!(parse_version(provider, &output), None);
  }
}
