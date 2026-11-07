//! What a team start reads before it asks the team server for anything: the pipeline is ready here, with the program
//! paths this computer set, or it is not, and the answer lists each missing part with its fix.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  run::Answerer,
  secret::{SecretValue, SecretVault},
};
use orchester_workspace::readiness::{NotReadyHere, NotReadyPart, ReadinessError, StartReadiness};
use serde_json::Value;

use crate::{
  support::{Fixture, LockedVault, MemoryVault, pipeline, profile, program, remote, remote_two_headers, secret},
  tools::AsksBeforeReading,
};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;
const TOKEN: &str = "fake-value-7Qx2";

#[test]
fn readiness_start_a_ready_pipeline_hands_over_the_program_paths_set_here() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let docs = fixture.team_server("knowledge", program(&["KNOWLEDGE_TOKEN"]));
  let builder = fixture.team_profile("builder", &profile(CLAUDE, &[docs], &[]));
  let delivery = fixture.team_resource("delivery", pipeline(&[Some(builder)], None));
  let work = fixture.workspace.clone();
  work
    .switch_team_mcp_on_here(&fixture.root, docs, true, Answerer::Person)
    .expect("on");
  let path = fixture.program_file("knowledge-server");
  work
    .set_team_mcp_program(&fixture.root, docs, &path, Answerer::Person)
    .expect("program");
  work
    .set_team_secret_value(
      &fixture.root,
      &fixture.vault,
      &secret("KNOWLEDGE_TOKEN"),
      &SecretValue::new(TOKEN.to_owned()).expect("value"),
    )
    .expect("secret");

  let ready = work
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  let StartReadiness::Ready(programs) = ready else {
    panic!("not ready: {ready:?}");
  };
  assert_eq!(programs.path(docs), Some(path.as_path()));
}

#[test]
fn readiness_start_a_pipeline_that_is_not_ready_lists_each_part_with_its_fix_and_no_value() {
  let fixture = Fixture::team();
  let docs = fixture.team_server("knowledge", program(&["KNOWLEDGE_TOKEN"]));
  let issues = fixture.team_server("issues", remote("ISSUES_KEY"));
  let builder = fixture.team_profile("builder", &profile(CLAUDE, &[docs, issues], &[]));
  let delivery = fixture.team_resource("delivery", pipeline(&[Some(builder)], None));
  fixture
    .workspace
    .set_team_secret_value(
      &fixture.root,
      &fixture.vault,
      &secret("ISSUES_KEY"),
      &SecretValue::new(TOKEN.to_owned()).expect("value"),
    )
    .expect("secret");

  let read = fixture
    .workspace
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  let StartReadiness::NotReady(refusal) = read else {
    panic!("ready: {read:?}");
  };
  assert_eq!(refusal.pipeline(), "delivery");
  let keys: Vec<&str> = refusal.messages().iter().map(|message| message.key).collect();
  assert_eq!(
    keys,
    [
      "notReadyHere.refused",
      "notReadyHere.connectionNoneOfKind",
      "notReadyHere.mcpOff",
      "notReadyHere.mcpOff",
      "notReadyHere.mcpProgramNotSet",
      "notReadyHere.secret",
    ]
  );
  let parts = refusal.parts_json();
  assert_eq!(parts[0]["kind"], "connection");
  assert_eq!(parts[0]["name"], "builder");
  assert_eq!(parts[3]["kind"], "mcp_program");
  assert_eq!(parts[3]["problem"]["kind"], "not_set");
  assert_eq!(parts[4]["name"], "KNOWLEDGE_TOKEN");
  assert!(
    refusal
      .parts()
      .iter()
      .all(|part| matches!(part, NotReadyPart::Missing(_)))
  );
  let shown = format!("{refusal:?} {parts:?}");
  assert!(!shown.contains(TOKEN), "a value is never in the answer");
}

#[test]
fn readiness_start_a_servers_secret_is_set_on_the_server_not_on_either_profile_that_chooses_it() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let docs = fixture.team_server("knowledge", program(&["KNOWLEDGE_TOKEN"]));
  let first = fixture.team_profile("builder", &profile(CLAUDE, &[docs], &[]));
  let second = fixture.team_profile("checker", &profile(CLAUDE, &[docs], &[]));
  let delivery = fixture.team_resource("delivery", pipeline(&[Some(first), Some(second)], None));

  let refusal = not_ready(&fixture, &fixture.vault, delivery);
  let owners: Vec<_> = refusal
    .parts()
    .iter()
    .filter_map(|part| match part {
      NotReadyPart::Missing(part) if part.name == "KNOWLEDGE_TOKEN" => Some(part.owner),
      _ => None,
    })
    .collect();
  assert!(!owners.is_empty(), "the secret is listed: {refusal:?}");
  assert!(
    owners.iter().all(|owner| *owner == Some(docs)),
    "the value is set on the server, whichever profile lacks it first: {owners:?}"
  );

  // A secret only a profile names is set on the profile.
  let own = fixture.team_profile("reviewer", &profile(CLAUDE, &[], &["DEPLOY_KEY"]));
  let alone = fixture.team_resource("alone", pipeline(&[Some(own)], None));
  let refusal = not_ready(&fixture, &fixture.vault, alone);
  let deploy = refusal.parts().iter().find_map(|part| match part {
    NotReadyPart::Missing(part) if part.name == "DEPLOY_KEY" => Some(part.owner),
    _ => None,
  });
  assert_eq!(deploy, Some(Some(own)));
}

