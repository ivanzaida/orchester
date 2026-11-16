//! A version shown read-only beside the panel's other views (`Pipelines · v2` P12 and P2, PV2-11 with PV2-07): a step
//! the route holds open wins over a version that was shown, the step's head has no *Versions*, and the list reads the
//! versions again after a save and a rename. `cargo test -p orchester-app --test agents pipeline_versions_views`.

use orchester_core::resource::StepKey;

use super::{
  pipeline_hub_menu::{key, start_rename},
  pipeline_step::{INSTRUCTIONS, keys, open_step, save_until, step_route},
  pipeline_versions::{BUTTON, DESCRIPTION, SAVE, open_list, two_versions, versions},
};

const VIEWING: &str = "pipeline-hub-version-viewing";

#[test]
fn pipeline_versions_a_step_the_route_opens_closes_the_version_that_was_shown() {
  let (mut agents, ready) = two_versions();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version", |s| s.exists(VIEWING));

  agents.window.navigation().go(step_route(ready, &plan));
  agents.until("the step", |s| s.exists("pipeline-hub-step-head"));
  assert!(
    !agents.exists(VIEWING),
    "the route is the stronger: the version is closed"
  );
  assert!(
    !agents.exists(BUTTON),
    "the step's head has no Versions; it returns to the pipeline"
  );
  assert!(agents.exists(SAVE), "the step has the edit's Save bar");

  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists(BUTTON));
  assert!(
    !agents.exists(VIEWING),
    "going back shows the pipeline, not the version"
  );
  assert_eq!(agents.value(DESCRIPTION), "Plan, then build");
}

#[test]
fn pipeline_versions_changes_made_in_a_step_are_kept_while_a_version_is_shown() {
  let (mut agents, ready) = two_versions();
  let [plan, _] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);
  agents.set(INSTRUCTIONS, "Plan it carefully");
  agents.until("the change is held", |s| s.exists("pipeline-hub-cancel"));
  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists(BUTTON));

  open_list(&mut agents);
  agents.click("pipeline-hub-version-1");
  agents.until("the version", |s| s.exists(VIEWING));
  assert_eq!(
    agents.text("pipeline-hub-version-kept"),
    "Your unsaved changes are kept"
  );
  agents.click("pipeline-hub-version-back");
  agents.until("the edit", |s| s.exists(SAVE));
  agents.window.navigation().go(step_route(ready, &plan));
  agents.until("the step", |s| s.exists("pipeline-hub-step-head"));
  assert_eq!(agents.value(INSTRUCTIONS), "Plan it carefully");
}

#[test]
fn pipeline_versions_the_list_reads_again_after_a_save() {
  let (mut agents, ready) = two_versions();
  open_list(&mut agents);
  assert_eq!(agents.text("pipeline-hub-version-2-current"), "current");
  assert!(!agents.exists("pipeline-hub-version-3"));
  agents.click(BUTTON);
  agents.until("the list is closed", |s| !s.exists("pipeline-hub-versions"));

  agents.set(DESCRIPTION, "A third description");
  save_until(&mut agents, "the third version", |s| versions(s, ready).len() == 3);
  agents.until("the head says v3", |s| {
    s.text("team-hub-panel-version").starts_with("v3 · ")
  });

  open_list(&mut agents);
  agents.until("the third version is listed", |s| s.exists("pipeline-hub-version-3"));
  assert_eq!(agents.text("pipeline-hub-version-3-current"), "current");
  assert!(agents.exists("pipeline-hub-version-3-check"));
  assert!(
    !agents.exists("pipeline-hub-version-2-current"),
    "v2 is no longer the current version"
  );
  let older = agents.text("pipeline-hub-version-2-meta");
  assert!(older.ends_with(" · no runs on it"), "{older}");
}

#[test]
fn pipeline_versions_the_list_names_the_pipeline_as_it_is_renamed() {
  let (mut agents, ready) = two_versions();
  open_list(&mut agents);
  assert_eq!(agents.text("pipeline-hub-versions-name"), "Feature delivery");
  agents.click(BUTTON);
  agents.until("the list is closed", |s| !s.exists("pipeline-hub-versions"));

  start_rename(&mut agents, ready);
  agents.set("pipeline-hub-rename", "Feature flow");
  key(&mut agents, "Enter");
  agents.until("renamed", |s| {
    s.exists("team-hub-panel-title") && s.text("team-hub-panel-title") == "Feature flow"
  });
  agents.until("the row follows", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-name")) == "Feature flow"
  });

  open_list(&mut agents);
  assert_eq!(agents.text("pipeline-hub-versions-name"), "Feature flow");
  assert_eq!(versions(&agents, ready).len(), 2, "renaming publishes nothing");
}
