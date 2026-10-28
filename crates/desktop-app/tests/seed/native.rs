//! The `native` scenario: native Anthropic API connections in each state the native frames show, and a run waiting
//! for a person to approve a native agent's command.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ConnectionStatus},
  run::{CommandShell, NativeCommandAsk},
  store::ConnectionStore,
};

use super::Seed;

#[test]
fn seed_native_has_native_connections_and_a_native_command_waiting() {
  let db = Seed::of("native").db();
  let mut native: Vec<(String, Option<ConnectionStatus>, usize)> = db
    .connections()
    .expect("connections")
    .into_iter()
    .filter_map(|stored| AgentConnection::try_from(stored).ok())
    .filter(|connection| connection.provider() == AgentProvider::AnthropicApi)
    .map(|connection| {
      let models = connection.api().map_or(0, |api| api.models().models().len());
      let status = connection.last_check().map(|check| check.result.status());
      (connection.name().as_str().to_owned(), status, models)
    })
    .collect();
  native.sort_by(|a, b| a.0.cmp(&b.0));
  assert_eq!(
    native,
    [
      ("Anthropic API · Archived".to_owned(), None, 0),
      (
        "Anthropic API · Kontur".to_owned(),
        Some(ConnectionStatus::Connected),
        3
      ),
      ("Anthropic API · New".to_owned(), None, 0),
      (
        "Anthropic API · Old key".to_owned(),
        Some(ConnectionStatus::KeyRejected),
        3
      ),
    ]
  );
  let waiting = db.pending_tool_approvals_all().expect("approvals");
  assert_eq!(waiting.len(), 1);
  assert_eq!(
    waiting[0].native,
    Some(NativeCommandAsk {
      shell: Some(CommandShell::Pwsh),
      time_limit_seconds: 120,
    })
  );
}
