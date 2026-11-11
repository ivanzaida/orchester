//! Leaving the panel of a local pipeline while its changes are not saved (PV2-10): every way out asks the same
//! question, as the Agents hub does, and *Discard changes* drops the edit. `cargo test -p orchester-app --test agents
//! pipeline_leaving`.

use orchester_app::routes::{PipelinesFilter, Route};
use orchester_core::id::ResourceId;

use super::{
  harness::Agents,
  pipeline_hub::{open, project, view},
};

const DISCARD: &str = "pipeline-hub-leave-discard";
const KEEP: &str = "pipeline-hub-leave-cancel";
const DESCRIPTION: &str = "pipeline-hub-description";

/// The hub open on `pipeline` with a description typed that is not saved.
fn editing(pipeline_at: impl FnOnce(&[ResourceId; 3]) -> ResourceId) -> (Agents, [ResourceId; 3], ResourceId) {
  let (mut agents, ids) = project();
  let chosen = pipeline_at(&ids);
  open(&mut agents, PipelinesFilter::All, Some(chosen));
  agents.until("the panel", |s| s.exists("pipeline-hub-save"));
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("the change held", |s| {
    s.text(&format!("pipeline-hub-row-{chosen}-line"))
      .starts_with("Unsaved changes")
  });
  (agents, ids, chosen)
}

/// Presses `id`, which leaves the panel: the question comes, nothing moves, *Keep editing* keeps what was typed, and
/// *Discard changes* goes to `place`.
fn asks_then_goes(agents: &mut Agents, from: &Route, id: &str, place: &Route) {
  agents.click(id);
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(
    &agents.window.navigation().route_now(),
    from,
    "{id}: nothing moved while it asks"
  );
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert_eq!(
    agents.value(DESCRIPTION),
    "Not saved yet",
    "{id}: Keep editing keeps what was typed"
  );

  agents.click(id);
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the place", |s| s.window.navigation().route_now() == *place);
}

#[test]
fn pipeline_leaving_for_another_pipeline_with_unsaved_changes_asks_first_and_discard_drops_them() {
  let (mut agents, [ready, draft, _], _) = editing(|ids| ids[0]);
  let here = view(PipelinesFilter::All, Some(ready));

  asks_then_goes(
    &mut agents,
    &here,
    &format!("pipeline-hub-row-{draft}"),
    &view(PipelinesFilter::All, Some(draft)),
  );

  agents.until("the other pipeline's panel", |s| {
    s.text("team-hub-panel-title") == "Dependency update"
  });
  assert_eq!(agents.value(DESCRIPTION), "", "the other pipeline's own description");
  // Coming back, the discarded change is gone.
  agents.click(&format!("pipeline-hub-row-{ready}"));
  agents.until("the first pipeline", |s| {
    s.text("team-hub-panel-title") == "Feature delivery"
  });
  assert_eq!(agents.value(DESCRIPTION), "");
  assert!(!agents.exists(DISCARD), "nothing left to lose, nothing asked");
}

#[test]
fn pipeline_leaving_by_new_pipeline_with_unsaved_changes_asks_first() {
  let (mut agents, [ready, ..], _) = editing(|ids| ids[0]);
  let here = view(PipelinesFilter::All, Some(ready));

  agents.click("pipeline-hub-new");
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(agents.window.navigation().route_now(), here);
  agents.click(DISCARD);
  agents.until("the new pipeline page", |s| s.window.navigation().route_now() != here);
}

#[test]
fn pipeline_leaving_a_card_opens_the_step_in_the_panel_and_keeps_the_edit() {
  let (mut agents, [ready, ..], _) = editing(|ids| ids[0]);

  // The step opens in the panel, which is not leaving it: nothing is asked and the edit stays.
  agents.click("pipeline-hub-flow-step-step");
  agents.until("the step", |s| s.exists("pipeline-hub-step-head"));
  assert!(!agents.exists(DISCARD), "opening a step asks nothing");
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes"),
    "the edit is still held"
  );
  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists(DESCRIPTION));
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet");
}

#[test]
fn pipeline_leaving_by_back_with_unsaved_changes_asks_first() {
  let (mut agents, [ready, draft, _]) = project();
  open(&mut agents, PipelinesFilter::All, Some(draft));
  let before = agents.window.navigation().route_now();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  let here = agents.window.navigation().route_now();
  agents.until("the panel", |s| s.exists("pipeline-hub-save"));
  agents.set(DESCRIPTION, "Not saved yet");
  agents.until("the change held", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });

  agents.window.navigation().back();
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(
    agents.window.navigation().route_now(),
    here,
    "nothing moved while it asks"
  );
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert_eq!(agents.value(DESCRIPTION), "Not saved yet");

  agents.window.navigation().back();
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("back", |s| s.window.navigation().route_now() == before);
}

#[test]
fn pipeline_leaving_without_changes_goes_at_once() {
  let (mut agents, [ready, draft, _]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists("pipeline-hub-save"));

  agents.click(&format!("pipeline-hub-row-{draft}"));
  agents.until("the other pipeline", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(draft))
  });
  assert!(!agents.exists(DISCARD), "nothing to lose, nothing asked");
}

#[test]
fn pipeline_leaving_after_a_save_goes_at_once() {
  let (mut agents, [ready, draft, _]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists("pipeline-hub-save"));
  agents.set(DESCRIPTION, "Saved");
  agents.press_until("pipeline-hub-save", "saved", |s| s.text("pipeline-hub-chip-0") == "v2");

  agents.click(&format!("pipeline-hub-row-{draft}"));
  agents.until("the other pipeline", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(draft))
  });
  assert!(!agents.exists(DISCARD), "what was saved is not lost");
}
