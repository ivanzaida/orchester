//! The chat's MCP server cards (ORC-89): adding a server is a card that shows the server as applying leaves it (its
//! program and arguments, a secret by its name, which tool calls run without asking) and warns that it widens what
//! agents may do; only a person applies it: automation's click is refused (`person_only`) and adds nothing, the
//! person's adds it. A program not found on this computer is warned of on the card.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::SecretId,
  orchestrator::ActionOutcome,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{McpServerStore, SecretStore},
  time::Timestamp,
};
use serde_json::json;

use super::{
  harness::{Orchestrator, Setup},
  proposal::{outcomes, proposal_card},
};

/// A program outside the project's repository, to stand for a server's: this test's own.
fn program() -> String {
  std::env::current_exe()
    .expect("the test program")
    .to_string_lossy()
    .into_owned()
}

/// Kontur with the secret PENCIL_TOKEN in Development, on a new conversation.
fn with_secret() -> Orchestrator {
  let mut screen = Orchestrator::new(Setup::Ready);
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("PENCIL_TOKEN").expect("a secret name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Development,
    },
    Timestamp::now(),
  );
  screen.db().create_secret(&secret).expect("the secret is stored");
  screen.go(OrchestratorChat::New);
  screen
}

/// Asks the chat's agent to call `mcp_add_server` with `arguments`, and waits for its card; the card's element id.
fn proposed(screen: &mut Orchestrator, arguments: &serde_json::Value) -> String {
  screen.send(&format!("Call mcp_add_server {arguments}"));
  screen.until("the proposal", |screen| {
    outcomes(screen).first() == Some(&ActionOutcome::Waits)
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = proposal_card(screen);
  screen.until("the card", |screen| screen.exists(&format!("{card}-start")));
  card
}

#[test]
fn orchestrator_mcp_server_card_is_applied_only_by_a_person() {
  let mut screen = with_secret();
  let arguments = json!({
    "project": "Kontur",
    "name": "pencil",
    "program": program(),
    "args": ["--app", "desktop"],
    "environment": [{ "name": "PENCIL_KEY", "value": "${secret:PENCIL_TOKEN@development}" }],
    "exempt_tools": ["get_screenshot"],
  });
  let card = proposed(&mut screen, &arguments);
  assert_eq!(screen.text(&format!("{card}-title")), "Add the MCP server pencil?");
  // The first three facts show; the rest behind *Show details*.
  screen.click(&format!("{card}-details"));
  let facts = screen.text(&format!("{card}-facts"));
  for shown in [
    "pencil",
    "--app desktop",
    "PENCIL_KEY from secret PENCIL_TOKEN@development",
    "get_screenshot run without asking; every other call asks",
    "No profile chooses it yet",
  ] {
    assert!(facts.contains(shown), "{shown}: {facts}");
  }
  assert!(
    screen.exists(&format!("{card}-mcp-widens")),
    "the card warns that it widens what agents may do"
  );
  assert!(!screen.exists(&format!("{card}-not-found")), "the program is found");
  assert!(
    screen.db().mcp_servers().expect("servers").is_empty(),
    "nothing is added on the agent's word"
  );

  // Pressed until the card takes presses: its button waits while the card reads.
  let (start, person_only) = (format!("{card}-start"), format!("{card}-person-only"));
  screen.until("the refusal", |screen| {
    with_synthetic_input(|| screen.click(&start));
    screen.exists(&person_only)
  });
  assert_eq!(
    outcomes(&screen)[0],
    ActionOutcome::Waits,
    "automation's click applies nothing"
  );
  assert!(screen.db().mcp_servers().expect("servers").is_empty());

  screen.click(&start);
  screen.until("the server", |screen| outcomes(screen)[0] == ActionOutcome::Applied);
  let servers = screen.db().mcp_servers().expect("servers");
  assert_eq!(servers.len(), 1, "the person's Apply added it");
  assert_eq!(servers[0].name().as_str(), "pencil");
  screen.until("the applied card", |screen| {
    screen.text(&format!("{card}-title")) == "Added the MCP server pencil"
  });
}

#[test]
fn orchestrator_mcp_server_card_warns_of_a_program_not_found() {
  let mut screen = with_secret();
  let missing = screen.root.join("..").join("no-such-pencil.exe");
  let missing = missing.to_string_lossy().into_owned();
  let card = proposed(
    &mut screen,
    &json!({ "project": "Kontur", "name": "missing", "program": missing }),
  );
  let warning = screen.text(&format!("{card}-not-found"));
  assert!(
    warning.contains("is not found on this computer's search path"),
    "{warning}"
  );
}
