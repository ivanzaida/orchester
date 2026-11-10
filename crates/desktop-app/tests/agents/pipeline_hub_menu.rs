//! The ⋯ in the Pipelines hub's panel head (`Pipelines · v2` P11 `nrTt0`) and the Archived tab (PV2-12): Rename in place,
//! Archive with its confirmation, and Restore. Duplicate and Make default are in `tests/pipelines/default.rs`.
//! `cargo test -p orchester-app --test agents pipeline_hub_menu`.

use orchester_app::routes::PipelinesFilter;
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::{open, project, setup},
};

fn names(agents: &Agents, id: ResourceId) -> (String, String) {
  let db = ProjectDb::open(&agents.root).expect("database");
  let resource = db.resource(id).expect("reads").expect("the pipeline");
  (resource.name().as_str().to_owned(), resource.slug().as_str().to_owned())
}

fn key(agents: &mut Agents, key: &str) {
  agents
    .window
    .tree
    .key_down(key.to_owned(), key.to_owned(), false, false, false);
  agents.window.pass();
}

fn archived(agents: &Agents) -> Vec<ResourceId> {
  ProjectDb::open(&agents.root)
    .expect("database")
    .archived_local_resources(ResourceKind::Pipeline)
    .expect("archived")
    .iter()
    .map(|(resource, _)| resource.id())
    .collect()
}

/// Opens the panel of `id` and its ⋯ menu.
fn open_menu(agents: &mut Agents, id: ResourceId) {
  agents.click(&format!("pipeline-hub-row-{id}"));
  agents.until("the panel", |s| {
    s.exists("pipeline-hub-more") && s.exists(&format!("pipeline-hub-row-{id}-name"))
  });
  agents.click("pipeline-hub-more");
  agents.until("the menu", |s| s.exists("pipeline-hub-menu-archive"));
}

/// Opens the panel of `id` and starts renaming it from the ⋯ menu.
fn start_rename(agents: &mut Agents, id: ResourceId) {
  open_menu(agents, id);
  agents.click("pipeline-hub-menu-rename");
  agents.until("the field", |s| s.exists("pipeline-hub-rename"));
}

#[test]
fn pipeline_hub_menu_rename_edits_the_title_in_place_and_enter_saves() {
  let (mut agents, [ready, ..]) = project();
  start_rename(&mut agents, ready);
  assert_eq!(agents.value("pipeline-hub-rename"), "Feature delivery");
  assert!(
    !agents.exists("team-hub-panel-title"),
    "the field stands in the title's place"
  );
  agents.set("pipeline-hub-rename", "  Feature flow ");
  key(&mut agents, "Enter");
  agents.until("renamed", |s| {
    s.exists("team-hub-panel-title") && s.text("team-hub-panel-title") == "Feature flow"
  });
  assert!(!agents.exists("pipeline-hub-rename"));
  assert_eq!(
    names(&agents, ready),
    ("Feature flow".to_owned(), "feature-delivery".to_owned()),
    "the name changes, the slug stays"
  );
  agents.until("the row follows", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-name")) == "Feature flow"
  });
  let db = ProjectDb::open(&agents.root).expect("database");
  let latest = db.latest_version::<Pipeline>(ready).expect("reads").expect("published");
  assert_eq!(latest.number().to_string(), "v1", "renaming publishes nothing");
}

#[test]
fn pipeline_hub_menu_rename_refuses_a_name_another_pipeline_has_and_escape_keeps_the_name() {
  let (mut agents, [ready, _, stuck]) = project();
  start_rename(&mut agents, ready);
  agents.set("pipeline-hub-rename", "quick fix");
  key(&mut agents, "Enter");
  agents.until("the refusal", |s| s.exists("pipeline-hub-rename-error"));
  let refusal = agents.text("pipeline-hub-rename-error");
  assert!(
    refusal.contains("Quick fix"),
    "the reason names the pipeline that has it: {refusal}"
  );
  assert!(agents.exists("pipeline-hub-rename"), "the field stays open");
  assert_eq!(names(&agents, ready).0, "Feature delivery");
  assert_eq!(names(&agents, stuck).0, "Quick fix");

  agents.set("pipeline-hub-rename", "   ");
  key(&mut agents, "Enter");
  agents.until("the blank refused", |s| {
    s.exists("pipeline-hub-rename-error") && s.text("pipeline-hub-rename-error").contains("needs a name")
  });

  agents.set("pipeline-hub-rename", "Something else");
  key(&mut agents, "Escape");
  agents.until("cancelled", |s| !s.exists("pipeline-hub-rename"));
  assert_eq!(agents.text("team-hub-panel-title"), "Feature delivery");
  assert_eq!(names(&agents, ready).0, "Feature delivery");
}

