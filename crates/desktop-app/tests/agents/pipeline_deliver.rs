//! The Deliver step opened in the Pipelines hub's panel (`Pipelines · v2` P15 `nPaMj`; PV2-23): the Deliver card opens
//! it as a step opens, with its head, the strip and its groups; *Mode*, *Merge method*, *Resolve conflicts with* and
//! *Re-run Check on the merged result* change the edit until *Save*; *Remove step* takes it off the edit without a
//! question, *Cancel* puts it back. `cargo test -p orchester-app --test agents pipeline_deliver`.

use orchester_app::routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  resource::{DeliverMode, DeliverStep, MergeMethod, Pipeline, PipelineEdit, RepositoryAccess, StepKind},
};

use super::{
  harness::Agents,
  pipeline_flow::{add, agent, store, tool},
  pipeline_hub::open,
  pipeline_step::{CANCEL, route, save_until, stored},
  pipeline_step_gate::claude,
  support::project_id,
};

pub(super) const HEAD: &str = "pipeline-hub-deliver-head";
pub(super) const AUTO: &str = "pipeline-hub-deliver-mode-auto";
pub(super) const RECHECK_HINT: &str = "pipeline-hub-deliver-recheck-hint";
pub(super) const ADD: &str = "pipeline-hub-flow-add-deliver";
const DELIVER_CARD: &str = "pipeline-hub-flow-deliver";
const REMOVE: &str = "pipeline-hub-deliver-remove";
const INTEGRATOR: &str = "pipeline-hub-deliver-integrator";
const INTEGRATOR_HINT: &str = "pipeline-hub-deliver-integrator-hint";

/// What a test pipeline is made of: a step per profile of `writers`, each one able to write and waiting for the one
/// before; a *Checks* step that runs the command tool `checks` after them; and its Deliver step.
#[derive(Default)]
pub(super) struct Made<'a> {
  pub writers: &'a [ResourceId],
  pub readers: &'a [ResourceId],
  pub checks: Option<ResourceId>,
  pub gated: bool,
  pub deliver: Option<DeliverStep>,
}

/// Pipeline `name` made of `made`, stored as its first version.
pub(super) fn pipeline(agents: &Agents, name: &str, made: &Made<'_>) -> ResourceId {
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let mut before = None;
  let steps = made
    .readers
    .iter()
    .map(|profile| (*profile, RepositoryAccess::ReadOnly))
    .chain(made.writers.iter().map(|profile| (*profile, RepositoryAccess::Write)));
  for (index, (profile, access)) in steps.enumerate() {
    let after: Vec<_> = before.iter().collect();
    let key = add(
      &mut edit,
      &format!("Step {}", index + 1),
      &after,
      agent(Some(profile), access),
    );
    if made.gated && index == 0 {
      edit.update(&key, |step| step.approval_gate = true);
    }
    before = Some(key);
  }
  if let Some(checks) = made.checks {
    let after: Vec<_> = before.iter().collect();
    add(&mut edit, "Checks", &after, StepKind::Tool { tool: Some(checks) });
  }
  let body = edit.validate().expect("a pipeline").with_deliver(made.deliver.clone());
  store(agents, name, &body)
}

/// Opens the hub on `id` and its Deliver step from its card, and waits for the step's head.
pub(super) fn open_deliver(agents: &mut Agents, id: ResourceId) {
  open(agents, PipelinesFilter::All, Some(id));
  agents.until("the Deliver card", |s| s.exists(DELIVER_CARD));
  agents.click(DELIVER_CARD);
  agents.until("the Deliver step", |s| s.exists(HEAD));
}

pub(super) fn deliver_route(id: ResourceId) -> Route {
  Route::Project(project_id(), ProjectPage::Pipeline(id, PipelineView::Deliver))
}

pub(super) fn pipeline_route(id: ResourceId) -> Route {
  Route::Project(
    project_id(),
    ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(id))),
  )
}

pub(super) fn unsaved(agents: &mut Agents, id: ResourceId) -> bool {
  agents
    .text(&format!("pipeline-hub-row-{id}-line"))
    .starts_with("Unsaved changes")
}

