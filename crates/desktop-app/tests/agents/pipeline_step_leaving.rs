//! Leaving an open step while the pipeline's edit has changes that are not saved (`Pipelines · v2` P2, PV2-07): every
//! way out of the pipeline asks, a way between the pipeline and its steps does not, and text that is not valid is kept
//! where it was typed and said in the bar. `cargo test -p orchester-app --test agents pipeline_step_leaving`.

use orchester_app::{
  navigation::press::OpenIn,
  routes::{PipelinesFilter, ProjectPage, Route},
};
use orchester_core::{id::ResourceId, resource::StepKey};

use super::{
  harness::Agents,
  pipeline_hub::{project, view},
  pipeline_step::{INSTRUCTIONS, SAVE, keys, open_step, route, step_route},
  support::project_id,
};

const DISCARD: &str = "pipeline-hub-leave-discard";
const KEEP: &str = "pipeline-hub-leave-cancel";
const HINT: &str = "team-hub-actions-hint";

/// The first step of the ready pipeline open, with instructions typed that are not saved.
fn editing() -> (Agents, [ResourceId; 3], StepKey) {
  let (mut agents, ids) = project();
  let plan = keys(&agents, ids[0])[0].clone();
  open_step(&mut agents, ids[0], &plan);
  agents.set(INSTRUCTIONS, "Not saved yet");
  agents.until("the change held", |s| {
    s.text(&format!("pipeline-hub-row-{}-line", ids[0]))
      .starts_with("Unsaved changes")
  });
  (agents, ids, plan)
}

/// Presses `id`, which leaves the pipeline: the question comes, nothing moves, *Keep editing* keeps what was typed,
/// and *Discard changes* goes to `place`.
fn asks_then_goes(agents: &mut Agents, from: &Route, id: &str, place: &Route) {
  agents.click(id);
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(&route(agents), from, "{id}: nothing moved while it asks");
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert_eq!(
    agents.value(INSTRUCTIONS),
    "Not saved yet",
    "{id}: Keep editing keeps what was typed"
  );
  agents.click(id);
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the place", |s| route(s) == *place);
}

#[test]
fn leaving_an_open_step_for_another_pipeline_asks_and_discard_drops_the_edit() {
  let (mut agents, [ready, draft, _], plan) = editing();
  asks_then_goes(
    &mut agents,
    &step_route(ready, &plan),
    &format!("pipeline-hub-row-{draft}"),
    &view(PipelinesFilter::All, Some(draft)),
  );
  agents.click(&format!("pipeline-hub-row-{ready}"));
  agents.until("the first pipeline", |s| {
    s.text("team-hub-panel-title") == "Feature delivery"
  });
  assert!(!agents.exists(DISCARD), "nothing left to lose, nothing asked");
  assert!(
    !agents
      .text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes"),
    "the edit went with the answer"
  );
}

#[test]
fn leaving_an_open_step_by_a_filter_asks_first() {
  let (mut agents, [ready, ..], plan) = editing();
  // The ready pipeline is no draft, so the Drafts tab holds no chosen pipeline, as from the pipeline's panel.
  asks_then_goes(
    &mut agents,
    &step_route(ready, &plan),
    "pipeline-hub-filter-drafts",
    &view(PipelinesFilter::Drafts, None),
  );
}

#[test]
fn leaving_an_open_step_out_of_pipelines_or_by_a_tab_or_back_asks_first() {
  let (mut agents, [ready, ..], plan) = editing();
  let here = step_route(ready, &plan);

  // Another page of the project.
  agents
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Overview(Default::default())));
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(route(&agents), here, "nothing moved while it asks");
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert_eq!(agents.value(INSTRUCTIONS), "Not saved yet");

  // Closing the tab.
  agents
    .window
    .navigation()
    .open(Route::AllProjects, OpenIn::BackgroundTab);
  agents.window.pass();
  let tabs = agents.window.navigation().tabs_now().tabs().len();
  agents.window.navigation().close_active();
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(route(&agents), here);
  assert_eq!(
    agents.window.navigation().tabs_now().tabs().len(),
    tabs,
    "nothing closed while it asks"
  );
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));

  // Back, to where the step was opened from: the pipeline's own page, which asks nothing and keeps the edit.
  agents.window.navigation().back();
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  assert!(!agents.exists(DISCARD), "the pipeline and its step are one place");
  agents.until("still unsaved", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
}

#[test]
fn leaving_without_changes_goes_at_once_from_an_open_step() {
  let (mut agents, [ready, draft, _]) = project();
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click(&format!("pipeline-hub-row-{draft}"));
  agents.until("the other pipeline", |s| {
    s.text("team-hub-panel-title") == "Dependency update"
  });
  assert!(!agents.exists(DISCARD));
}

#[test]
fn text_that_is_not_valid_is_kept_across_steps_and_the_bar_says_what_to_fix() {
  let (mut agents, [ready, ..]) = project();
  let [plan, build] = <[StepKey; 2]>::try_from(keys(&agents, ready)).expect("two steps");
  open_step(&mut agents, ready, &plan);
  agents.set(INSTRUCTIONS, "Wrong {{nonsense.thing}}");
  agents.until("the bar says what to fix", |s| {
    s.text(HINT) == "Fix Instructions in Plan to save"
  });

  // Another step: the text stays with the step it was typed in, and the bar goes on saying so.
  agents.click(&format!("pipeline-hub-step-chip-{build}"));
  agents.until("the second step", |s| s.text("pipeline-hub-step-title") == "Build");
  assert_eq!(agents.text(HINT), "Fix Instructions in Plan to save");
  agents.click(&format!("pipeline-hub-step-chip-{plan}"));
  agents.until("the first step again", |s| s.exists(INSTRUCTIONS));
  assert_eq!(
    agents.value(INSTRUCTIONS),
    "Wrong {{nonsense.thing}}",
    "the person's text was kept"
  );
  assert!(
    agents.exists("pipeline-hub-step-instructions-note"),
    "and what is wrong with it is said again"
  );

  // Back to the pipeline: still not offered, and still said.
  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  assert_eq!(agents.text(HINT), "Fix Instructions in Plan to save");
  for _ in 0..3 {
    agents.click(SAVE);
  }
  assert!(
    super::pipeline_step::stored(&agents, ready)
      .step(&plan)
      .expect("step")
      .instructions
      .as_str()
      .is_empty(),
    "nothing was stored"
  );

  // Fixed, the bar says what Save makes again.
  agents.click(&format!("pipeline-hub-flow-step-{plan}"));
  agents.until("the step", |s| s.exists(INSTRUCTIONS));
  agents.set(INSTRUCTIONS, "Right {{task.title}}");
  agents.until("the bar is as before", |s| {
    s.text(HINT).starts_with("Unsaved changes · Save makes v2")
  });
}
