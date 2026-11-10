//! A pipeline is ready when everything its steps run, and its integrator, is.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{CommandAction, CommandProgram, Tool},
  sharing::{ConnectionProblem, Missing},
};
use orchester_workspace::readiness::{ReadinessStanding, Undecided};

use crate::support::{Fixture, pipeline, profile};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;
const CODEX: AgentProvider = AgentProvider::CodexCli;

#[test]
fn readiness_a_pipeline_asks_what_its_profiles_and_integrator_ask() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let builder = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));
  let reviewer = fixture.team_profile("reviewer", &profile(CODEX, &[], &[]));
  let integrator = fixture.team_profile("integrator", &profile(CODEX, &[], &[]));
  let ready = fixture.team_resource("ready", &pipeline(&[Some(builder), Some(builder)], None));
  let blocked = fixture.team_resource("blocked", &pipeline(&[Some(builder), Some(reviewer)], Some(integrator)));

  let read = fixture.read();
  assert_eq!(
    read.pipeline(ready).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
  let blocked = read.pipeline(blocked).expect("listed");
  assert_eq!(
    blocked.standing().missing(),
    [
      Missing::Connection {
        profile: reviewer,
        cli: CODEX,
        problem: ConnectionProblem::NoneOfKind
      },
      Missing::Connection {
        profile: integrator,
        cli: CODEX,
        problem: ConnectionProblem::NoneOfKind
      },
    ],
    "the missing part of each profile, once"
  );

  fixture.connection(CODEX, "codex");
  assert_eq!(
    fixture.read().pipeline(blocked.pipeline()).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}

#[test]
fn readiness_a_pipeline_running_a_resource_this_computer_has_no_version_of_is_undecided() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let builder = fixture.team_profile("builder", &profile(CLAUDE, &[], &[]));
  let unknown = ResourceId::generate();
  let id = fixture.team_resource("odd", &pipeline(&[Some(builder), Some(unknown)], None));

  assert_eq!(
    fixture.read().pipeline(id).expect("listed").standing(),
    &ReadinessStanding::Undecided(Undecided::RunsUnknown(unknown))
  );
}

#[test]
fn readiness_steps_without_a_profile_yet_and_a_pipeline_of_tools_ask_nothing() {
  let fixture = Fixture::team();
  let tool = fixture.team_resource(
    "check",
    &Tool::command(
      "",
      CommandAction::new(CommandProgram::new("cargo").expect("program"), vec![]),
    ),
  );
  let unassigned = fixture.team_resource("draftish", &pipeline(&[None], None));
  assert_eq!(
    fixture.read().pipeline(unassigned).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
  assert_eq!(
    fixture.read().resource(tool).expect("listed").standing(),
    &ReadinessStanding::Ready
  );
}