#[test]
fn pipeline_deliver_the_card_opens_the_step_in_the_panel_with_its_head_strip_and_groups() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "Feature delivery",
    &Made {
      writers: &[builder],
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);

  assert_eq!(
    route(&agents),
    deliver_route(id),
    "the route holds the open Deliver step"
  );
  assert!(
    agents.exists("screen-pipeline-hub") && !agents.exists("screen-pipelines"),
    "the hub's panel, not the old page"
  );
  assert_eq!(agents.text("pipeline-hub-step-back-name"), "Feature delivery");
  assert_eq!(agents.text("pipeline-hub-step-position"), "step 02 of 2");
  assert_eq!(agents.text("pipeline-hub-deliver-title"), "Deliver");
  assert!(agents.text(REMOVE).contains("Remove step"));
  assert_eq!(
    agents.text("pipeline-hub-step-chip-deliver"),
    "02 Deliver",
    "the strip numbers Deliver after the step"
  );
  for (group, label) in [
    ("pipeline-hub-deliver-mode", "Mode"),
    ("pipeline-hub-deliver-merge", "Merge"),
    ("pipeline-hub-deliver-conflicts", "Conflicts"),
    ("pipeline-hub-deliver-check", "Check"),
  ] {
    assert_eq!(agents.text(&format!("{group}-label")), label);
  }
  assert!(
    agents
      .text("pipeline-hub-deliver-mode-pull-request")
      .contains("Coming later")
  );
  assert_eq!(
    agents.text("pipeline-hub-deliver-method-hint"),
    "The base branch gets a merge commit; the run's commits stay as they are."
  );
  assert_eq!(agents.text(&format!("{INTEGRATOR}-name")), "Builder");

  agents.click("pipeline-hub-step-back");
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  assert_eq!(route(&agents), pipeline_route(id));
}

#[test]
fn pipeline_deliver_mode_and_method_change_the_edit_until_save() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let checks = tool(&agents, true);
  let id = pipeline(
    &agents,
    "Feature delivery",
    &Made {
      writers: &[builder],
      checks: Some(checks),
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);

  agents.click("pipeline-hub-deliver-mode-pull-request");
  agents.until("a pass", |_| true);
  assert!(!unsaved(&mut agents, id), "Pull request cannot be chosen");

  agents.click(AUTO);
  agents.until("Auto-merge", |s| {
    s.text(RECHECK_HINT) == "Always on for Auto-merge: nothing merges without a passing check on the merged result."
  });
  assert!(
    agents.exists("pipeline-hub-deliver-recheck-locked"),
    "the switch is locked with its lock"
  );
  agents.click("pipeline-hub-deliver-squash");
  agents.until("Squash's words", |s| s.exists("pipeline-hub-deliver-squash-body"));
  assert_eq!(
    agents.text("pipeline-hub-deliver-method-hint"),
    "The task's key and title, e.g. KON-16: Rate-limit messages"
  );
  assert_eq!(
    agents.text("pipeline-hub-deliver-squash-body"),
    "Body: a short summary of the work and the run's number"
  );
  assert!(unsaved(&mut agents, id));
  assert_eq!(
    stored(&agents, id).deliver(),
    Some(&DeliverStep::default()),
    "nothing stored before Save"
  );

  save_until(&mut agents, "Auto-merge and Squash stored", |s| {
    stored(s, id)
      .deliver()
      .is_some_and(|deliver| deliver.mode() == DeliverMode::AutoMerge && deliver.method() == MergeMethod::Squash)
  });
  assert!(stored(&agents, id).deliver().is_some_and(DeliverStep::recheck));
}

#[test]
fn pipeline_deliver_recheck_names_the_checks_and_turns_off_until_save() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let checks = tool(&agents, true);
  let id = pipeline(
    &agents,
    "Feature delivery",
    &Made {
      writers: &[builder],
      checks: Some(checks),
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);
  assert_eq!(
    agents.text(RECHECK_HINT),
    "Runs Checks again after syncing with the base branch. A run never merges on a check of an older base."
  );

  agents.click("pipeline-hub-deliver-recheck");
  agents.until("off", |s| {
    s.text(RECHECK_HINT)
      .starts_with("Off: a run merges once it is synced with the base")
  });
  save_until(&mut agents, "the re-check off stored", |s| {
    stored(s, id).deliver().is_some_and(|deliver| !deliver.recheck())
  });
}

