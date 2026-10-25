//! Orchestrator settings › Profile on a Claude Code connection that runs on an API provider
//! (`docs/design/api-providers.md`): it is offered like any connection the Orchestrator runs on, its models are the
//! ones typed for the provider, and saving the settings on it is accepted. Run with
//! `cargo test -p orchester-workspace --test orchestrator_settings orchestrator_settings`.

mod support;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiProvider, BaseUrl},
  id::{ConnectionId, SecretId},
  name::Name,
  orchestrator::OrchestratorSettings,
  resource::ModelId,
  store::ConnectionStore,
};
use orchester_workspace::orchestrator::settings::{Keep, OrchestratorSetup, Readiness, save_in};
use support::{Fixture, at};

const MODELS: [&str; 2] = ["deepseek-v4-flash", "deepseek-v4-pro"];

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("valid model")
}

/// Keeps a Claude Code connection on DeepSeek's Anthropic API with [`MODELS`] typed for it.
fn on_provider(fixture: &mut Fixture) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · DeepSeek").expect("valid name"),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("bin").join("claude.exe"),
    at(1),
  )
  .expect("absolute executable");
  let provider = ApiProvider::new(
    BaseUrl::new("https://api.deepseek.com/anthropic").expect("valid base URL"),
    SecretId::generate(),
    MODELS.iter().map(|id| model(id)).collect(),
  )
  .expect("a model");
  connection.set_api_provider(Some(provider)).expect("on Default");
  let id = connection.id();
  fixture
    .db
    .create_connection(&connection.into_connection())
    .expect("kept");
  id
}

#[test]
fn orchestrator_settings_offer_a_claude_code_connection_on_an_api_provider_with_its_typed_models() {
  let mut fixture = Fixture::new();
  let connection = on_provider(&mut fixture);
  let setup = OrchestratorSetup::load_in(fixture.repo.path()).expect("read");
  let choice = setup
    .connections()
    .into_iter()
    .find(|choice| choice.id == connection)
    .expect("listed");
  assert_eq!(choice.unavailable, None, "the Orchestrator can run on it");
  let models: Vec<String> = setup
    .models(connection, None)
    .into_iter()
    .map(|option| option.id.as_str().to_owned())
    .collect();
  assert_eq!(models, MODELS, "the models typed for the provider");
  assert_eq!(
    setup.choosing(connection),
    Some(OrchestratorSettings::new(connection, model(MODELS[0]), None)),
    "choosing it starts on its first typed model"
  );
}

#[test]
fn orchestrator_settings_on_an_api_provider_are_saved_and_published() {
  let mut fixture = Fixture::new();
  let connection = on_provider(&mut fixture);
  let root = fixture.repo.path();
  let draft = OrchestratorSettings::new(connection, model(MODELS[1]), None);
  assert_eq!(save_in(root, &draft, Keep::Draft), Ok(None), "the draft is accepted");
  assert_eq!(save_in(root, &draft, Keep::Publish), Ok(Some(1)), "v1 is published");
  let setup = OrchestratorSetup::load_in(root).expect("read");
  assert_eq!(setup.settings, Some(draft));
  assert_eq!(
    setup.readiness(),
    Readiness::Ready {
      connection: "Claude · DeepSeek".into(),
      model: model(MODELS[1]),
    },
    "the chat can send on it"
  );
  assert_eq!(setup.provider(), Some(AgentProvider::ClaudeCodeCli));
}
