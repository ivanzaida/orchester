//! Orchestrator settings (`Xir64`): choosing what the Orchestrator runs on saves it at once with the connection's
//! first model; a connection it cannot run on is listed, not chosen; the tabs not built say why.

use orchester_core::resource::ModelId;

use super::harness::{Orchestrator, Setup};

#[test]
fn orchestrator_settings_save_the_chosen_connection_with_its_first_model() {
  let mut screen = Orchestrator::new(Setup::NotChosen);
  screen.settings();
  let summary = screen.text("orchestrator-settings-summary");
  assert!(summary.contains("No connection"), "{summary}");
  let model = screen.text("orchestrator-model-row");
  assert!(model.contains("Choose a connection first."), "{model}");
  assert_eq!(
    screen.options("orchestrator-connection"),
    ["Claude · Kontur", "Codex · Kontur"]
  );
  // The Codex connection is listed but can't be chosen: choosing it saves nothing.
  screen.choose("orchestrator-connection", "Codex · Kontur");
  for _ in 0..20 {
    screen.window.tree.tick_futures();
    screen.window.pass();
  }
  assert_eq!(screen.db().orchestrator_settings().expect("settings"), None);
  screen.choose("orchestrator-connection", "Claude · Kontur");
  screen.until("the saved choice", |screen| {
    screen.db().orchestrator_settings().expect("settings").is_some()
  });
  let saved = screen.db().orchestrator_settings().expect("settings").expect("saved");
  assert_eq!(saved.connection(), screen.claude);
  assert_eq!(saved.model(), &ModelId::new("haiku").expect("model"));
  assert_eq!(saved.effort(), None);
  screen.until("its summary", |screen| {
    screen
      .text("orchestrator-settings-summary")
      .contains("Claude · Kontur · haiku")
  });
  assert!(screen.text("orchestrator-settings-saved").contains("All changes saved"));

  screen.choose("orchestrator-model", "opus");
  screen.until("the other model saved", |screen| {
    screen
      .db()
      .orchestrator_settings()
      .expect("settings")
      .expect("saved")
      .model()
      .as_str()
      == "opus"
  });
  assert_eq!(screen.options("orchestrator-effort"), ["CLI default", "Low", "High"]);
  screen.choose("orchestrator-effort", "High");
  screen.until("the effort saved", |screen| {
    screen
      .db()
      .orchestrator_settings()
      .expect("settings")
      .expect("saved")
      .effort()
      == Some("high")
  });
}

#[test]
fn orchestrator_settings_tabs_not_built_say_why() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.settings();
  assert!(screen.text("orchestrator-settings-tabs").contains("Profile"));
  assert!(screen.exists("orchestrator-settings-tab-actions"));
  assert!(
    screen
      .text("orchestrator-settings-connection")
      .contains("Connection & model")
  );
  assert!(
    screen
      .text("orchestrator-settings-note")
      .contains("a change applies from the next reply")
  );
}

#[test]
fn orchestrator_settings_head_is_laid_out_as_designed() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.settings();
  screen.until("the summary", |screen| {
    screen.text("orchestrator-settings-summary").contains("haiku")
  });
  // `Head` `b8gGj`, from the breadcrumb's foot: the title row (47) and the summary (24), 12 apart, then the tabs.
  // The shared breadcrumb is 18 high where the design's is 16; the rest is measured from it.
  let crumbs = screen.bounds("orchestrator-settings-crumbs");
  let foot = crumbs.y + crumbs.height;
  let summary = screen.bounds("orchestrator-settings-summary");
  let tabs = screen.bounds("orchestrator-settings-tabs");
  assert_eq!(
    (summary.y - foot, summary.height, tabs.y - foot),
    (71.0, 24.0, 107.0)
  );
  assert!(
    screen
      .text("orchestrator-settings-head")
      .contains("coordinates Kontur's queue"),
    "{}",
    screen.text("orchestrator-settings-head")
  );
}
