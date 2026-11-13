//! The MCP tools that make a team pipeline (`share_pipeline`, `duplicate_pipeline`) wait for the team server as the
//! page's save does, and the page's history reads the team server's versions.

use orchester_core::{id::ResourceId, store::ResourceStore, time::Timestamp};
use orchester_store::ResourceSharing;

use super::{Team, checks};

/// `share_pipeline` makes a pipeline of this computer's own, hidden in the team project, a team pipeline on the team
/// server at once: the copy takes the hidden one's slug, which stays this computer's, and nothing waits.
#[test]
fn team_pipeline_save_share_pipeline_makes_the_team_copy_beside_the_hidden_local_one() {
  let team = Team::owner();
  let (local, _) = team
    .db()
    .kept_pipeline_for_test("delivery", "Delivery", &checks(team.tool, "Ours."), Timestamp::now())
    .expect("a pipeline of this computer's own");
  let shared = team.call(
    "share_pipeline",
    serde_json::json!({"project": "Kontur", "pipeline": "Delivery"}),
  );

  assert_eq!(shared["pipeline"]["state"], "team");
  assert_eq!(shared["pipeline"]["slug"], "delivery");
  let copy: ResourceId = shared["pipeline"]["id"]
    .as_str()
    .and_then(|id| id.parse().ok())
    .expect("the copy's id");
  assert_ne!(copy, local, "a new team pipeline");
  let db = team.db();
  assert!(!team.in_database(copy), "the team's copy is not kept in the database");
  assert_eq!(
    db.resource_sharing(local).expect("read"),
    Some(ResourceSharing::NotShared)
  );
  let kept = db.resource(local).expect("read").expect("the hidden one stays");
  assert_eq!(kept.slug().as_str(), "delivery", "it keeps its slug");
  assert_eq!(team.latest(copy), (1, "Ours.".to_owned()));
  assert_eq!(team.on_server(copy), Some((1, "Delivery".to_owned())));
}

/// `duplicate_pipeline` in a team project is not refused as a draft tool: the copy is made on the team server at once
/// and kept here once the server made it (D-043).
#[test]
fn team_pipeline_save_duplicate_pipeline_makes_the_copy_on_the_team_server() {
  let team = Team::owner();
  let source = team.created();

  let copied = team.call(
    "duplicate_pipeline",
    serde_json::json!({"project": "Kontur", "pipeline": "Checks", "name": "Checks copy"}),
  );

  let copy: ResourceId = copied["pipeline"]["id"]
    .as_str()
    .and_then(|id| id.parse().ok())
    .expect("the copy's id");
  assert_ne!(copy, source);
  assert_eq!(team.on_server(copy), Some((1, "Checks copy".to_owned())));
  assert!(!team.in_database(copy), "the copy is not kept in the database");
  assert_eq!(team.latest(copy), (1, "Ours.".to_owned()));
}

/// The page's history is the team server's: every version it numbered, newest first.
#[test]
fn team_pipeline_save_history_reads_the_versions_of_the_team_server() {
  let team = Team::owner();
  let id = team.created();
  team.publish(id, 1, "Changed.").expect("published");

  let read = team.desk.run(team.servers().team_pipeline_history(&team.root, id));

  let history = read.unwrap_or_else(|why| panic!("not read: {}", why.key));
  let numbers: Vec<u32> = history.versions().iter().map(|version| version.number.get()).collect();
  assert_eq!(numbers, [2, 1]);
}
