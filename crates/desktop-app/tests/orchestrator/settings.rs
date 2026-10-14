//! Orchestrator settings (`Xir64`): what the Orchestrator runs on is versioned as agent profiles are. The first
//! version is published from the form; after that a change is saved as a draft and published as the next version,
//! which the chat answers on. A connection it cannot run on is listed, not publishable; a model the list doesn't have
//! is typed under *Other model id…*, and with no connection at all the select has nothing to open; the tabs not built
//! say why.

use orchester_core::{orchestrator::OrchestratorSettings, resource::ModelId};

use super::harness::{Orchestrator, Setup};

/// The published version and the draft.
fn kept(screen: &Orchestrator) -> (Option<(u32, OrchestratorSettings)>, Option<OrchestratorSettings>) {
  let db = screen.db();
  (
    db.orchestrator_published().expect("published"),
    db.orchestrator_draft().expect("draft"),
  )
}

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("model")
}

#[test]
fn orchestrator_settings_publish_the_first_version_then_draft_and_publish_the_next() {
  let mut screen = Orchestrator::new(Setup::NotChosen);
  screen.settings();
  // The first connection it can run on, with its first model, is in the form; nothing is kept yet.
  screen.until("the first version to publish", |screen| {
    screen.text("orchestrator-settings-saved") == "Not published"
  });
  assert!(screen.text("orchestrator-settings-summary").contains("No connection"));
  assert!(
    screen
      .text("orchestrator-settings-publish-note")
      .contains("can send once v1 is published")
  );
  assert!(!screen.exists("orchestrator-settings-save-draft"));
  assert_eq!(
    screen.options("orchestrator-connection"),
    ["Claude · Kontur", "Codex · Kontur"]
  );
  screen.until("Publish v1", |screen| {
    screen.text("orchestrator-settings-publish") == "Publish v1"
      && screen.text("orchestrator-settings-publish-note").contains("v1")
  });
  screen.click("orchestrator-settings-publish");
  screen.until("v1", |screen| kept(screen).0.is_some());
  let first = OrchestratorSettings::new(screen.claude, model("haiku"), None);
  assert_eq!(kept(&screen), (Some((1, first.clone())), None));
  screen.until("its state", |screen| {
    screen.text("orchestrator-settings-saved") == "Published v1"
      && screen
        .text("orchestrator-settings-summary")
        .contains("Claude · Kontur · haiku")
  });
  assert!(
    screen
      .text("orchestrator-settings-publish-note")
      .contains("No changes since v1.")
  );

  // A change waits for Save draft, and the chat stays on v1 until v2 is published.
  screen.choose("orchestrator-model", "opus");
  assert_eq!(screen.options("orchestrator-effort"), ["CLI default", "Low", "High"]);
  screen.choose("orchestrator-effort", "High");
  screen.until("the unsaved change", |screen| {
    screen.text("orchestrator-settings-saved") == "Unsaved changes · Published v1"
  });
  assert!(
    screen
      .text("orchestrator-settings-publish-note")
      .contains("Save the draft before publishing.")
  );
  screen.click("orchestrator-settings-save-draft");
  let second = OrchestratorSettings::new(screen.claude, model("opus"), Some("high".to_owned()));
  screen.until("the draft", |screen| kept(screen).1.is_some());
  assert_eq!(kept(&screen), (Some((1, first)), Some(second.clone())));
  screen.until("Draft saved", |screen| {
    screen.text("orchestrator-settings-saved") == "Draft saved · Published v1"
  });
  screen.click("orchestrator-settings-publish");
  screen.until("v2", |screen| kept(screen) == (Some((2, second.clone())), None));
}

#[test]
fn orchestrator_settings_take_a_model_the_list_does_not_have() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.settings();
  screen.until("Published v1", |screen| {
    screen.text("orchestrator-settings-saved") == "Published v1"
  });
  screen.choose("orchestrator-effort", "High");
  screen.choose("orchestrator-model", "Other model id…");
  screen.until("the model id field", |screen| {
    screen.exists("orchestrator-model-custom")
  });
  // Nothing typed yet: the head says the model is what publishing needs.
  screen.until("the true reason", |screen| {
    screen
      .text("orchestrator-settings-publish-note")
      .contains("Publishing needs a model")
  });
  // The chosen effort stays, among the efforts the connection's models list, unchecked for this one.
  assert_eq!(screen.options("orchestrator-effort"), ["CLI default", "Low", "High"]);
  assert!(
    screen
      .text("orchestrator-effort-row")
      .contains("Not checked for this model")
  );
  screen.set("orchestrator-model-custom", "claude opus");
  screen.until("why it isn't one", |screen| {
    screen.exists("orchestrator-model-custom-error")
  });
  // Save draft takes nothing that is no model id.
  screen.click("orchestrator-settings-save-draft");
  for _ in 0..20 {
    screen.window.tree.tick_futures();
    screen.window.pass();
  }
  assert_eq!(kept(&screen).1, None);
  screen.set("orchestrator-model-custom", "claude-opus-5-5");
  screen.until("a valid id", |screen| !screen.exists("orchestrator-model-custom-error"));
  screen.click("orchestrator-settings-save-draft");
  screen.until("the draft", |screen| {
    kept(screen)
      .1
      .is_some_and(|draft| draft.model().as_str() == "claude-opus-5-5" && draft.effort() == Some("high"))
  });
}

#[test]
fn orchestrator_settings_connection_select_opens_only_with_connections_to_list() {
  // Connections it cannot run on are listed with why (Codex is held back), so the menu opens on them.
  let mut screen = Orchestrator::new(Setup::NotChosen);
  screen.settings();
  screen.until("the connections", |screen| {
    screen.options("orchestrator-connection").len() == 2
  });
  screen.press("orchestrator-connection");
  assert!(screen.is_open("orchestrator-connection"));

  // With none at all there is nothing to list: the select does not open an empty menu.
  let mut screen = Orchestrator::new(Setup::NoConnection);
  screen.settings();
  screen.until("the page", |screen| {
    screen
      .text("orchestrator-settings-connection")
      .contains("No usable connection")
  });
  assert!(screen.options("orchestrator-connection").is_empty());
  screen.press("orchestrator-connection");
  assert!(!screen.is_open("orchestrator-connection"));
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
      .contains("a published change applies from the next reply")
  );
}

#[test]
fn orchestrator_settings_head_is_laid_out_as_designed() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.settings();
  screen.until("the summary", |screen| {
    screen.text("orchestrator-settings-summary").contains("haiku")
  });
  // As `jmyWu` draws it: a draft saved, Publish v2, no note under them.
  screen.choose("orchestrator-model", "opus");
  screen.click("orchestrator-settings-save-draft");
  screen.until("the draft", |screen| {
    screen.text("orchestrator-settings-saved") == "Draft saved · Published v1"
  });
  // `Head` `b8gGj`, from the breadcrumb's foot: the title row (47) and the summary (24), 12 apart, then the tabs.
  // The shared breadcrumb is 18 high where the design's is 16; the rest is measured from it.
  let crumbs = screen.bounds("orchestrator-settings-crumbs");
  let foot = crumbs.y + crumbs.height;
  let summary = screen.bounds("orchestrator-settings-summary");
  let tabs = screen.bounds("orchestrator-settings-tabs");
  assert_eq!((summary.y - foot, summary.height, tabs.y - foot), (71.0, 24.0, 107.0));
  assert!(
    screen
      .text("orchestrator-settings-head")
      .contains("coordinates Kontur's queue"),
    "{}",
    screen.text("orchestrator-settings-head")
  );
}
