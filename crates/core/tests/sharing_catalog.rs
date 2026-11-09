//! What this computer reports for a shared project's web editor (`docs/architecture/server.md`, row 75): the agent
//! roles of its connections and their model ids, and nothing else.
//!
//! `cargo test -p orchester-core --test sharing_catalog`

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::ConnectionId,
  name::Name,
  resource::ModelId,
  sharing::{CATALOG_MODELS_PER_ROLE, catalog},
  time::Timestamp,
};

fn connection(provider: AgentProvider, called: &str, models: &[String]) -> orchester_core::connection::Connection {
  let at = Timestamp::from_unix_millis(1);
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(called).expect("a name"),
    provider,
    std::env::temp_dir().join("agent-cli"),
    at,
  )
  .expect("absolute path");
  let models = models
    .iter()
    .map(|id| CatalogModel::new(ModelId::new(id).expect("a model id"), Vec::new()))
    .collect();
  connection
    .record_check(ConnectionCheck {
      at,
      result: CheckResult::Available {
        version: "1.0.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, models),
      },
    })
    .expect("an agent check fits a CLI connection");
  connection.into_connection()
}

fn ids(ids: &[&str]) -> Vec<String> {
  ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn catalog_lists_the_roles_once_sorted_and_the_models_per_role() {
  let connections = [
    connection(AgentProvider::CodexCli, "Codex", &ids(&["gpt-6", "gpt-5.5"])),
    connection(AgentProvider::CodexCli, "Codex at home", &ids(&["gpt-5.5"])),
    connection(AgentProvider::ClaudeCodeCli, "Claude", &[]),
  ];
  let reported = catalog(&connections);
  assert_eq!(reported.cli_kinds, ["claude_code_cli", "codex_cli"]);
  let codex: Vec<_> = reported.models["codex_cli"].iter().map(ModelId::as_str).collect();
  assert_eq!(codex, ["gpt-5.5", "gpt-6"]);
  assert!(!reported.models.contains_key("claude_code_cli"), "no models, no entry");
}

#[test]
fn catalog_is_empty_without_agent_connections() {
  assert_eq!(catalog(&[]), Default::default());
}

#[test]
fn catalog_keeps_at_most_the_most_model_ids_per_role() {
  let many: Vec<String> = (0..CATALOG_MODELS_PER_ROLE + 8)
    .map(|number| format!("model-{number:04}"))
    .collect();
  let reported = catalog(&[connection(AgentProvider::ClaudeCodeCli, "Claude", &many)]);
  assert_eq!(reported.models["claude_code_cli"].len(), CATALOG_MODELS_PER_ROLE);
}
