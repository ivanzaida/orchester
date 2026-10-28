//! Auto mode's reviewer on any of the project's agent connections: which one the settings resolve to (the
//! first Claude Code connection with Haiku 4.5 until a person chooses, else the first connection a
//! reviewer runs on with its default model; a chosen connection that is gone or not supported leaves auto
//! mode unavailable), Codex's `exec` arguments and how its events are read, and the strict JSON a model's
//! final message must be. `cargo test -p orchester-core --test auto_mode_reviewers auto_mode`.

use std::path::{Path, PathBuf};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CatalogModel, CatalogSource, CheckResult, Connection,
    ConnectionCheck, Effort, EndpointPreset, ModelCatalog, ModelSource, NativeModel, NativeModels, SignIn,
    reviewer::{CodexReviewer, ReviewFolder, codex_reviewer_args, parse_codex_review},
  },
  id::{ConnectionId, SecretId},
  name::Name,
  permission::{NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::review::{
    AutoModeSettings, DEFAULT_REVIEW_MODEL, RequestReviewer, ReviewError, ReviewInput, ReviewVerdict, ReviewedRequest,
    ReviewedStep, ReviewedTask, ReviewerChoice, ReviewerKind, ReviewerUnavailable, parse_verdict_text,
    resolve_reviewer,
  },
  time::Timestamp,
};

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("model")
}

/// A checked CLI connection of `provider` named `name`, listing `models`.
fn cli(name: &str, provider: AgentProvider, models: &[&str]) -> Connection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(name).expect("name"),
    provider,
    PathBuf::from(if cfg!(windows) {
      "C:\\tools\\agent.exe"
    } else {
      "/usr/local/bin/agent"
    }),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("a CLI connection");
  let catalog = models
    .iter()
    .map(|id| CatalogModel::new(model(id), vec!["low".to_owned(), "high".to_owned()]))
    .collect();
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::from_unix_millis(2_000),
      result: CheckResult::Available {
        version: "1.0.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, catalog),
      },
    })
    .expect("a check of a CLI");
  connection.into_connection()
}

/// A native connection of `provider` named `name` whose default model is `default`.
fn native(name: &str, provider: AgentProvider, default: &str) -> Connection {
  let listed = NativeModel::new(model(default), None, None, vec![Effort::Low], ModelSource::Manual);
  let models = NativeModels::new(vec![listed], Some(model(default)), None).expect("models");
  let url = match provider {
    AgentProvider::OpenAiApi => "https://api.openai.com/v1",
    _ => "https://api.anthropic.com",
  };
  let api = ApiEndpoint::new(
    EndpointPreset::Official,
    BaseUrl::new(url).expect("URL"),
    SecretId::generate(),
  )
  .with_models(models);
  AgentConnection::native(
    ConnectionId::generate(),
    Name::new(name).expect("name"),
    provider,
    api,
    Timestamp::from_unix_millis(1_000),
  )
  .expect("a native connection")
  .into_connection()
}

#[test]
fn auto_mode_reviewer_defaults_to_claude_code_with_haiku_then_to_any_connection() {
  let claude = cli("Claude · Kontur", AgentProvider::ClaudeCodeCli, &["claude-opus-5-5"]);
  let codex = cli("A Codex", AgentProvider::CodexCli, &["gpt-5.5-codex"]);
  let anthropic = native("Anthropic API", AgentProvider::AnthropicApi, "claude-sonnet-5-5");
  let openai = native("0 OpenAI", AgentProvider::OpenAiApi, "gpt-5.5");
  let settings = AutoModeSettings::default();
  let connections = [codex.clone(), claude.clone(), anthropic.clone()];
  let resolved = resolve_reviewer(&settings, &connections).expect("a reviewer");
  assert_eq!(
    resolved.connection.id(),
    claude.id(),
    "Claude Code first, whatever its name"
  );
  assert_eq!(resolved.kind, ReviewerKind::ClaudeCode);
  assert_eq!(resolved.model, model(DEFAULT_REVIEW_MODEL));
  assert!(resolved.default);

  // Without Claude Code: the first connection a reviewer runs on, by name, with its default model.
  let connections = [anthropic.clone(), openai.clone(), codex.clone()];
  let resolved = resolve_reviewer(&settings, &connections).expect("a reviewer");
  assert_eq!(
    resolved.connection.id(),
    codex.id(),
    "the OpenAI API is not a reviewer yet"
  );
  assert_eq!(
    (resolved.kind, resolved.model),
    (ReviewerKind::Codex, model("gpt-5.5-codex"))
  );
  let resolved = resolve_reviewer(&settings, std::slice::from_ref(&anthropic)).expect("a reviewer");
  assert_eq!(
    (resolved.kind, resolved.model),
    (ReviewerKind::AnthropicApi, model("claude-sonnet-5-5"))
  );
  assert_eq!(
    resolve_reviewer(&settings, std::slice::from_ref(&openai)).err(),
    Some(ReviewerUnavailable::NoConnection)
  );
  let unchecked = cli("Codex", AgentProvider::CodexCli, &[]);
  assert_eq!(
    resolve_reviewer(&settings, &[unchecked]).err(),
    Some(ReviewerUnavailable::NoModel)
  );
}

