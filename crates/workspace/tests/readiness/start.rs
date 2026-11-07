//! A start on a pipeline that is not ready here is refused with the parts missing, and starts nothing: the parts are
//! typed, each has its message, and no secret value reaches either.

use std::collections::BTreeSet;

use orchester_core::{
  connection::{AgentProvider, mcp::McpServerName},
  id::ResourceId,
  resource::{Pipeline, PromptText, Step, StepKey, StepKind},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::{LocalProgramProblem, Missing},
};
use orchester_workspace::readiness::{NotReadyHere, Unready};

use crate::support::{Fixture, http_tool, name, pipeline, profile_on, secret};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn refusal(fixture: &Fixture, pipeline: ResourceId) -> Option<NotReadyHere> {
  fixture
    .workspace
    .not_ready_here(&fixture.root, &fixture.vault, pipeline)
    .expect("readiness")
}

/// A pipeline with one step that runs `tool`.
fn tool_pipeline(tool: ResourceId) -> Pipeline {
  let step = Step {
    key: StepKey::new("deploy").expect("valid key"),
    name: name("Deploy"),
    kind: StepKind::Tool { tool: Some(tool) },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  Pipeline::new(vec![step]).expect("a valid pipeline")
}

fn keys(refused: &NotReadyHere) -> Vec<&'static str> {
  refused.messages().iter().map(|message| message.key).collect()
}

#[test]
fn readiness_start_a_ready_pipeline_is_not_refused() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let program = fixture.program_outside("knowledge.sh");
  let server = fixture.local_server("knowledge", program.to_str().expect("utf-8"));
  let builder = fixture.own("builder", &profile_on(own, &[server]));
  let ready = fixture.own("ship", &pipeline(&[Some(builder)], None));

  assert_eq!(refusal(&fixture, ready), None);
}

#[test]
fn readiness_start_a_program_that_is_gone_is_refused_with_the_server_named() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "claude");
  let gone = fixture.dir_path().join("gone.sh");
  let server = fixture.local_server("knowledge", gone.to_str().expect("utf-8"));
  let builder = fixture.own("builder", &profile_on(own, &[server]));
  let ship = fixture.own("ship", &pipeline(&[Some(builder)], None));

  let refused = refusal(&fixture, ship).expect("not ready here");
  assert_eq!(refused.pipeline(), "ship");
  assert_eq!(
    refused.parts(),
    [Unready::Missing(Missing::LocalMcpProgram {
      server,
      name: McpServerName::new("knowledge").expect("name"),
      problem: LocalProgramProblem::NotFound,
    })]
  );
  let messages = refused.messages();
  assert_eq!(
    keys(&refused),
    ["runs.notReadyHere.lead", "runs.notReadyHere.localProgram.notFound"]
  );
  assert_eq!(messages[1].args, [("server", "knowledge".to_owned())]);
}

#[test]
fn readiness_start_a_tool_secret_without_a_value_is_refused_and_the_value_stays_out() {
  let fixture = Fixture::local();
  let token = fixture.project_secret("DEPLOY_TOKEN", None);
  let tool = fixture.own("deploy", &http_tool(token));
  let ship = fixture.own("ship", &tool_pipeline(tool));

  let refused = refusal(&fixture, ship).expect("not ready here");
  assert_eq!(
    refused.parts(),
    [Unready::Missing(Missing::LocalSecret {
      secret: token,
      name: secret("DEPLOY_TOKEN"),
    })]
  );
  assert_eq!(
    keys(&refused),
    ["runs.notReadyHere.lead", "runs.notReadyHere.localSecret"]
  );

  let other = Fixture::local();
  let set = other.project_secret("DEPLOY_TOKEN", Some("s3cret-value"));
  let tool = other.own("deploy", &http_tool(set));
  let ship = other.own("ship", &tool_pipeline(tool));
  assert_eq!(refusal(&other, ship), None, "a value in the keychain makes it ready");
  assert!(!format!("{refused:?}").contains("s3cret-value"));
}

#[test]
fn readiness_start_a_keychain_that_cannot_say_refuses_and_a_missing_connection_is_the_preflights() {
  struct Locked;
  impl SecretVault for Locked {
    fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
      Err(VaultError::AccessDenied("locked".to_owned()))
    }

    fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
      Err(VaultError::AccessDenied("locked".to_owned()))
    }

    fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
      Ok(())
    }
  }

  let fixture = Fixture::local();
  let token = fixture.project_secret("DEPLOY_TOKEN", Some("a"));
  let tool = fixture.own("deploy", &http_tool(token));
  let ship = fixture.own("ship", &tool_pipeline(tool));
  let refused = fixture
    .workspace
    .not_ready_here(&fixture.root, &Locked, ship)
    .expect("readiness")
    .expect("not ready here");
  assert_eq!(refused.parts(), [Unready::SecretUnreadable(token)]);
  assert_eq!(
    keys(&refused),
    ["runs.notReadyHere.lead", "runs.notReadyHere.secretUnreadable"]
  );

  let gone = fixture.own(
    "builder",
    &profile_on(orchester_core::id::ConnectionId::generate(), &[]),
  );
  let runs_gone = fixture.own("lost", &pipeline(&[Some(gone)], None));
  assert_eq!(
    refusal(&fixture, runs_gone),
    None,
    "a profile whose connection is gone is refused by the preflight with its own reason"
  );
}
