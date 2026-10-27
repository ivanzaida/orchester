//! The Modules list (`01 · Modules`, `04 · Modules · none yet`, the Closed line of `07`): its views Open and Closed
//! with their counts, each a place of its own; its search; each row's progress and forecast; a project without modules.

use orchester_app::routes::{ModuleListTab, ModuleTab, ProjectPage, Route};
use orchester_core::{module::ModuleState, task::organization::WorkflowStatus};

use super::harness::{Seeded, details, module_with, with};

fn rows(seeded: &mut Seeded) -> Vec<String> {
  let mut rows: Vec<String> = seeded
    .ids("modules-row-")
    .into_iter()
    .filter(|rest| rest.parse::<usize>().is_ok())
    .collect();
  rows.sort();
  rows
    .iter()
    .map(|index| seeded.text(&format!("modules-row-{index}")))
    .collect()
}

#[test]
fn modules_list_views_open_and_closed_modules_and_searches_them() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    let picks = [(WorkflowStatus::Done, 1), (WorkflowStatus::Todo, 2)];
    picked = Some(module_with(db, details("Checkout", (3, 4)), &picks).0);
    let payments = with(
      &details("Payments", (1, 9)),
      "Card payments and refunds",
      ModuleState::Open,
    );
    module_with(db, payments, &[]);
    let archive = with(&details("Archive", (20, 0)), "", ModuleState::Closed);
    module_with(db, archive, &[]);
  });
  let checkout = picked.expect("the module");
  seeded.open(ProjectPage::Modules(ModuleListTab::Open), "modules-row-1");
  assert_eq!(seeded.text("modules-tab-open"), "Open 2");
  assert_eq!(seeded.text("modules-tab-closed"), "Closed 1");
  let open = rows(&mut seeded);
  assert_eq!(open.len(), 2, "the open modules: {open:?}");
  assert!(
    open[0].contains("Checkout") && open[0].contains("1 of 3 tasks done"),
    "{}",
    open[0]
  );
  assert!(open[1].contains("Payments") && open[1].contains("Card payments and refunds"));
  seeded.until("the forecast", |seeded| seeded.shows("modules-row-0-forecast"));
  assert_eq!(
    seeded.text("modules-row-0-forecast"),
    "Not enough history",
    "nothing merged in the project yet"
  );

  seeded.click("modules-tab-closed");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::Modules(ModuleListTab::Closed))
  );
  let closed = rows(&mut seeded);
  assert!(closed.len() == 1 && closed[0].contains("Archive"), "{closed:?}");
  assert!(
    !seeded.shows("modules-row-0-forecast"),
    "a closed module has no forecast"
  );

  seeded.open(ProjectPage::Modules(ModuleListTab::Open), "modules-row-1");
  seeded.set("modules-search", "REFUND");
  let found = rows(&mut seeded);
  assert!(
    found.len() == 1 && found[0].contains("Payments"),
    "the description is searched too: {found:?}"
  );
  seeded.set("modules-search", "nothing like it");
  assert_eq!(seeded.text("modules-no-match"), "No module matches “nothing like it”.");
  seeded.set("modules-search", "check");
  seeded.window.press("modules-row-0");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::Module(checkout, ModuleTab::Summary)),
    "a row opens its module's Summary"
  );
}

#[test]
fn modules_list_without_modules_or_closed_ones_says_so() {
  let mut seeded = Seeded::new("tasks", |_| {});
  seeded.open(ProjectPage::Modules(ModuleListTab::Open), "modules-empty");
  assert!(seeded.text("modules-empty").contains("No modules yet"));
  assert!(!seeded.shows("modules-search"), "nothing to search");
  seeded.click("modules-empty-new");
  assert_eq!(seeded.route(), Route::Project(seeded.project, ProjectPage::NewModule));

  let mut seeded = Seeded::new("tasks", |db| {
    module_with(db, details("Checkout", (0, 7)), &[]);
  });
  seeded.open(ProjectPage::Modules(ModuleListTab::Closed), "modules-view-empty");
  assert_eq!(
    seeded.text("modules-view-empty"),
    "No module is closed yet. A module moves here when its state is Closed."
  );
  assert_eq!(seeded.text("modules-tab-closed"), "Closed 0");
}

#[test]
fn modules_old_routes_still_land() {
  let project = orchester_core::id::ProjectId::generate();
  let module = orchester_core::id::ModuleId::generate();
  let at = |page| Route::Project(project, page);
  assert_eq!(
    Route::parse(&format!("/project/{project}/modules")),
    Ok(at(ProjectPage::Modules(ModuleListTab::Open)))
  );
  assert_eq!(
    Route::parse(&format!("/project/{project}/modules/{module}")),
    Ok(at(ProjectPage::Module(module, ModuleTab::Summary))),
    "a module's address from before its tabs opens its Summary"
  );
  for (page, path) in [
    (
      ProjectPage::Modules(ModuleListTab::Closed),
      format!("/project/{project}/modules?tab=closed"),
    ),
    (ProjectPage::NewModule, format!("/project/{project}/modules/new")),
    (
      ProjectPage::Module(module, ModuleTab::Burndown),
      format!("/project/{project}/modules/{module}/burndown"),
    ),
    (
      ProjectPage::EditModule(module),
      format!("/project/{project}/modules/{module}/edit"),
    ),
    (
      ProjectPage::NewTaskInModule(module),
      format!("/project/{project}/tasks/new?module={module}"),
    ),
  ] {
    assert_eq!(at(page.clone()).path(), path);
    assert_eq!(Route::parse(&path), Ok(at(page)), "{path}");
  }
  assert_eq!(
    Route::parse(&format!("/project/{project}/modules/{module}/summary")),
    Ok(at(ProjectPage::Module(module, ModuleTab::Summary)))
  );
}