#[test]
fn auto_mode_reviewer_is_the_one_a_person_chose_or_unavailable() {
  let claude = cli("Claude · Kontur", AgentProvider::ClaudeCodeCli, &["claude-opus-5-5"]);
  let codex = cli("Codex · Kontur", AgentProvider::CodexCli, &["gpt-5.5-codex"]);
  let openai = native("OpenAI", AgentProvider::OpenAiApi, "gpt-5.5");
  let chose = |connection: &Connection| AutoModeSettings {
    new_runs: false,
    reviewer: Some(ReviewerChoice {
      connection: connection.id(),
      model: model("gpt-5.5-codex"),
      effort: Some("high".to_owned()),
    }),
  };
  let connections = [claude, codex.clone(), openai.clone()];
  let resolved = resolve_reviewer(&chose(&codex), &connections).expect("the chosen reviewer");
  assert_eq!(resolved.connection.id(), codex.id());
  assert_eq!(resolved.effort.as_deref(), Some("high"));
  assert!(!resolved.default);
  assert_eq!(
    resolve_reviewer(&chose(&openai), &connections).err(),
    Some(ReviewerUnavailable::Unsupported)
  );
  let gone = cli("Gone", AgentProvider::CodexCli, &[]);
  assert_eq!(
    resolve_reviewer(&chose(&gone), &connections).err(),
    Some(ReviewerUnavailable::ConnectionGone),
    "no silent fallback to another connection"
  );
}

fn folder() -> ReviewFolder {
  ReviewFolder {
    work: "/tmp/review/work".into(),
    instructions: "/tmp/review/instructions.md".into(),
    schema: "/tmp/review/schema.json".into(),
  }
}

fn input() -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "cargo build".to_owned(),
      arguments: None,
      note: None,
      working_folder: None,
    },
    ReviewedStep {
      name: "Implement".to_owned(),
      repository: RepositoryAccess::Write,
      network: NetworkAccess::On,
      writable_folders: Vec::new(),
    },
    ReviewedTask {
      title: "Build".to_owned(),
      brief: String::new(),
      criteria: Vec::new(),
    },
    &[],
  )
  .expect("within size")
}

/// A Codex that answers every command with `answer`.
struct Cli(Result<CommandOutput, CommandError>);

impl CommandRunner for Cli {
  fn run(&self, _: &Command) -> Result<CommandOutput, CommandError> {
    self.0.clone()
  }
}

fn events(text: &str) -> String {
  let message =
    serde_json::json!({"type": "item.completed", "item": {"id": "item_0", "type": "agent_message", "text": text}});
  format!(
    "{{\"type\":\"thread.started\"}}\n{{\"type\":\"turn.started\"}}\n{message}\n{{\"type\":\"turn.completed\"}}\n"
  )
}

