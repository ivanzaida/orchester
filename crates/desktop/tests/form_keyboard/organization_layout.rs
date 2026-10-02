use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{
    TaskDetails,
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{harness::Desktop, task_run::set_text};

fn seed(repo: &std::path::Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let desktop = db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
  let ux = db.create_project_tag(Name::new("UX").expect("name")).expect("tag");
  let details = TaskDetails {
    title: Name::new("Title above tags").expect("title"),
    brief: String::new(),
    acceptance_criteria: vec![],
    pipeline: None,
    limits: None,
  };
  let tasks = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![details.clone(), details],
      Timestamp::now(),
    )
    .expect("tasks");
  for (task, tags) in tasks.iter().zip([vec![desktop.id()], vec![ux.id()]]) {
    db.set_task_organization(
      task.id(),
      &TaskOrganization::default().with_tags(tags),
      ChangeActor::Person,
      Timestamp::now(),
    )
    .expect("organization");
  }
}

#[test]
fn organization_title_keeps_line_after_resize_without_navigation() {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  for width in [1440, 960, 1440, 1440] {
    desktop.tree.resize(width, 960);
    desktop.pass();
    if width == 1440 {
      for key in ["KON-1", "KON-2"] {
        let title = desktop.bounds(&format!("task-row-title-{key}"));
        let row = desktop.bounds(&format!("task-row-{key}"));
        assert!(title.height > 0.0 && title.width > 0.0, "title retains its own line");
        assert!(
          title.y >= row.y && title.y + title.height <= row.y + row.height,
          "title is inside the visible row"
        );
      }
    }
  }
}

#[test]
fn organization_search_and_any_all_tag_filter_preserve_selected_values() {
  let mut desktop = Desktop::with_seeded_project(seed);
  let tags = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags");
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("filter-tags");
  desktop.wait_for("filter-tag-search");
  set_text(&mut desktop, "filter-tag-search", "UX");
  let ux = tags.iter().find(|tag| tag.name().as_str() == "UX").expect("UX");
  let other = tags
    .iter()
    .find(|tag| tag.name().as_str() == "Desktop")
    .expect("Desktop");
  assert!(
    desktop
      .tree
      .get_element_by_id(&format!("filter-tag-{}", other.id()))
      .is_none()
  );
  desktop.press(&format!("filter-tag-{}", ux.id()));
  set_text(&mut desktop, "filter-tag-search", "");
  desktop.press(&format!("filter-tag-{}", other.id()));
  desktop
    .tree
    .get_element_by_id_mut("filter-tag-match")
    .and_then(|element| element.as_select())
    .expect("mode")
    .commit(0);
  desktop.pass();
  desktop.key("Escape", "Escape");
  assert!(desktop.tree.get_element_by_id("task-row-KON-1").is_none());
  assert!(desktop.tree.get_element_by_id("task-row-KON-2").is_none());
  desktop.press("filter-tags");
  desktop.wait_for("filter-tag-match");
  desktop
    .tree
    .get_element_by_id_mut("filter-tag-match")
    .and_then(|element| element.as_select())
    .expect("mode")
    .commit(1);
  desktop.pass();
  desktop.key("Escape", "Escape");
  desktop.wait_for("task-row-KON-1");
  desktop.wait_for("task-row-KON-2");
  assert!(crate::github::shows(&mut desktop, "filter-tags", "Tags · 2"));
}

#[test]
fn organization_overview_rows_span_the_page_and_keep_run_tabs_reachable() {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("task-row-KON-1");
  let row = desktop.bounds("task-row-KON-1");
  let list = desktop.bounds("overview-tasks");
  assert!(row.width > 1000.0, "overview rows span the page");
  assert!(row.height > 0.0 && list.height >= row.height);
  assert!(desktop.tree.get_element_by_id("overview-tab-failed").is_none());
  assert!(desktop.tree.get_element_by_id("overview-tokens").is_none());
  // Expanding the extra execution controls preserves the existing routes and attention actions.
  desktop.click("overview-insights");
  desktop.wait_for("overview-tab-failed");
  desktop.click("overview-tab-failed");
  desktop.wait_for("overview-tab-all");
  desktop.click("overview-tab-all");
  desktop.wait_for("task-row-KON-1");
  assert!(desktop.tree.get_element_by_id("overview-attention").is_some());
  assert!(desktop.tree.get_element_by_id("overview-tokens").is_some());
}

#[test]
fn organization_blocked_flag_preserves_the_run_label_in_overview_and_tasks() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    use orchester_core::store::RunStore;

    use crate::unreadable_run::{at, delivery, started};

    seed(repo);
    let mut db = ProjectDb::open(repo).expect("open");
    let snapshot = delivery(&mut db);
    let task = db.tasks().expect("tasks").remove(0);
    let mut run = started(&mut db, (task.id(), &snapshot), 1, 11);
    let events = run.request_pause(at(12)).expect("request pause");
    db.append_events(&run, &events).expect("request persisted");
    let events = run.pause(at(13)).expect("pause");
    db.append_events(&run, &events).expect("pause persisted");
    let task = db.task(task.id()).expect("read").expect("task");
    let organization = TaskOrganization::new(
      task.organization().status(),
      task.organization().priority(),
      Some("Waiting for a decision".into()),
      vec![],
      task.organization().tags().to_vec(),
    )
    .expect("flag");
    db.set_task_organization(task.id(), &organization, ChangeActor::Person, at(14))
      .expect("organization");
  });
  desktop.wait_for("task-row-KON-1");
  for label in ["In progress", "Paused", "Blocked"] {
    assert!(crate::github::shows(&mut desktop, "task-row-KON-1", label));
  }
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  for label in ["In progress", "Paused", "Blocked"] {
    assert!(crate::github::shows(&mut desktop, "task-row-KON-1", label));
  }
}
