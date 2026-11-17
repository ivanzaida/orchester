//! A team task's organization through the tools (OA-35): the tasks that block it, its tags, its story points and
//! *Don't merge automatically* go to its team server as a local task's go to its project, named the same way; a task
//! blocked by a task the server does not hold Done is not started, as a local blocked task is not.

use orchester_core::id::{TagId, TaskId};
use serde_json::json;

use crate::{Checkout, tools::server};

#[test]
fn team_mcp_a_team_tasks_organization_goes_to_its_server() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let (blocker, tag) = (TaskId::generate(), TagId::generate());
  {
    let mut seen = seen.lock();
    seen.more_tasks = vec![json!({"id": blocker, "number": 8, "key": "KON-8", "title": "Say goodbye"})];
    seen.tags = vec![json!({"id": tag, "name": "Backend"})];
  }

  // A member's change of them is the server's to refuse; what was sent is what the local tools take, resolved there.
  let update = json!({"project": "Kontur", "key": "KON-7", "blocked_by": [{"task": "kon-8"}], "tags": ["backend"],
    "no_automatic_merge": true});
  assert_eq!(checkout.refused("update_task", &update), "not_admin");
  let sent = seen.lock().edited_changes.clone();
  assert_eq!(sent["blocking"], json!([{"task": blocker, "gate": "merge"}]), "{sent}");
  assert_eq!(sent["tags"], json!([tag]), "{sent}");
  assert_eq!(sent["no_automatic_merge"], true, "{sent}");
  assert_eq!(seen.lock().tag_creates, 0, "the team's own tag, matched ignoring case");

  let points = json!({"project": "Kontur", "key": "KON-7", "story_points": 5});
  assert_eq!(checkout.refused("update_task", &points), "not_admin");
  let sent = seen.lock().edited_changes.clone();
  assert_eq!(sent["story_points"], 5, "{sent}");
  assert!(
    sent["tags"].is_null() && sent["blocking"].is_null(),
    "the points alone: {sent}"
  );
  let off = json!({"project": "Kontur", "key": "KON-7", "story_points": 4});
  assert_eq!(checkout.refused("update_task", &off), "invalid_story_points");

  // A tag the team lacks is made on its server first, as the picker makes it; a member's is refused, and no change of
  // the task is sent.
  let edits = seen.lock().task_edits;
  let new_tag = json!({"project": "Kontur", "key": "KON-7", "tags": ["Frontend"]});
  assert_eq!(checkout.refused("update_task", &new_tag), "not_admin");
  let made = seen.lock().tag_creates;
  assert_eq!((made, seen.lock().task_edits), (1, edits));
  let unknown = json!({"project": "Kontur", "key": "KON-7", "blocked_by": [{"task": "KON-99"}]});
  assert_eq!(checkout.refused("update_task", &unknown), "task_not_found");
  assert_eq!(
    seen.lock().task_edits,
    edits,
    "nothing sent for a task the server does not have"
  );

  // A new task carries its organization in `task.create`; its points follow as a change of their own.
  let create = json!({"project": "Kontur", "title": "Say goodbye again", "blocked_by": [{"task": "KON-8"}],
    "tags": ["Backend"], "no_automatic_merge": true, "story_points": 3});
  assert_eq!(checkout.refused("create_task", &create), "not_admin");
  let content = seen.lock().created_content.clone();
  assert_eq!(
    content["blocking"],
    json!([{"task": blocker, "gate": "merge"}]),
    "{content}"
  );
  assert_eq!(content["tags"], json!([tag]), "{content}");
  assert_eq!(content["no_automatic_merge"], true, "{content}");
  assert!(content.get("story_points").is_none(), "{content}");
}

#[test]
fn team_mcp_a_team_task_waits_for_the_tasks_that_block_it() {
  let (server, seen) = server();
  let checkout = Checkout::new(Some(&server), &[]);
  let blocker = TaskId::generate();
  {
    let mut seen = seen.lock();
    seen.more_tasks = vec![json!({"id": blocker, "number": 8, "key": "KON-8", "workflow": "todo", "blocking": []})];
    seen.task_fields = json!({"blocking": [{"task": blocker, "gate": "merge"}]});
  }
  let run = json!({"project": "Kontur", "key": "KON-7"});
  assert_eq!(checkout.refused("run_task", &run), "invalid_organization");
  assert!(server.state().claim.is_none(), "nothing claimed");

  // Once the server holds it Done, the blocking task holds nothing back.
  seen.lock().more_tasks[0]["workflow"] = json!("done");
  let started = checkout.call("run_task", &run);
  let code = started.err().map(|error| error["error"]["code"].clone());
  assert_ne!(code, Some(json!("invalid_organization")), "{code:?}");
}