#[test]
fn pipeline_deliver_recheck_without_a_command_tool_step_is_muted_and_takes_no_press() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "No checks",
    &Made {
      writers: &[builder],
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);
  assert_eq!(
    agents.text(RECHECK_HINT),
    "The pipeline has no command tool step to run again on the merged result."
  );
  agents.click("pipeline-hub-deliver-recheck");
  agents.until("a pass", |_| true);
  assert!(!unsaved(&mut agents, id), "the switch takes no press");
}

#[test]
fn pipeline_deliver_conflicts_offers_the_writing_profiles_and_keeps_the_one_chosen() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let fixer = claude(&agents, "Fixer", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "Two writers",
    &Made {
      writers: &[builder, fixer],
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);
  assert_eq!(
    agents.text(INTEGRATOR_HINT),
    "Integrate runs with this profile when syncing with the base branch conflicts."
  );
  let shown = agents.text(&format!("{INTEGRATOR}-name"));
  let (other, other_id) = if shown == "Builder" {
    ("Fixer", fixer)
  } else {
    ("Builder", builder)
  };

  agents.click(INTEGRATOR);
  agents.until("the picker", |s| s.exists(&format!("{INTEGRATOR}-picker-item-0")));
  let place = (0..2)
    .find(|place| agents.text(&format!("{INTEGRATOR}-picker-item-{place}-name")) == other)
    .expect("the other writing profile is offered");
  agents.click(&format!("{INTEGRATOR}-picker-item-{place}"));
  agents.until("the other profile", |s| s.text(&format!("{INTEGRATOR}-name")) == other);
  assert!(unsaved(&mut agents, id));

  save_until(&mut agents, "the integrator stored", |s| {
    stored(s, id).deliver().and_then(DeliverStep::integrator) == Some(other_id)
  });
}

#[test]
fn pipeline_deliver_conflicts_go_to_the_person_when_no_step_can_change_files() {
  let mut agents = Agents::new(true);
  let reader = claude(&agents, "Reader", RepositoryAccess::ReadOnly, &["cargo test"]);
  let id = pipeline(
    &agents,
    "Read only",
    &Made {
      readers: &[reader],
      deliver: Some(DeliverStep::default()),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);
  assert_eq!(
    agents.text(INTEGRATOR_HINT),
    "No step of this pipeline can change files, so conflicts go to you."
  );
  assert!(!agents.exists(INTEGRATOR), "no profile to choose");
}

#[test]
fn pipeline_deliver_remove_step_shows_add_deliver_and_cancel_puts_it_back() {
  let mut agents = Agents::new(true);
  let builder = claude(&agents, "Builder", RepositoryAccess::Write, &["cargo test"]);
  let id = pipeline(
    &agents,
    "Feature delivery",
    &Made {
      writers: &[builder],
      deliver: Some(DeliverStep::new(
        DeliverMode::AutoMerge,
        MergeMethod::Squash,
        None,
        true,
      )),
      ..Made::default()
    },
  );
  open_deliver(&mut agents, id);

  agents.click(REMOVE);
  agents.until("the pipeline", |s| s.exists("pipeline-hub-flow"));
  assert!(!agents.exists("pipeline-hub-leave"), "no question: nothing is stored");
  assert_eq!(route(&agents), pipeline_route(id));
  assert!(agents.exists(ADD) && !agents.exists(DELIVER_CARD));
  assert!(unsaved(&mut agents, id));
  assert!(stored(&agents, id).deliver().is_some(), "nothing stored before Save");

  agents.click(CANCEL);
  agents.until("Deliver is back", |s| s.exists(DELIVER_CARD));
  assert!(!agents.exists(ADD));

  agents.click(DELIVER_CARD);
  agents.until("the Deliver step", |s| s.exists(HEAD));
  agents.click(REMOVE);
  agents.until("the pipeline", |s| s.exists(ADD));
  save_until(&mut agents, "no Deliver step stored", |s| {
    stored(s, id).deliver().is_none()
  });
}
