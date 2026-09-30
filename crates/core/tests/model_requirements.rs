//! The CLI version a model needs, learned from the provider's refusal, and the connections that
//! cannot run it.

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, CliTooOld, ConnectionCheck, ModelCatalog,
    ModelRequirements, SignIn, parse_cli_too_old,
  },
  id::ConnectionId,
  name::Name,
  resource::ModelId,
  time::Timestamp,
};

/// What Claude Code 2.1.89 reported for Opus 5.5 on 2026-09-30 (`docs/evidence/claude-code-e2e-2026-09-30.md`).
const REFUSAL: &str = r#"API Error: 400 {"type":"error","error":{"type":"invalid_request_error","message":"Claude Code 2.1.89 does not support this model; version 2.1.280 or newer is required. Run 'claude update', or update the Claude desktop app, then try again.","details":{"error_code":"claude_code_version_too_old"}},"request_id":"req_011CfaC5sRWN5TgLMQevCUrj"}"#;

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("valid model id")
}

fn claude(version: &str) -> AgentConnection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude").expect("name"),
    AgentProvider::ClaudeCodeCli,
    std::env::current_dir().expect("directory").join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute path");
  connection.record_check(ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: version.to_owned(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
    },
  });
  connection
}

#[test]
fn model_requirements_read_the_refusal_of_an_old_cli() {
  assert_eq!(
    parse_cli_too_old(REFUSAL),
    Some(CliTooOld {
      installed: "2.1.89".to_owned(),
      required: "2.1.280".to_owned()
    })
  );
  for other in [
    "API Error: 400 model not found",
    "Claude Code 2.1.89 does not support this model; version newer or newer is required",
    "does not support this model; version 2.1.280 or newer is required",
  ] {
    assert_eq!(parse_cli_too_old(other), None, "{other}");
  }
}

#[test]
fn model_requirements_keep_the_highest_version_each_model_needs() {
  let mut requirements = ModelRequirements::default();
  assert!(requirements.is_empty());
  requirements.record(&model("claude-opus-5-5"), "2.1.280");
  requirements.record(&model("claude-opus-5-5"), "2.1.9");
  assert_eq!(requirements.required(&model("claude-opus-5-5")), Some("2.1.280"));
  assert_eq!(requirements.required(&model("claude-sonnet-5-5")), None);
  let stored = serde_json::to_string(&requirements).expect("JSON");
  assert_eq!(stored, r#"{"claude-opus-5-5":"2.1.280"}"#);
  assert_eq!(
    serde_json::from_str::<ModelRequirements>(&stored).expect("JSON"),
    requirements
  );
}

#[test]
fn model_requirements_tell_which_connection_cannot_run_a_model() {
  let opus = model("claude-opus-5-5");
  let mut old = claude("2.1.89");
  assert_eq!(old.cannot_run(&opus), None, "nothing is known before a refusal");
  old.record_model_requirement(&opus, "2.1.280");
  assert_eq!(
    old.cannot_run(&opus),
    Some(CliTooOld {
      installed: "2.1.89".to_owned(),
      required: "2.1.280".to_owned()
    })
  );
  assert_eq!(old.cannot_run(&model("claude-sonnet-5-5")), None);
  let mut new = claude("2.1.284");
  new.record_model_requirement(&opus, "2.1.280");
  assert_eq!(new.cannot_run(&opus), None);
  let unchecked = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude").expect("name"),
    AgentProvider::ClaudeCodeCli,
    std::env::current_dir().expect("directory").join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute path")
  .into_connection()
  .with_model_requirements(old.model_requirements().clone());
  let unchecked = AgentConnection::try_from(unchecked).expect("agent");
  assert_eq!(
    unchecked.cannot_run(&opus),
    None,
    "an unknown version warns about nothing"
  );
}
