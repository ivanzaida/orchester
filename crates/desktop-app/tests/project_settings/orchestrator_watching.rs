//! Orchestrator › *Watching* (07 `DE2Pj`, `b00C9G`): the default order for watching and when a run without progress
//! is reported, saved as the person leaves a field or flips the switch and read back from the project.
use orchester_core::orchestrator::WatchSettings;
use orchester_workspace::tasks::outside::Outside;

use super::harness::Settings;

const ORDER: &str = "settings-orchestrator-default-order-field";
const MINUTES: &str = "settings-orchestrator-no-progress-minutes";
const SWITCH: &str = "settings-orchestrator-no-progress-switch";
const ERROR: &str = "settings-orchestrator-no-progress-error";

/// Project settings on its Orchestrator tab, *Watching* read.
fn watching() -> Settings {
  let mut page = Settings::new(false);
  page.tab("orchestrator", ORDER);
  page.until("the watch settings read", |p| p.exists(MINUTES));
  page
}

fn stored(page: &Settings) -> WatchSettings {
  page.db().orchestrator_watch().expect("watch settings")
}

/// Moves the focus to `id`, as clicking another field does.
fn focus(page: &mut Settings, id: &str) {
  page
    .window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .focus();
  page.window.pass();
}

#[test]
fn project_settings_orchestrator_watching_sits_between_the_switches_and_this_computer() {
  let mut page = watching();
  let top = |page: &mut Settings, id: &str| {
    page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .y
  };
  let (may, watching, computer) = (
    top(&mut page, "settings-orchestrator-may"),
    top(&mut page, "settings-orchestrator-watching"),
    top(&mut page, "settings-orchestrator-computer"),
  );
  assert!(may < watching && watching < computer, "{may} {watching} {computer}");
  assert!(page.text("settings-orchestrator-watching-head").contains("Watching"));
  assert!(
    page
      .text("settings-orchestrator-watching-where")
      .contains("For /watch-module and /watch-task in this project")
  );
  assert!(
    page
      .text("settings-orchestrator-default-order-label")
      .contains("Default order for watching")
  );
  assert!(page.text(&format!("{ORDER}-foot")).contains("Shift Enter adds a line"));
  assert!(
    page
      .text("settings-orchestrator-no-progress-label")
      .contains("A run without progress for")
  );
  assert_eq!(page.text("settings-orchestrator-no-progress-unit"), "min");
  assert!(
    page
      .text("settings-orchestrator-watching-foot")
      .contains("no default order and 20 min")
  );
  // A new project: no order, and a run reported after 20 minutes.
  assert_eq!(page.value(ORDER), "");
  assert_eq!(page.value(MINUTES), "20");
  assert!(page.switch_on(SWITCH));
}

#[test]
fn project_settings_orchestrator_watching_saves_the_order_and_the_minutes() {
  let mut page = watching();
  let order = "Answer agents from the brief.\nAsk me before merging into a module branch.";
  page.set(ORDER, order);
  focus(&mut page, MINUTES);
  page.until("the order saved", |p| stored(p).default_order() == order);

  // A wrong value says why and saves nothing; leaving puts the saved minutes back.
  page.set(MINUTES, "0");
  page.until("the error", |p| p.exists(ERROR));
  assert!(page.text(ERROR).contains("Whole minutes, 1 to 240"));
  focus(&mut page, ORDER);
  page.until("the saved minutes back", |p| p.value(MINUTES) == "20");
  assert!(!page.exists(ERROR));
  assert_eq!(stored(&page).no_progress_minutes(), Some(20));

  page.set(MINUTES, "45");
  focus(&mut page, ORDER);
  page.until("the minutes saved", |p| stored(p).no_progress_minutes() == Some(45));

  // Off is saved, and the box keeps its minutes for when it is on again.
  page.click(SWITCH);
  page.until("off saved", |p| stored(p).no_progress_minutes().is_none());
  assert!(!page.switch_on(SWITCH));
  assert_eq!(page.value(MINUTES), "45");
  page.click(SWITCH);
  page.until("on again", |p| stored(p).no_progress_minutes() == Some(45));
  assert_eq!(stored(&page).default_order(), order);
  page.saved();
}

#[test]
fn project_settings_orchestrator_watching_loads_what_the_project_holds() {
  let mut page = watching();
  let watch = WatchSettings::new("Tell me about every failed run", None).expect("valid");
  page.db().save_orchestrator_watch(&watch).expect("saved");
  // A change made elsewhere reads the settings again.
  page.outside(Outside::Orchestrator);
  page.until("the stored order", |p| {
    p.value(ORDER) == "Tell me about every failed run"
  });
  page.until("off", |p| !p.switch_on(SWITCH));
  assert_eq!(page.value(MINUTES), "20");
}