#[test]
fn readiness_start_a_pipeline_this_computer_has_no_version_of_is_unknown_and_a_local_project_has_no_team_start() {
  let fixture = Fixture::team();
  assert!(matches!(
    fixture
      .workspace
      .team_start_readiness(&fixture.root, &fixture.vault, ResourceId::generate())
      .expect("read"),
    StartReadiness::PipelineUnknown
  ));

  let local = Fixture::local();
  let error = local
    .workspace
    .team_start_readiness(&local.root, &local.vault, ResourceId::generate())
    .expect_err("a local project");
  assert!(matches!(error, ReadinessError::NotATeamProject));
}

/// A pipeline of one profile that chooses a remote server, switched on and given its secret's value in the fixture's
/// keychain, so only what a test changes keeps it from being ready.
fn ready_with_remote(fixture: &Fixture, server: Value) -> ResourceId {
  fixture.connection(CLAUDE, "claude");
  let issues = fixture.team_server("issues", server);
  let builder = fixture.team_profile("builder", &profile(CLAUDE, &[issues], &[]));
  let delivery = fixture.team_resource("delivery", pipeline(&[Some(builder)], None));
  fixture
    .workspace
    .switch_team_mcp_on_here(&fixture.root, issues, true, Answerer::Person)
    .expect("on");
  for name in ["ISSUES_KEY", "ORG_KEY"] {
    fixture
      .workspace
      .set_team_secret_value(
        &fixture.root,
        &fixture.vault,
        &secret(name),
        &SecretValue::new(TOKEN.to_owned()).expect("value"),
      )
      .expect("secret");
  }
  delivery
}

fn not_ready(fixture: &Fixture, vault: &dyn SecretVault, pipeline: ResourceId) -> NotReadyHere {
  let read = fixture
    .workspace
    .team_start_readiness(&fixture.root, vault, pipeline)
    .expect("read");
  let StartReadiness::NotReady(refusal) = read else {
    panic!("ready: {read:?}");
  };
  refusal
}

#[test]
fn readiness_start_a_secret_whose_keychain_value_is_gone_is_missing_though_its_row_is_there() {
  let fixture = Fixture::team();
  let delivery = ready_with_remote(&fixture, remote("ISSUES_KEY"));
  assert!(matches!(
    fixture
      .workspace
      .team_start_readiness(&fixture.root, &fixture.vault, delivery)
      .expect("read"),
    StartReadiness::Ready(_)
  ));

  // The same rows, read where the keychain holds no value.
  let refusal = not_ready(&fixture, &MemoryVault::default(), delivery);
  let keys: Vec<&str> = refusal.messages().iter().map(|message| message.key).collect();
  assert_eq!(keys, ["notReadyHere.refused", "notReadyHere.secret"]);
  assert_eq!(refusal.parts_json()[0]["kind"], "secret");
  assert_eq!(refusal.parts_json()[0]["name"], "ISSUES_KEY");
}

#[test]
fn readiness_start_a_keychain_that_cannot_say_leaves_the_secret_unreadable_not_set() {
  let fixture = Fixture::team();
  let delivery = ready_with_remote(&fixture, remote("ISSUES_KEY"));

  let refusal = not_ready(&fixture, &LockedVault, delivery);
  let keys: Vec<&str> = refusal.messages().iter().map(|message| message.key).collect();
  assert_eq!(keys, ["notReadyHere.refused", "notReadyHere.secretUnreadable"]);
  let parts = refusal.parts_json();
  assert_eq!(parts[0]["kind"], "secret_unreadable");
  assert_eq!(parts[0]["name"], "ISSUES_KEY");
  assert!(!format!("{parts:?}").contains(TOKEN), "a value is never in the answer");
}

#[test]
fn readiness_start_a_remote_server_with_several_secret_headers_is_not_ready_for_it() {
  let fixture = Fixture::team();
  let delivery = ready_with_remote(&fixture, remote_two_headers("ISSUES_KEY", "ORG_KEY"));

  let refusal = not_ready(&fixture, &fixture.vault, delivery);
  let keys: Vec<&str> = refusal.messages().iter().map(|message| message.key).collect();
  assert_eq!(
    keys,
    ["notReadyHere.refused", "notReadyHere.mcpMoreThanOneSecretHeader"]
  );
  let parts = refusal.parts_json();
  assert_eq!(parts[0]["kind"], "mcp_unsupported");
  assert_eq!(parts[0]["why"], "more_than_one_secret_header");
  assert_eq!(parts[0]["name"], "issues");
}

#[test]
fn readiness_start_a_refusal_after_the_server_made_the_work_says_so_in_its_lead() {
  let after = NotReadyHere::after_start("delivery", Vec::new());
  assert_eq!(after.messages()[0].key, "notReadyHere.refusedAfterStart");
}

#[test]
fn readiness_start_a_team_secret_is_asked_for_with_exists_and_never_read() {
  let fixture = Fixture::team();
  let delivery = ready_with_remote(&fixture, remote("ISSUES_KEY"));
  let ready = fixture
    .workspace
    .team_start_readiness(&fixture.root, &AsksBeforeReading(Ok(true)), delivery)
    .expect("read");
  assert!(matches!(ready, StartReadiness::Ready(_)), "{ready:?}");
  not_ready(&fixture, &AsksBeforeReading(Ok(false)), delivery);
}
