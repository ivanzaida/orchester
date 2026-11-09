//! What an archive of a team resource leaves on this computer (`docs/design/team-configuration.md`, edge case 8, and
//! A22 `pUu78`): a profile that still names the archived MCP server, tool or instruction cannot run, so Run again is
//! refused as not ready here until an admin restores it or the profile stops naming it. A skill is the exception: a run
//! does not refuse an archived one.

use orchester_core::{
  connection::AgentProvider,
  id::{ResourceId, SecretId},
  run::Answerer,
};
use orchester_workspace::readiness::StartReadiness;

use crate::support::{Fixture, http_tool, instruction, pipeline, profile, program, skill};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

#[test]
fn readiness_archived_run_again_is_refused_while_a_profile_still_chooses_the_archived_server() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let docs = fixture.team_server("knowledge", program(&[]));
  let builder = fixture.team_profile("builder", &profile(CLAUDE, &[docs], &[]));
  let delivery = fixture.team_resource("delivery", &pipeline(&[Some(builder)], None));
  let work = fixture.workspace.clone();
  work
    .switch_team_mcp_on_here(&fixture.root, docs, true, Answerer::Person)
    .expect("on");
  let path = fixture.program_file("knowledge-server");
  work
    .set_team_mcp_program(&fixture.root, docs, &path, Answerer::Person)
    .expect("program");
  let ready = work
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  assert!(
    matches!(ready, StartReadiness::Ready(_)),
    "ready before the archive: {ready:?}"
  );

  // The server's archive reaches this computer through its event stream, which removes the resource here.
  fixture.archive(docs, true);
  let read = work
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  let StartReadiness::NotReady(refusal) = read else {
    panic!("Run again went on with Builder choosing an archived server: {read:?}");
  };
  let keys: Vec<&str> = refusal.messages().iter().map(|message| message.key).collect();
  assert!(
    keys.contains(&"notReadyHere.undecidedChoosesArchivedMcpServer"),
    "the refusal says Builder chooses a server the team does not offer: {keys:?}"
  );

  // An admin restores it: the server's `resource.archived` with `archived` false brings it back.
  fixture.archive(docs, false);
  let again = work
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  assert!(
    matches!(again, StartReadiness::Ready(_)),
    "ready once restored: {again:?}"
  );
}

/// A delivery pipeline of a team whose Builder names `tool` and `instruction` besides a ready server; the server is on
/// here with its program.
fn team_with_listed(fixture: &Fixture, tool: ResourceId, instruction: ResourceId) -> ResourceId {
  let docs = fixture.team_server("knowledge", program(&[]));
  let listed = profile(CLAUDE, &[docs], &[])
    .with_tools(vec![tool])
    .expect("a tool")
    .with_instruction_resources(vec![instruction])
    .expect("an instruction");
  let builder = fixture.team_profile("builder", &listed);
  let delivery = fixture.team_resource("delivery", &pipeline(&[Some(builder)], None));
  let work = fixture.workspace.clone();
  work
    .switch_team_mcp_on_here(&fixture.root, docs, true, Answerer::Person)
    .expect("on");
  let path = fixture.program_file("knowledge-server");
  work
    .set_team_mcp_program(&fixture.root, docs, &path, Answerer::Person)
    .expect("program");
  delivery
}

fn start_keys(fixture: &Fixture, delivery: ResourceId) -> Option<Vec<&'static str>> {
  let read = fixture
    .workspace
    .team_start_readiness(&fixture.root, &fixture.vault, delivery)
    .expect("read");
  match read {
    StartReadiness::Ready(_) => None,
    StartReadiness::NotReady(refusal) => Some(refusal.messages().iter().map(|message| message.key).collect()),
    other => panic!("neither ready nor not ready: {other:?}"),
  }
}

#[test]
fn readiness_archived_run_again_is_refused_while_a_profile_still_names_the_archived_tool() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let tool = fixture.team_resource("deploy", &http_tool(SecretId::generate()));
  let guide = fixture.team_resource("standards", &instruction());
  let delivery = team_with_listed(&fixture, tool, guide);
  assert_eq!(start_keys(&fixture, delivery), None, "ready before the archive");

  fixture.archive(tool, true);
  let keys = start_keys(&fixture, delivery).expect("refused");
  assert!(
    keys.contains(&"notReadyHere.undecidedUsesArchivedTool"),
    "Run again says Builder names a tool the team archived: {keys:?}"
  );

  fixture.archive(tool, false);
  assert_eq!(start_keys(&fixture, delivery), None, "ready once the tool is restored");
}

#[test]
fn readiness_archived_run_again_is_refused_while_a_profile_still_names_the_archived_instruction() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let tool = fixture.team_resource("deploy", &http_tool(SecretId::generate()));
  let guide = fixture.team_resource("standards", &instruction());
  let delivery = team_with_listed(&fixture, tool, guide);

  fixture.archive(guide, true);
  let keys = start_keys(&fixture, delivery).expect("refused");
  assert!(
    keys.contains(&"notReadyHere.undecidedUsesArchivedInstruction"),
    "Run again says Builder names an instruction the team archived: {keys:?}"
  );

  fixture.archive(guide, false);
  assert_eq!(
    start_keys(&fixture, delivery),
    None,
    "ready once the instruction is restored"
  );
}

#[test]
fn readiness_archived_a_skill_does_not_stop_a_profile_naming_one() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let review = fixture.team_resource("review", &skill());
  let listed = profile(CLAUDE, &[], &[]).with_skills(vec![review]).expect("a skill");
  let builder = fixture.team_profile("builder", &listed);
  let delivery = fixture.team_resource("delivery", &pipeline(&[Some(builder)], None));

  fixture.archive(review, true);
  assert_eq!(start_keys(&fixture, delivery), None);
}
