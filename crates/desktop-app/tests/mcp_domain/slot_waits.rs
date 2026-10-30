//! A ready step that only waits for an agent slot (ORC-403), with the real runner and the fake Codex: two parallel
//! steps on one slot show the wait in `get_run`, `list_runs`, the task's Summary and the Overview, a step whose
//! dependencies have not passed stays as it was, and raising the slots ends it all.
//! `cargo test -p orchester-app --test mcp_domain domain_slot_wait`.

use orchester_app::routes::{OverviewTab, ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{id::ProjectId, project::TaskKey};
use serde_json::{Value, json};

use super::{Served, live, tools_window::text};

/// The step `key` of `read`, a `get_run`'s result.
fn step<'a>(read: &'a Value, key: &str) -> &'a Value {
  read["steps"]
    .as_array()
    .and_then(|steps| steps.iter().find(|step| step["key"] == key))
    .unwrap_or_else(|| panic!("step {key} in {read}"))
}

/// What the window tools report of the element `id`, as text.
fn inspected(served: &mut Served, id: &str) -> String {
  served.ok("lurq_find_by_id", &json!({ "id": id })).to_string()
}

fn go(served: &Served, project: ProjectId, page: ProjectPage) {
  let navigation = served.link.navigation().expect("the window attached its navigation");
  navigation.go(Route::Project(project, page));
}

/// Implement, then Checks and Review side by side, then Report after both; one agent slot. "Keep working" is not their
/// first sentence: a step's prompt shows that of each later step, and the fake would keep Implement working.
fn parallel_steps_on_one_slot(served: &mut Served) {
  served.ok(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Delivery", "steps": [
      { "key": "implement", "name": "Implement", "profile": "Builder", "repository": "write", "instructions": "Append one line." },
      { "key": "checks", "name": "Checks", "profile": "Builder", "repository": "write", "instructions": "Check the work. Keep working.", "runs_after": ["implement"] },
      { "key": "review", "name": "Review", "profile": "Builder", "repository": "write", "instructions": "Review the work. Keep working.", "runs_after": ["implement"] },
      { "key": "report", "name": "Report", "profile": "Builder", "repository": "write", "instructions": "Append one line.", "runs_after": ["checks", "review"] },
    ] }),
  );
  served.ok("publish_pipeline", &json!({ "project": "Kontur", "pipeline": "Delivery" }));
  served.ok("update_project_limits", &json!({ "project": "Kontur", "agent_slots": 1 }));
}

#[test]
fn domain_slot_wait_shows_in_get_run_list_runs_the_summary_and_the_overview() {
  let (mut served, repo, fake) = live::served(true);
  live::set_up(&mut served, &repo, &fake);
  parallel_steps_on_one_slot(&mut served);
  let project: ProjectId = served.ok("get_project", &json!({ "project": "Kontur" }))["project"]["id"]
    .as_str()
    .and_then(|id| id.parse().ok())
    .expect("the project's id");
  let key = live::run(&mut served, "Say hello.");
  let task: TaskKey = key.parse().expect("a task's key");
  let run_one = json!({ "project": "Kontur", "run": "1" });
  served.until("Review to wait for a slot", |served| {
    let read = served.ok("get_run", &run_one);
    step(&read, "review")["status"]["waits_for"] == "agent_slot"
  });

  let read = served.ok("get_run", &run_one);
  assert_eq!(step(&read, "checks")["status"]["state"], "running", "{read}");
  assert_eq!(
    step(&read, "review")["status"],
    json!({ "state": "pending", "waits_for": "agent_slot" }),
    "{read}"
  );
  assert_eq!(
    step(&read, "report")["status"],
    json!({ "state": "pending", "waits_for": null }),
    "Report's dependencies have not passed: {read}"
  );
  let listed = served.ok("list_runs", &json!({ "project": "Kontur" }));
  let detail = listed["runs"][0]["detail"].as_str().expect("the run's detail");
  assert!(detail.ends_with(" · Review waits for an agent slot"), "{detail}");

  go(
    &served,
    project,
    ProjectPage::task(task.clone(), TaskView::of(TaskTab::Summary), false),
  );
  served.until("the rail", |served| {
    served.tree.get_element_by_id("task-step-review").is_some()
  });
  served.until("the chip to say the wait", |served| {
    inspected(served, "task-step-review").contains("WaitsForSlot")
  });
  let report = inspected(&mut served, "task-step-report");
  assert!(report.contains("Waiting") && !report.contains("WaitsForSlot"), "{report}");
  assert!(
    text(&mut served, "task-event-text").contains("Review is ready and starts when an agent slot frees up"),
    "{}",
    text(&mut served, "task-event-text")
  );

  go(&served, project, ProjectPage::Overview(OverviewTab::default()));
  served.until("the Overview to say the wait", |served| {
    text(served, "overview-at-work-slot-waits") == "1 step waits for a slot · raise Agents at once to start it"
  });

  served.ok("update_project_limits", &json!({ "project": "Kontur", "agent_slots": 2 }));
  served.until("Review to start", |served| {
    let read = served.ok("get_run", &run_one);
    step(&read, "review")["status"] == json!({ "state": "running", "waits_for": null })
  });
  served.until("the Overview to drop the wait", |served| {
    served.tree.get_element_by_id("overview-at-work-slot-waits").is_none()
  });
  let listed = served.ok("list_runs", &json!({ "project": "Kontur" }));
  let detail = listed["runs"][0]["detail"].as_str().expect("the run's detail");
  assert!(!detail.contains("agent slot"), "{detail}");
  go(
    &served,
    project,
    ProjectPage::task(task, TaskView::of(TaskTab::Summary), false),
  );
  served.until("the chip to say Working", |served| {
    inspected(served, "task-step-review").contains("Working")
  });
  assert!(!text(&mut served, "task-event-text").contains("agent slot"));
  served.ok("cancel_run", &run_one);
}
