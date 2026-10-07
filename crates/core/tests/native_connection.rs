//! Native agent connections (`docs/architecture/native-agent.md` §2.1, §5.4, §7) before native agents
//! run: their providers, endpoints and base URL rules, how Orchester will enforce their permissions, and
//! that no program check describes them.

use std::path::PathBuf;

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentConnection, AgentEndpoint, AgentProvider, AgentRuntime, ApiEndpoint, BaseUrl, CheckResult, Connection,
    ConnectionCheck, ConnectionError, ConnectionKind, ConnectionStatus, EndpointError, EndpointPreset, EngineVendor,
    LastCheck, WireApi, check, invocation::Platform, parse_version,
  },
  id::{ConnectionId, SecretId},
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, FolderBounds, NetworkAccess, OutsideCommands,
    RepositoryAccess, WritableFolder, WritableFolders,
    capability::{Mechanism, PermissionOption, PermissionProblem, PermissionSetting, Support, Unavailable, support},
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
fn native_providers_run_over_a_wire_format_and_only_openai_runs_no_agent_yet() {
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
    assert_eq!(provider.runs_agents(), provider != AgentProvider::OpenAiApi);
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
  assert_eq!(url.host(), "api.deepseek.com");
  assert_eq!(
    BaseUrl::new("http://[::1]:8080/v1").expect("valid").host(),
    "[::1]:8080"
  );
  assert_eq!(
    BaseUrl::new("https://gateway.example.dev:8443/v1")
      .expect("valid")
      .as_str(),
    "https://gateway.example.dev:8443/v1"
  );
  assert!(BaseUrl::new("https://[2001:db8::1]:443/v1").is_ok());
  // A combining mark (the virama) is part of an IDN label.
  assert!(BaseUrl::new("https://हिन्दी.भारत/v1").is_ok());
  for local in [
    "http://localhost:11434/v1",
    "http://127.0.0.1:8080",
    "http://[::1]:8080/",
    "HTTP://LOCALHOST:11434",
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
    (r"https://api.example.dev\@evil.example", EndpointError::Backslash),
    (r"http://localhost\.evil.example", EndpointError::Backslash),
    ("http://[::1]x", EndpointError::InvalidHost),
    ("http://[::1", EndpointError::InvalidHost),
    ("https://[]/v1", EndpointError::InvalidHost),
    ("https://[api.example.dev]", EndpointError::InvalidHost),
    ("https://api.example.dev:", EndpointError::InvalidHost),
    ("https://api.example.dev:https", EndpointError::InvalidHost),
    ("https://api.example.dev:+443", EndpointError::InvalidHost),
    ("https://api.example.dev:99999", EndpointError::InvalidHost),
    ("https://api<1>.example.dev", EndpointError::InvalidHost),
    // Stricter than an MCP server's URL: no percent-encoded host, no zone id, no empty port.
    ("https://%61pi.example.dev", EndpointError::InvalidHost),
    ("http://[fe80::1%25eth0]:8080", EndpointError::InvalidHost),
    ("http://localhost:8080@api.example.dev", EndpointError::UrlCredentials),
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
fn orchester_enforces_a_native_provider_alike_on_both_platforms() {
  use Mechanism as M;
  let rows = [
    (
      PermissionOption::ReadOnly,
      Support::Enforced(M::NativeWriteToolsWithheld),
    ),
    (PermissionOption::EditWorktree, Support::Enforced(M::NativeFileScope)),
    (
      PermissionOption::AskBeforeCommands,
      Support::Enforced(M::NativeCommandApprovals),
    ),
    (
      PermissionOption::AllowedCommands,
      Support::Enforced(M::NativeCommandList),
    ),
    (
      PermissionOption::AnyCommand,
      Support::Unavailable(Unavailable::CannotConfineCommands),
    ),
    (
      PermissionOption::NetworkOff,
      Support::Unavailable(Unavailable::CannotBlockNetwork),
    ),
    (
      PermissionOption::NetworkOn,
      Support::Enforced(M::NativeNetworkUnrestricted),
    ),
  ];
  let root = tempfile::tempdir().expect("temporary folder");
  std::fs::create_dir_all(root.path().join("repo")).expect("repository folder");
  let bounds = FolderBounds::new(&root.path().join("repo"), None).expect("bounds");
  let target = WritableFolder::new(".orchester/target", &bounds).expect("folder");
  let folders = WritableFolders::new(vec![target]).expect("folders");
  let offline = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
  .with_writable_folders(folders);
  let any = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  for provider in [AgentProvider::AnthropicApi, AgentProvider::OpenAiApi] {
    for platform in [Platform::Windows, Platform::MacOs] {
      for (option, expected) in rows {
        assert_eq!(support(provider, platform, option), expected, "{option:?}");
      }
      let enforced = offline.enforced_on(provider, platform).expect("enforced by Orchester");
      assert_eq!(enforced.mechanism(PermissionSetting::Repository), M::NativeFileScope);
      assert_eq!(enforced.mechanism(PermissionSetting::Commands), M::NativeCommandList);
      assert_eq!(
        enforced.mechanism(PermissionSetting::Network),
        M::NativeNetworkUnrestricted
      );
      assert_eq!(enforced.writable_folders(), Some(M::NativeFileScopeFolders));
      // Off cannot be enforced, so commands keep the network, as with Claude Code.
      assert_eq!(enforced.network_stays_on(), Some(Unavailable::CannotBlockNetwork));
      assert_eq!(enforced.permissions().network(), NetworkAccess::On);
      assert_eq!(enforced.outside_commands(), OutsideCommands::OrchesterAsks);
      assert_eq!(offline.effective_network(provider, platform), NetworkAccess::On);
      assert_eq!(
        any.enforced_on(provider, platform),
        Err(PermissionProblem::NotEnforceable {
          option: PermissionOption::AnyCommand,
          reason: Unavailable::CannotConfineCommands,
        })
      );
    }
  }
}

#[test]
fn native_listed_commands_run_unconfined_and_a_cli_list_is_not_orchesters() {
  let listed = AllowedCommands::new(vec![
    CommandPrefix::new("git").expect("prefix"),
    CommandPrefix::new("pnpm test").expect("prefix"),
  ])
  .expect("list");
  let permissions = |commands| AgentPermissions::new(RepositoryAccess::Write, commands, NetworkAccess::On);
  let with_list = permissions(CommandPermission::Allowed(listed.clone()));
  for provider in [AgentProvider::AnthropicApi, AgentProvider::OpenAiApi] {
    let enforced = with_list.enforced_on(provider, Platform::Windows).expect("enforced");
    assert_eq!(enforced.unconfined_list(), Some(&listed));
    // Nothing listed, or every command asking, leaves no command unconfined without a person.
    for commands in [
      CommandPermission::Allowed(AllowedCommands::default()),
      CommandPermission::AskBefore,
    ] {
      let enforced = permissions(commands)
        .enforced_on(provider, Platform::MacOs)
        .expect("enforced");
      assert_eq!(enforced.unconfined_list(), None);
    }
  }
  // Claude Code runs its own listed commands; Orchester does not.
  let claude = with_list
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
    .expect("enforced by Claude Code");
  assert_eq!(claude.unconfined_list(), None);
}

#[test]
fn native_engine_is_named_after_whoever_receives_the_prompts() {
  let official = native(
    AgentProvider::AnthropicApi,
    api(EndpointPreset::Official, "https://api.anthropic.com"),
  );
  assert_eq!(
    official.expect("official").vendor(),
    EngineVendor::Provider(AgentProvider::AnthropicApi)
  );
  let deepseek = native(
    AgentProvider::OpenAiApi,
    api(EndpointPreset::DeepSeek, "https://api.deepseek.com"),
  );
  assert_eq!(
    deepseek.expect("deepseek").vendor(),
    EngineVendor::Preset(EndpointPreset::DeepSeek)
  );
  let custom = native(
    AgentProvider::AnthropicApi,
    api(EndpointPreset::Custom, "https://gateway.kontur.lan:8443/v1"),
  );
  assert_eq!(
    custom.expect("custom").vendor(),
    EngineVendor::Host("gateway.kontur.lan:8443".to_owned())
  );
  let cli = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex · Kontur").expect("name"),
    AgentProvider::CodexCli,
    program(),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("a CLI connection");
  assert_eq!(cli.vendor(), EngineVendor::Provider(AgentProvider::CodexCli));
}

#[test]
fn nothing_checks_a_native_provider_as_a_program() {
  for provider in [AgentProvider::AnthropicApi, AgentProvider::OpenAiApi] {
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
