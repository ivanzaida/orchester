//! A local run uses a team MCP server through the profile versions it pins, as an archive reads it: the used-by list
//! is the runs the archive stops (`pins` in `sharing/sync/archive.rs`).

use orchester_core::id::ResourceId;
use orchester_workspace::used_by::used_by;

use super::support::{Fixture, at, chosen};

#[test]
fn used_by_local_lists_a_run_whose_pinned_profile_chooses_the_team_mcp_server() {
  let (knowledge, other) = (ResourceId::generate(), ResourceId::generate());
  let mut fixture = Fixture::choosing(&[chosen(knowledge)]);
  fixture.keep_team_server(knowledge, "Knowledge");
  fixture.keep_team_server(other, "Other");
  let task = fixture.task("Search", at(10));
  let run = fixture.started_run(task, at(11));
  let runtime = tokio::runtime::Runtime::new().expect("runtime");

  let chosen_by = runtime
    .block_on(used_by(fixture.repo.path(), None, knowledge))
    .expect("the server's users");
  assert_eq!(
    chosen_by.runs.iter().map(|used| used.run).collect::<Vec<_>>(),
    [run.id()],
    "the run pins no server itself; its profile version chooses this one"
  );
  assert_eq!(chosen_by.runs[0].task_key, "ORC-1");
  let unchosen = runtime
    .block_on(used_by(fixture.repo.path(), None, other))
    .expect("the server's users");
  assert!(unchosen.runs.is_empty(), "no pinned profile chooses the other server");
}
