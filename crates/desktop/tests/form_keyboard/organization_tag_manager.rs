use orchester_core::name::Name;
use orchester_store::ProjectDb;

use crate::{harness::Desktop, task_run::set_text};

#[test]
fn organization_tag_confirmation_disables_invalid_no_target_and_pending_actions() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
    db.create_project_tag(Name::new("UX").expect("name")).expect("tag");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("project-tags-manage");
  desktop.press("project-tags-manage");
  desktop.wait_for("project-tag-new");
  desktop.press("project-tag-new");
  desktop.wait_for("project-tag-name");
  assert!(!desktop.stops(12, false).contains(&Some("tag-editor-confirm".into())));
  set_text(&mut desktop, "project-tag-name", "   ");
  assert!(!desktop.stops(12, false).contains(&Some("tag-editor-confirm".into())));
  desktop.press("tag-editor-confirm");
  assert_eq!(
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .len(),
    2
  );
  desktop.key("Escape", "Escape");
  let source = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags")[0]
    .id();
  desktop.press(&format!("project-tag-merge-{source}"));
  desktop.wait_for("tag-merge-target");
  assert!(!desktop.stops(12, false).contains(&Some("tag-editor-confirm".into())));
  desktop.press("tag-editor-confirm");
  desktop.key("Escape", "Escape");
  desktop.press(&format!("project-tag-rename-{source}"));
  desktop.wait_for("project-tag-name");
  set_text(&mut desktop, "project-tag-name", "Client");
  assert!(desktop.stops(12, false).contains(&Some("tag-editor-confirm".into())));
  let connection = rusqlite::Connection::open(desktop.repo.join(".orchester/orchester.db")).expect("connection");
  connection.execute_batch("BEGIN IMMEDIATE").expect("hold write lock");
  desktop.tab_to("tag-editor-confirm", 12);
  desktop.enter();
  let stops = desktop.stops(12, false);
  assert!(!stops.contains(&Some("tag-editor-confirm".into())));
  assert!(!stops.contains(&Some("tag-editor-cancel".into())));
  desktop.press("tag-editor-confirm");
  connection.execute_batch("ROLLBACK").expect("release write lock");
  desktop.wait_until("rename completes", |desktop| {
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .iter()
      .any(|tag| tag.name().as_str() == "Client")
  });
  assert_eq!(
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .len(),
    2
  );
}

#[test]
fn organization_tag_rename_onto_an_existing_name_offers_merge() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
    db.create_project_tag(Name::new("UX").expect("name")).expect("tag");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("project-tags-manage");
  desktop.press("project-tags-manage");
  let tags = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags");
  let (desktop_tag, ux) = (tags[0].id(), tags[1].id());
  desktop.wait_for(&format!("project-tag-rename-{ux}"));
  desktop.press(&format!("project-tag-rename-{ux}"));
  desktop.wait_for("project-tag-name");
  set_text(&mut desktop, "project-tag-name", "desktop");
  desktop.wait_for("tag-editor-merge-duplicate");
  assert!(crate::agent_permissions::shows(
    &mut desktop,
    "Desktop already exists in"
  ));
  assert!(!desktop.stops(12, false).contains(&Some("tag-editor-confirm".into())));
  desktop.press("tag-editor-merge-duplicate");
  desktop.wait_for("tag-merge-target");
  desktop.tab_to("tag-editor-confirm", 12);
  desktop.enter();
  desktop.wait_until("UX merged into Desktop", |desktop| {
    let tags = ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags");
    tags.len() == 1 && tags[0].id() == desktop_tag
  });
}
