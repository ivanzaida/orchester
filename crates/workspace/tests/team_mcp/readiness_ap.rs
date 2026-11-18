//! Automation fixes what a team MCP server lacks here while a person lets it change agent permissions: it turns the
//! server on, and an absolute program is kept as this computer's path, which the readiness then no longer misses.

use orchester_core::connection::AgentProvider;
use serde_json::json;

use super::readiness::{
  kinds, kontur, listed_server, profile_resource, program_file, program_server, server_resource, team_profile,
};
use crate::{Checkout, team_server::TeamServer};

#[test]
fn team_mcp_readiness_automation_turns_a_team_server_on_and_sets_its_program_while_a_person_lets_it_widen() {
  let server = TeamServer::start(crate::fixture::Config::new("Say hello."));
  let mut ids = None;
  let widens = crate::Standing {
    role: "member",
    widens: true,
  };
  let checkout = Checkout::standing(Some(&server), (&[], widens), |db| {
    let knowledge = server_resource(db, program_server());
    let profile = profile_resource(db, &team_profile(AgentProvider::CodexCli, &[knowledge], &["API_TOKEN"]));
    ids = Some((profile, knowledge));
  });
  let (_profile, knowledge) = ids.expect("prepared");

  let on = kontur(json!({"server": knowledge.to_string(), "enabled": true}));
  let turned = checkout.call("set_mcp_server_enabled", &on).expect("turned on");
  assert_eq!(turned["server"]["enabled"], true, "{turned}");
  assert!(!kinds(&listed_server(&checkout)["readiness"]).contains(&"mcp_off".to_owned()));

  // An absolute program is this computer's path, kept here; the team's version is not published again.
  let file = program_file(&checkout);
  let program = kontur(json!({"server": knowledge.to_string(), "program": file.to_string_lossy()}));
  let kept = checkout.call("update_mcp_server", &program).expect("kept here");
  assert_eq!(kept["program_here"], file.to_string_lossy().as_ref(), "{kept}");
  assert!(kept.get("unchanged").is_none(), "the path was kept: {kept}");
  assert!(!kinds(&listed_server(&checkout)["readiness"]).contains(&"mcp_program".to_owned()));
}
