//! A team project's agent profiles through the tools, so through the Orchestrator chat: `list_profiles` and
//! `get_profile` read the team's profiles from its configuration, and `save_profile_draft` publishes a profile's next
//! version on the team server at once, made on the version it was read from, with the team's MCP servers named and the
//! profile's secrets kept. What it widens is a person's decision, as for a local profile. The chat's MCP server tools
//! change the team's servers through the app's own tools, while it drives or in a turn the person's message started.

use orchester_core::{connection::AgentProvider, id::ResourceId};
use serde_json::{Value, json};

use super::{
  chat_task::{asked, chat_call},
  mcp_servers::{Answers, Seen, docs, start},
  readiness::{profile_resource, program_server, server_resource, team_profile},
};
use crate::{Checkout, Standing};

/// A team whose Builder profile needs the team secret `API_TOKEN` and chooses no MCP server, with the team's
/// `knowledge` server, on a computer signed in as an owner; `widens` is whether a person lets automation change agent
/// permissions.
fn team(
  widens: bool,
) -> (
  crate::team_server::TeamServer,
  std::sync::Arc<parking_lot::Mutex<Seen>>,
  Checkout,
  ResourceId,
  ResourceId,
) {
  let (server, seen) = start(Answers::default());
  let mut ids = None;
  let owner = Standing { role: "owner", widens };
  let checkout = Checkout::build(
    Some(&server),
    (&[], owner),
    |db| {
      let knowledge = server_resource(db, program_server());
      let profile = profile_resource(db, &team_profile(AgentProvider::CodexCli, &[], &["API_TOKEN"]));
      ids = Some((profile, knowledge));
    },
    true,
  );
  let (profile, knowledge) = ids.expect("prepared");
  (server, seen, checkout, profile, knowledge)
}

fn save(profile: ResourceId, servers: &[&str]) -> Value {
  json!({
    "project": "Kontur", "profile": profile.to_string(), "role": "Builder", "account": "default", "cli": "codex",
    "model": "gpt-5.5", "instructions": "Build.", "mcp_servers": servers,
  })
}

#[test]
fn team_mcp_profiles_a_team_profile_is_listed_and_opened_from_the_team() {
  let (_server, _seen, checkout, profile, _) = team(true);

  let opened = checkout
    .call("get_profile", &json!({"project": "Kontur", "profile": "builder"}))
    .expect("opened by its role");
  let shown = &opened["profile"];
  assert_eq!(shown["id"], profile.to_string(), "{opened}");
  assert_eq!(shown["team"], true);
  assert_eq!(shown["revision"], "1");
  assert_eq!(shown["secrets"], json!(["API_TOKEN"]));
  assert_eq!(shown["state"]["published_version"], 1);

  let listed = checkout
    .call("list_profiles", &json!({"project": "Kontur"}))
    .expect("listed");
  let ids: Vec<&str> = listed["profiles"]
    .as_array()
    .expect("profiles")
    .iter()
    .filter_map(|profile| profile["id"].as_str())
    .collect();
  assert_eq!(
    ids.iter().filter(|id| **id == profile.to_string()).count(),
    1,
    "{listed}"
  );
}

#[test]
fn team_mcp_profiles_a_save_publishes_the_next_version_with_the_teams_server_and_secrets() {
  let (_server, seen, checkout, profile, knowledge) = team(true);

  let saved = checkout
    .call("save_profile_draft", &save(profile, &["knowledge"]))
    .expect("published");

  assert_eq!(saved["profile"]["revision"], "2", "{saved}");
  assert_eq!(
    saved["profile"]["mcp_servers"][0]["id"],
    knowledge.to_string(),
    "{saved}"
  );
  let seen = seen.lock();
  assert_eq!(seen.calls, ["publish"], "published at once, no draft");
  let published = &seen.published[0];
  assert_eq!(published["based_on"], 1);
  assert_eq!(
    published["body"]["mcp_servers"],
    json!([knowledge.to_string()]),
    "{published}"
  );
  assert_eq!(
    published["body"]["secrets"],
    json!(["API_TOKEN"]),
    "the team's secrets are kept"
  );
  drop(seen);

  // publish_profile has nothing left to publish: it answers the profile as it stands.
  let answered = checkout
    .call(
      "publish_profile",
      &json!({"project": "Kontur", "profile": profile.to_string()}),
    )
    .expect("answered");
  assert_eq!(answered["profile"]["revision"], "2", "{answered}");
}

#[test]
fn team_mcp_profiles_a_save_that_adds_a_server_is_a_persons_unless_automation_may_widen() {
  let (_server, seen, checkout, profile, _) = team(false);

  let refused = checkout
    .call("save_profile_draft", &save(profile, &["knowledge"]))
    .expect_err("widens");

  assert_eq!(refused["error"]["code"], "person_only", "{refused}");
  assert!(seen.lock().calls.is_empty(), "nothing was sent");

  // Narrowing nothing and widening nothing is taken.
  checkout
    .call("save_profile_draft", &save(profile, &[]))
    .expect("published");
  assert_eq!(seen.lock().calls, ["publish"]);
}

#[test]
fn team_mcp_profiles_the_chat_adds_a_team_mcp_server_through_the_apps_tool() {
  let (_server, seen, checkout, ..) = team(true);
  checkout.db().set_orchestrator_drives(true).expect("drives");
  let scope = asked(&checkout);
  let mut args = docs();
  args.as_object_mut().expect("an object").remove("project");

  let made = chat_call(&checkout, &scope, "mcp_add_server", &args).expect("made");

  assert_eq!(made["server"]["published_version"], 1, "{made}");
  assert_eq!(seen.lock().calls, ["create"], "created on the team server, no card");
}

#[test]
fn team_mcp_profiles_a_team_mcp_change_waits_for_the_person_in_a_turn_nobody_asked_for() {
  let (_server, seen, checkout, ..) = team(true);
  let mut scope = asked(&checkout);
  scope.on_its_own = true;
  let mut args = docs();
  args.as_object_mut().expect("an object").remove("project");

  let refused = chat_call(&checkout, &scope, "mcp_add_server", &args).expect_err("waits");

  assert_eq!(refused.code(), "waits_for_person", "{refused:?}");
  assert!(seen.lock().calls.is_empty(), "nothing was sent");
}
