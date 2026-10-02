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
