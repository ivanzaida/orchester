//! The connections of the Orchestrator tests' project: a checked, signed-in CLI connection with its provider's own
//! models, and a native agent connection, which the Orchestrator does not run on.

use std::path::Path;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CatalogModel, CatalogSource, CheckResult, ConnectionCheck,
    EndpointPreset, ModelCatalog, SignIn,
  },
  id::{ConnectionId, SecretId},
  name::Name,
  resource::ModelId,
  time::Timestamp,
};

/// The models a CLI's connection lists, each with its efforts: Claude Code's `haiku` and `opus`, Codex's `gpt-5.5`
/// and `gpt-5.5-mini`.
fn catalog(provider: AgentProvider) -> ModelCatalog {
  let model = |id: &str, efforts: &[&str]| {
    CatalogModel::new(
      ModelId::new(id).expect("model"),
      efforts.iter().map(|effort| (*effort).to_owned()).collect(),
    )
  };
  let models = match provider {
    AgentProvider::CodexCli => vec![
      model("gpt-5.5", &["low", "medium", "high", "xhigh"]),
      model("gpt-5.5-mini", &["low", "high"]),
    ],
    _ => vec![model("haiku", &["low", "high"]), model("opus", &["low", "high"])],
  };
  ModelCatalog::new(CatalogSource::Provider, models)
}

/// A connection of `provider` named `name` that runs `program`, checked and signed in.
pub fn cli(provider: AgentProvider, name: &str, program: &Path) -> AgentConnection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(name).expect("name"),
    provider,
    program.to_path_buf(),
    Timestamp::now(),
  )
  .expect("absolute program");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: catalog(provider),
      },
    })
    .expect("check");
  connection
}

/// A native agent connection to Anthropic's API.
pub fn native(name: &str) -> AgentConnection {
  AgentConnection::native(
    ConnectionId::generate(),
    Name::new(name).expect("name"),
    AgentProvider::AnthropicApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.anthropic.com").expect("url"),
      SecretId::generate(),
    ),
    Timestamp::now(),
  )
  .expect("native")
}
