//! New module and Edit module as pages (`08`, `09`): the name typed as the title, the dates picked on their chips'
//! calendars with the other date ruling the range, a name another module has refused under the chips, a saved module's
//! page taking the form's place, and a change over one made meanwhile refused with the typed values kept.

use chrono::{Days, NaiveDate};
use orchester_app::routes::{ModuleListTab, ModuleTab, ProjectPage, Route};
use orchester_core::{
  module::{ModuleDetails, ModuleState},
  name::Name,
  task::organization::WorkflowStatus,
};

use super::harness::{Seeded, details, module_with, today};

/// Picks `day` on the open calendar `picker`, turning to its month when the grid does not show it.
fn pick(seeded: &mut Seeded, picker: &str, day: NaiveDate) {
  let id = format!("{picker}-day-{day}");
  for key in ["PageDown", "PageUp", "PageUp"] {
    if seeded.shows(&id) {
      break;
    }
    seeded.key(key);
  }
  seeded.click(&id);
}

#[test]
fn modules_new_module_is_a_page_that_makes_a_module() {
  let mut seeded = Seeded::new("tasks", |db| {
    module_with(db, details("Checkout", (0, 7)), &[]);
  });
  seeded.open(ProjectPage::Modules(ModuleListTab::Open), "modules-row-0");
  seeded.window.press("modules-new");
  assert_eq!(seeded.route(), Route::Project(seeded.project, ProjectPage::NewModule));
  seeded.until("the form", |seeded| seeded.shows("module-form"));
  assert!(seeded.text("module-form-head").contains("New module"));
  assert!(!seeded.shows("module-state"), "a new module is open");
  let start = today();
  let target = today().checked_add_days(Days::new(10)).expect("a day");
  seeded.click("module-target");
  seeded.until("the calendar", |seeded| seeded.shows("module-target-picker"));
  pick(&mut seeded, "module-target-picker", target);
  assert!(!seeded.shows("module-target-picker"), "a pick closes the calendar");
  seeded.click("module-start");
  seeded.until("the calendar", |seeded| seeded.shows("module-start-picker"));
  let after = target.succ_opt().expect("a day");
  pick(&mut seeded, "module-start-picker", after);
  assert!(
    seeded.shows("module-start-picker"),
    "the start cannot be after the target"
  );
  seeded.key("Escape");
  seeded.click("module-start");
  seeded.until("the calendar", |seeded| seeded.shows("module-start-picker"));
  pick(&mut seeded, "module-start-picker", start);
  seeded.set("module-name", " checkout ");
  seeded.click("module-editor-save");
  seeded.until("the refusal", |seeded| seeded.shows("module-editor-error"));
  assert_eq!(
    seeded.text("module-editor-error"),
    "Another module already has this name."
  );
  seeded.set("module-name", "Payments");
  seeded.set("module-description", "Card payments and refunds");
  seeded.click("module-editor-save");
  seeded.until("the module's page", |seeded| seeded.shows("module-summary"));
  let made = seeded.module_named("Payments").expect("Payments is stored");
  assert_eq!(
    (made.details().start(), made.details().target()),
    (Some(start), Some(target))
  );
  assert_eq!(made.details().description(), "Card payments and refunds");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::Module(made.id(), ModuleTab::Summary))
  );
}

#[test]
fn modules_edit_module_refuses_a_change_made_meanwhile_and_keeps_what_was_typed() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[(WorkflowStatus::Todo, 1)]).0);
  });
  let module = picked.expect("the module");
  seeded.open(ProjectPage::EditModule(module), "module-form");
  assert_eq!(seeded.text("module-state-value"), "Open");
  seeded.click("module-start");
  seeded.until("the calendar", |seeded| seeded.shows("module-start-picker"));
  seeded.click("module-start-picker-clear");
  assert_eq!(seeded.text("module-start-value"), "None", "Clear empties the date");
  // Another app, or a tool, renames the module while the form is open.
  let stored = seeded.module_named("Checkout").expect("stored");
  let renamed = ModuleDetails::new(
    Name::new("Checkout v2").expect("a name"),
    "",
    (stored.details().start(), stored.details().target()),
    ModuleState::Open,
  )
  .expect("details");
  seeded
    .db()
    .update_module(module, stored.details(), renamed)
    .expect("renamed elsewhere");
  seeded.set("module-name", "Checkout (closed)");
  seeded.click("module-editor-save");
  seeded.until("the refusal", |seeded| seeded.shows("module-editor-error"));
  assert!(
    seeded
      .text("module-editor-error")
      .starts_with("This module was changed since you opened it."),
    "{}",
    seeded.text("module-editor-error")
  );
  assert_eq!(seeded.text("module-start-value"), "None", "the typed values stay");
  assert!(
    seeded.module_named("Checkout v2").is_some(),
    "the change made meanwhile stays"
  );
}
