//! What a team start reads before it asks the team server for anything: the pipeline is ready here, with the program
//! paths this computer set, or it is not, and the answer lists each missing part with its fix.

use orchester_core::{connection::AgentProvider, id::ResourceId, run::Answerer, secret::SecretValue};
use orchester_workspace::readiness::{NotReadyPart, ReadinessError, StartReadiness};

use crate::support::{Fixture, MemoryVault, pipeline, profile, program, remote, secret};

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
      &MemoryVault::default(),
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
      &MemoryVault::default(),
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