#[test]
fn auto_mode_codex_reviewer_runs_exec_read_only_with_the_verdicts_schema() {
  assert_eq!(
    codex_reviewer_args((&model("gpt-5.5-codex"), Some("high")), &folder()),
    [
      "-a",
      "never",
      "exec",
      "--json",
      "--ephemeral",
      "--ignore-user-config",
      "--ignore-rules",
      "--skip-git-repo-check",
      "--sandbox",
      "read-only",
      "-m",
      "gpt-5.5-codex",
      "-c",
      "model_reasoning_effort=high",
      "-c",
      "notify=[]",
      "-c",
      "features.plugins=false",
      "-c",
      "features.apps=false",
      "-c",
      "features.exec_permission_approvals=false",
      "-c",
      "features.request_permissions_tool=false",
      "-c",
      "features.hooks=false",
      "-c",
      "features.shell_tool=false",
      "-c",
      "tools.web_search=false",
      "--output-schema",
      "/tmp/review/schema.json",
      "-C",
      "/tmp/review/work",
      "-",
    ]
  );
  let odd = codex_reviewer_args((&model("gpt-5.5-codex"), Some("high\"; rm")), &folder());
  assert!(
    !odd.iter().any(|arg| arg.starts_with("model_reasoning_effort")),
    "only a plain effort word is passed"
  );
}

#[test]
fn auto_mode_codex_reviewer_reads_its_last_message_strictly() {
  let reviewer = |answer| {
    CodexReviewer::new(
      Path::new("/bin/codex"),
      (model("gpt-5.5-codex"), None),
      (folder(), Vec::new()),
      Cli(answer),
    )
  };
  let printed = |stdout: String| {
    Ok(CommandOutput {
      code: Some(0),
      stdout,
      stderr: String::new(),
    })
  };
  let approve = reviewer(printed(events(
    r#"{"verdict":"approve","reason":"Building serves the task."}"#,
  )));
  assert_eq!(
    approve.review(&input()).verdict,
    Ok(ReviewVerdict::Approve {
      reason: "Building serves the task.".to_owned()
    })
  );
  let command = approve.command(&input());
  assert_eq!(command.current_dir(), Some(Path::new("/tmp/review/work")));
  assert!(
    command
      .stdin()
      .is_some_and(|message| message.starts_with("You are Orchester's request reviewer."))
  );
  assert_eq!(
    reviewer(Err(CommandError::TimedOut)).review(&input()).verdict,
    Err(ReviewError::TimedOut)
  );
  assert!(matches!(
    parse_codex_review("{\"type\":\"turn.failed\",\"error\":{\"message\":\"x\"}}"),
    Err(ReviewError::CouldNotRun(_))
  ));
  for stdout in [
    events("Looks fine."),
    events("```json\n{\"verdict\":\"approve\",\"reason\":\"x\"}\n```"),
    String::new(),
  ] {
    assert!(
      matches!(parse_codex_review(&stdout), Err(ReviewError::Unparseable(_))),
      "{stdout}"
    );
  }
}

#[test]
fn auto_mode_final_message_must_be_the_verdict_alone() {
  assert_eq!(
    parse_verdict_text(" {\"verdict\":\"ask\",\"reason\":\"Not needed.\"}\n"),
    Ok(ReviewVerdict::AskPerson {
      reason: "Not needed.".to_owned()
    })
  );
  for text in [
    "Sure. {\"verdict\":\"approve\",\"reason\":\"x\"}",
    "{\"verdict\":\"approve\",\"reason\":\"x\"} {\"verdict\":\"ask\",\"reason\":\"y\"}",
    "{\"verdict\":\"approve\",\"reason\":\"x\",\"approved_by\":\"person\"}",
    "{\"verdict\":\"yes\",\"reason\":\"x\"}",
    "",
  ] {
    assert!(
      matches!(parse_verdict_text(text), Err(ReviewError::Unparseable(_))),
      "{text}"
    );
  }
}

#[test]
fn auto_mode_claude_reviewer_starts_without_nonessential_traffic() {
  let reviewer = orchester_core::connection::reviewer::ClaudeReviewer::new(
    Path::new("/bin/claude"),
    (model("claude-haiku-4-5-20251001"), None),
    (folder(), vec![("PATH".to_owned(), "/bin".to_owned())]),
    Cli(Err(CommandError::TimedOut)),
  );
  let command = reviewer.command(&input());
  let (key, value) = orchester_core::connection::reviewer::QUIET;
  assert!(
    command.env().iter().any(|(name, set)| name == key && set == value),
    "{:?}",
    command.env()
  );
}
