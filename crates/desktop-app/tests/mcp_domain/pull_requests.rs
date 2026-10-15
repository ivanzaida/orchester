//! `list_pull_requests` and `get_pull_request` read the pull requests the Pull requests page lists: the linked
//! repository's open and closed ones as the last sync stored them, each with its state, review, checks, the task
//! and run whose branch it is, its app `link` and its GitHub `url`; `refresh` syncs first through the fake `gh` of
//! the `pull-requests` scenario. Without a project they answer for every project.

use serde_json::{Value, json};

use super::Served;

fn numbers(listed: &Value) -> Vec<u64> {
  listed["pull_requests"]
    .as_array()
    .expect("a list")
    .iter()
    .map(|item| item["number"].as_u64().expect("a number"))
    .collect()
}

#[test]
fn domain_pull_requests_are_listed_with_their_task_and_links() {
  let (mut served, seeded) = Served::seeded("pull-requests");
  let project = seeded.project.to_string();
  let all = served.ok("list_pull_requests", &json!({}));
  assert_eq!(numbers(&all), [142, 137, 139, 136, 134, 133, 131, 128, 127, 125]);
  let repository = &all["repositories"][0];
  assert_eq!(repository["repository"], "github.com/ivanzaida/kontur");
  assert_eq!(repository["sync"], "synced", "{repository}");
  assert!(repository["attempted_at"].is_number(), "{repository}");
  assert_eq!(repository["paused_until"], Value::Null);
  assert_eq!(repository["link"], format!("/project/{project}/pull-requests"));
  let review = served.ok(
    "list_pull_requests",
    &json!({ "project": "Kontur", "tab": "needs_review" }),
  );
  assert_eq!(numbers(&review), [142, 136]);
  let first = &review["pull_requests"][0];
  assert_eq!(first["state"], "open");
  assert_eq!(first["review"], "review_required");
  assert_eq!(first["checks"]["passed"], 5);
  assert_eq!(first["checks"]["running"], 1);
  assert_eq!(first["link"], format!("/project/{project}/pull-requests/142"));
  assert_eq!(first["url"], "https://github.com/ivanzaida/kontur/pull/142");
  assert_eq!(first["project"]["name"], "Kontur");
  let task = &first["task"];
  assert_eq!(task["title"], "Add retry to webhook sender");
  assert_eq!(task["agent_working"], true);
  let key = task["key"].as_str().expect("a key");
  assert_eq!(task["link"], format!("/project/{project}/tasks/{key}/summary"));
  let found = served.ok("list_pull_requests", &json!({ "project": "Kontur", "search": key }));
  assert_eq!(numbers(&found), [142]);
}

#[test]
fn domain_one_pull_request_is_read_by_number_after_a_sync() {
  let (mut served, _) = Served::seeded("pull-requests");
  let merged = served.ok(
    "get_pull_request",
    &json!({ "project": "Kontur", "number": 131, "refresh": true }),
  );
  assert_eq!(merged["state"], "merged");
  assert_eq!(merged["review"], "approved");
  assert!(merged["merged_at"].is_number(), "{merged}");
  assert_eq!(merged["from_fork"], false);
  assert_eq!(merged["repository"]["sync"], "synced");
  let none = served.ok("get_pull_request", &json!({ "project": "Kontur", "number": 127 }));
  assert_eq!(none["task"], Value::Null);
  assert_eq!(none["checks"]["passed"], 0);
  let unsynced = served.refused("get_pull_request", &json!({ "project": "Kontur", "number": 9999 }));
  assert_eq!(unsynced["code"], "pull_request_not_synced", "{unsynced}");
  let missing = served.refused(
    "get_pull_request",
    &json!({ "project": "Kontur", "number": 9999, "refresh": true }),
  );
  assert_eq!(missing["code"], "pull_request_not_found", "{missing}");
  let everywhere = served.refused("list_pull_requests", &json!({ "refresh": true }));
  assert_eq!(everywhere["code"], "invalid_arguments", "{everywhere}");
}

#[test]
fn domain_pull_requests_of_a_project_without_a_repository_are_none() {
  let (mut served, _) = Served::seeded("tasks");
  let listed = served.ok("list_pull_requests", &json!({ "project": "Kontur" }));
  assert_eq!(numbers(&listed), Vec::<u64>::new());
  assert_eq!(listed["repositories"][0]["repository"], Value::Null);
}
