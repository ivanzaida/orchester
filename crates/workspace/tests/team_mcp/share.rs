//! `share_pipeline` (`docs/architecture/server.md` row 80): an Admin makes one of their own local pipelines, hidden in
//! a team project, a team pipeline, its newest version the new team pipeline's first, waiting here until the server
//! took it; refused for a member, in a local project, without a person's switch, and while its steps run what is not
//! the team's.

use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{
    AgentProfile, DeliverMode, DeliverStep, MergeMethod, ModelId, PromptText, ProviderSettings, Resource, ResourceKind,
    Slug,
  },
  sharing::TaskRevision,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{Checkout, Standing, fixture, tools::server};

/// A pipeline of this computer's own, Delivery, published before the project was linked, its step run by `profile`.
fn own_delivery(db: &mut ProjectDb, profile: ResourceId) -> ResourceId {
  let body = fixture::one_step(profile, "Say hello.");
  db.published_pipeline_for_test("delivery", "Delivery", body.body(), Timestamp::now())
    .expect("a pipeline")
    .0
}

/// An agent profile of the team's, as sync keeps it.
fn team_profile(db: &mut ProjectDb) -> ResourceId {
  let id = ResourceId::generate();
  let resource = Resource::new(
    id,
    ResourceKind::AgentProfile,
    Slug::new("builder").expect("slug"),
    Name::new("Builder").expect("name"),
    Timestamp::now(),
  );
  db.keep_shared_resource(&resource, TaskRevision::FIRST)
    .expect("the team's profile");
  id
}

fn listed(checkout: &Checkout) -> Vec<String> {
  let listed = checkout
    .call("list_pipelines", &json!({"project": "Kontur"}))
    .expect("listed");
  listed["pipelines"]
    .as_array()
    .expect("pipelines")
    .iter()
    .filter_map(|pipeline| pipeline["id"].as_str().map(str::to_owned))
    .collect()
}

const SHARE: &str = "share_pipeline";

#[test]
fn team_mcp_an_admin_shares_a_pipeline_of_their_own_for_the_team() {
  let (server, _seen) = server();
  let mut local = None;
  let admin = Standing {
    role: "owner",
    widens: true,
  };
  let checkout = Checkout::standing(Some(&server), (&[], admin), |db| {
    let profile = team_profile(db);
    local = Some(own_delivery(db, profile));
  });
  let local = local.expect("made");
  assert!(listed(&checkout).is_empty(), "a team project hides its own");

  let shared = checkout
    .call(SHARE, &json!({"project": "Kontur", "pipeline": "Delivery"}))
    .expect("shared");
  let copy = shared["pipeline"]["id"].as_str().expect("an id").to_owned();
  assert_ne!(copy, local.to_string(), "a new team pipeline");
  assert_eq!(shared["pipeline"]["slug"], "delivery");
  // This computer is not signed in to the fake server's account: the version waits to be sent, never shown as sent.
  assert_eq!(shared["pipeline"]["state"], "waiting");
  assert_eq!(listed(&checkout), std::slice::from_ref(&copy), "offered as the team's");
  let db = checkout.db();
  let waiting: Vec<String> = db
    .waiting_publishes()
    .expect("read")
    .iter()
    .map(|waiting| waiting.resource.to_string())
    .collect();
  assert_eq!(waiting, [copy]);
  let kept = db.resource(local).expect("read").expect("kept");
  assert_eq!(
    kept.slug().as_str(),
    "delivery",
    "the local one is kept as it is, with its slug"
  );
}

#[test]
fn team_mcp_share_pipeline_is_refused_for_a_member_without_the_switch_locally_and_for_local_steps() {
  let (server, _seen) = server();
  let args = json!({"project": "Kontur", "pipeline": "Delivery"});
  let with_team_profile = |db: &mut ProjectDb| {
    let profile = team_profile(db);
    own_delivery(db, profile);
  };

  let member = Checkout::prepared(Some(&server), &[], with_team_profile);
  assert_eq!(member.refused(SHARE, &args), "not_admin");
  let switch_off = Standing {
    role: "owner",
    widens: false,
  };
  let admin = Checkout::standing(Some(&server), (&[], switch_off), with_team_profile);
  assert_eq!(admin.refused(SHARE, &args), "person_only");
  assert!(
    admin.db().waiting_publishes().expect("read").is_empty(),
    "nothing copied"
  );
  let local = Checkout::prepared(None, &[], with_team_profile);
  assert_eq!(local.refused(SHARE, &args), "team_project_required");

  let switch_on = Standing {
    role: "owner",
    widens: true,
  };
  let own_profile = Checkout::standing(Some(&server), (&[], switch_on), |db| {
    own_delivery(db, ResourceId::generate());
  });
  assert_eq!(own_profile.refused(SHARE, &args), "team_resources_required");
  assert!(own_profile.db().waiting_publishes().expect("read").is_empty());
}

/// The Deliver step's integrator is a profile too: one that only this computer keeps would leave the team's copy
/// naming an id the server does not have, and the copy would wait for ever.
#[test]
fn team_mcp_share_pipeline_is_refused_when_the_integrator_is_a_profile_of_this_computer() {
  let (server, _seen) = server();
  let args = json!({"project": "Kontur", "pipeline": "Delivery"});
  let switch_on = Standing {
    role: "owner",
    widens: true,
  };
  let checkout = Checkout::standing(Some(&server), (&[], switch_on), |db| {
    let profile = team_profile(db);
    let integrator = db
      .create_resource(
        Slug::new("scratch").expect("slug"),
        Name::new("Scratch").expect("name"),
        &AgentProfile::new(
          Name::new("Scratch").expect("name"),
          String::new(),
          PromptText::new("Merge.").expect("prompt"),
          ConnectionId::generate(),
          ModelId::new("haiku").expect("model"),
          ProviderSettings::default(),
        ),
        Timestamp::now(),
      )
      .expect("a profile")
      .id();
    let deliver = DeliverStep::new(DeliverMode::AutoMerge, MergeMethod::Squash, Some(integrator), true);
    let body = fixture::one_step(profile, "Say hello.")
      .body()
      .clone()
      .with_deliver(Some(deliver));
    db.published_pipeline_for_test("delivery", "Delivery", &body, Timestamp::now())
      .expect("a pipeline");
  });

  let refused = checkout
    .call(SHARE, &args)
    .expect_err("the integrator is not the team's");

  assert_eq!(refused["error"]["code"], "team_resources_required", "{refused}");
  assert!(
    checkout.db().waiting_publishes().expect("read").is_empty(),
    "nothing was copied"
  );
}
