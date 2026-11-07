//! The hub's groups: Needs you here, Ready here, Off here.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, ResourceKind},
  run::Answerer,
};
use orchester_workspace::readiness::{Group, ReadinessStanding, Undecided};

use crate::support::{Fixture, name, profile, program, remote};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

#[test]
fn readiness_resources_fall_into_needs_you_ready_and_off() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let off = fixture.team_server("knowledge", program(&[]));
  let on = fixture.team_server("issues", remote("ISSUES_KEY"));
  let undecided = fixture.team_server("notes", remote("NOTES_KEY"));
  fixture
    .workspace
    .switch_team_mcp_on_here(&fixture.root, on, true, Answerer::Person)
    .expect("on");
  fixture
    .workspace
    .switch_team_mcp_on_here(&fixture.root, off, false, Answerer::Person)
    .expect("off");
  let plain = fixture.team_profile("plain", &profile(CLAUDE, &[], &[]));
  let needs_server = fixture.team_profile("needs-server", &profile(CLAUDE, &[off], &[]));

  let read = fixture.read();
  let ids = |group| read.group(group).map(|found| found.resource()).collect::<Vec<_>>();
  assert_eq!(ids(Group::OffHere), [off], "a team server switched off here");
  assert_eq!(
    ids(Group::NeedsYouHere),
    [needs_server, on, undecided],
    "a profile that chooses an off server cannot run, a server that is on still lacks its secret, and one nobody \
     decided on needs the person"
  );
  assert_eq!(ids(Group::Ready), [plain]);

  let kinds: Vec<_> = read.resources().iter().map(|found| found.kind()).collect();
  assert_eq!(
    kinds,
    [
      ResourceKind::AgentProfile,
      ResourceKind::AgentProfile,
      ResourceKind::McpServer,
      ResourceKind::McpServer,
      ResourceKind::McpServer
    ],
    "profiles, then servers"
  );
}

#[test]
fn readiness_an_off_server_still_lists_what_else_it_lacks() {
  let fixture = Fixture::team();
  let server = fixture.team_server("knowledge", program(&[]));
  let undecided = fixture.read();
  let found = undecided.resource(server).expect("listed");
  assert_eq!(found.group(), Group::NeedsYouHere, "nobody decided on it here");
  assert_eq!(found.standing().missing().len(), 2, "off, and no program");

  fixture
    .workspace
    .switch_team_mcp_on_here(&fixture.root, server, false, Answerer::Person)
    .expect("off");
  let read = fixture.read();
  let found = read.resource(server).expect("listed");
  assert_eq!(found.group(), Group::OffHere, "switched off here");
  assert_eq!(found.standing().missing().len(), 2, "off, and no program");
}

#[test]
fn readiness_a_profile_choosing_a_server_the_team_does_not_offer_is_undecided() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let gone = ResourceId::generate();
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[gone], &[]));

  let read = fixture.read();
  let found = read.resource(id).expect("listed");
  assert_eq!(found.group(), Group::NeedsYouHere);
  assert_eq!(
    found.standing(),
    &ReadinessStanding::Undecided(Undecided::ChoosesMcpServer(gone))
  );
}

#[test]
fn readiness_a_profile_version_that_is_not_in_the_teams_form_is_undecided() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  // The server's body of a version cached before the team configuration model is a local body with a connection.
  let legacy = AgentProfile::new(
    name("Legacy"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let id = fixture.team_resource("legacy", legacy);

  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::NotTeamForm)
  );
}