#[test]
fn pipeline_hub_menu_archive_says_how_many_tasks_use_it_and_asks_first() {
  let (mut agents, [ready, draft, _]) = project();
  open_menu(&mut agents, ready);
  assert_eq!(agents.text("pipeline-hub-menu-archive-hint"), "3 tasks");
  agents.click("pipeline-hub-menu-archive");
  agents.until("the confirmation", |s| s.exists("team-hub-archive-confirm"));
  assert!(
    archived(&agents).is_empty(),
    "nothing is archived before the person confirms"
  );
  agents.click("team-hub-archive-cancel");
  agents.until("the question is gone", |s| !s.exists("team-hub-archive-confirm"));
  assert!(archived(&agents).is_empty());
  assert_eq!(agents.text("pipeline-hub-filter-archived-count"), "0");

  // A pipeline that no task uses says nothing at the end of the row.
  open_menu(&mut agents, draft);
  assert!(!agents.exists("pipeline-hub-menu-archive-hint"));
}

#[test]
fn pipeline_hub_menu_archiving_a_pipeline_in_use_moves_it_to_archived_and_restore_brings_it_back() {
  let (mut agents, [ready, draft, stuck]) = project();
  open_menu(&mut agents, ready);
  agents.click("pipeline-hub-menu-archive");
  agents.until("the confirmation", |s| s.exists("team-hub-archive-confirm"));
  agents.press_until("team-hub-archive-confirm", "archived", |s| archived(s).contains(&ready));
  agents.until("the lists follow", |s| {
    s.text("pipeline-hub-filter-all-count") == "2" && s.text("pipeline-hub-filter-archived-count") == "1"
  });
  assert!(!agents.exists(&format!("pipeline-hub-row-{ready}")), "it left All");
  assert!(agents.exists(&format!("pipeline-hub-row-{draft}")));
  assert!(agents.exists(&format!("pipeline-hub-row-{stuck}")));
  assert!(!agents.exists("team-hub-archive-confirm"));

  agents.click("pipeline-hub-filter-archived");
  agents.until("Archived", |s| s.exists(&format!("pipeline-hub-row-{ready}")));
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Archived ")
  );
  assert_eq!(agents.text("team-hub-panel-title"), "Feature delivery");
  assert!(
    !agents.exists("pipeline-hub-more"),
    "an archived pipeline has no ⋯ menu"
  );
  assert_eq!(agents.text("pipeline-hub-restore"), "Restore Feature delivery");
  agents.press_until("pipeline-hub-restore", "restored", |s| archived(s).is_empty());
  agents.until("the lists follow", |s| {
    s.text("pipeline-hub-filter-all-count") == "3" && s.text("pipeline-hub-filter-archived-count") == "0"
  });
  agents.until("the Archived tab is empty again", |s| {
    s.exists("pipeline-hub-empty") && s.text("pipeline-hub-empty-title") == "Nothing archived"
  });
  open(&mut agents, PipelinesFilter::All, Some(ready));
  assert_eq!(
    agents.text(&format!("pipeline-hub-row-{ready}-line")),
    "2 steps · used by 3 tasks",
    "restored as it was, still used by its tasks"
  );
}

#[test]
fn pipeline_hub_menu_an_archived_default_is_not_tagged() {
  let (mut agents, [ready, ..]) = setup();
  ProjectDb::open(&agents.root)
    .expect("database")
    .save_default_pipeline(Some(ready))
    .expect("default stored");
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("tagged", |s| s.exists(&format!("pipeline-hub-row-{ready}-tag")));
  assert_eq!(agents.text(&format!("pipeline-hub-row-{ready}-tag")), "Default");
  open_menu(&mut agents, ready);
  agents.click("pipeline-hub-menu-archive");
  agents.until("the confirmation", |s| s.exists("team-hub-archive-confirm"));
  agents.press_until("team-hub-archive-confirm", "archived", |s| archived(s).contains(&ready));
  agents.click("pipeline-hub-filter-archived");
  agents.until("Archived", |s| s.exists(&format!("pipeline-hub-row-{ready}")));
  assert!(
    !agents.exists(&format!("pipeline-hub-row-{ready}-tag")),
    "an archived pipeline is not offered first by New task"
  );
  assert_eq!(
    ProjectDb::open(&agents.root)
      .expect("database")
      .default_pipeline()
      .expect("reads"),
    Some(ready),
    "the stored default is left alone, so restoring brings the tag back"
  );
  agents.press_until("pipeline-hub-restore", "restored", |s| archived(s).is_empty());
  agents.until("the Archived tab is empty", |s| s.exists("pipeline-hub-empty"));
  agents.click("pipeline-hub-filter-all");
  agents.until("tagged again", |s| s.exists(&format!("pipeline-hub-row-{ready}-tag")));
}
