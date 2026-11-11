//! The Save bar of a local pipeline's panel (`Pipelines · v2` P1, PV2-10): every change is held until *Save*, which
//! publishes the next version at once; *Cancel* puts back what is stored; a save that cannot publish keeps the draft
//! and says why; a refused one says so next to the bar. `cargo test -p orchester-app --test agents pipeline_save`.

use std::collections::BTreeMap;

use orchester_app::routes::PipelinesFilter;
use orchester_core::{
  id::ResourceId,
  resource::{Deliverable, Pipeline, PipelineSources},
  source::SourcePath,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::{
  data::OpenedPipeline,
  write::{Allowed, save_here_in},
};

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline, project, setup},
};

const SAVE: &str = "pipeline-hub-save";
const CANCEL: &str = "pipeline-hub-cancel";
const DESCRIPTION: &str = "pipeline-hub-description";
const HINT: &str = "team-hub-actions-hint";

/// The versions the project's database holds of `id`, oldest first, and its stored draft.
fn stored(agents: &Agents, id: ResourceId) -> (Vec<Pipeline>, Option<Pipeline>) {
  let db = ProjectDb::open(&agents.root).expect("database");
  let mut versions = db.versions::<Pipeline>(id).expect("versions");
  versions.sort_by_key(|version| version.number());
  let draft = db
    .draft::<Pipeline>(id)
    .expect("draft")
    .map(|draft| draft.body().clone());
  (versions.iter().map(|version| version.body().clone()).collect(), draft)
}

/// Chooses the option `label` of the select `id`, as a person's press on it would.
fn choose(agents: &mut Agents, id: &str, label: &str) {
  let mut select = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("{id} is a select"));
  let index = select
    .labels()
    .iter()
    .position(|option| &**option == label)
    .unwrap_or_else(|| panic!("an option {label}"));
  select.commit(index);
  agents.window.pass();
}

/// Types `text` into the description of the open pipeline, which is a change the panel holds.
fn describe(agents: &mut Agents, text: &str) {
  agents.set(DESCRIPTION, text);
}

#[test]
fn pipeline_save_publishes_the_next_version_and_runs_keep_theirs() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  assert_eq!(
    agents.text(HINT),
    "Save makes v2 · runs keep the version they started on"
  );

  // Nothing differs: Save does nothing.
  agents.click(SAVE);
  agents.until("a pass", |_| true);
  assert_eq!(stored(&agents, ready).0.len(), 1);

  describe(&mut agents, "Plan, then build");
  agents.until("the row says it is not saved", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  assert_eq!(stored(&agents, ready).0.len(), 1, "nothing is stored before Save");

  agents.press_until(SAVE, "the next version", |s| stored(s, ready).0.len() == 2);
  let (versions, draft) = stored(&agents, ready);
  assert_eq!(
    versions[0].description(),
    "",
    "the version runs started on is untouched"
  );
  assert_eq!(versions[1].description(), "Plan, then build");
  assert!(draft.is_none(), "the draft became the version");
  agents.until("the panel shows v2", |s| {
    s.text("pipeline-hub-chip-0") == "v2" && !s.text(&format!("pipeline-hub-row-{ready}-line")).starts_with("Unsaved")
  });
  assert_eq!(agents.value(DESCRIPTION), "Plan, then build");
}

#[test]
fn pipeline_save_cancel_puts_back_what_is_stored() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));

  describe(&mut agents, "Never saved");
  agents.until("the row says it is not saved", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-line"))
      .starts_with("Unsaved changes")
  });
  agents.click(CANCEL);

  agents.until("what is stored shows again", |s| {
    s.value(DESCRIPTION).is_empty() && !s.text(&format!("pipeline-hub-row-{ready}-line")).starts_with("Unsaved")
  });
  let (versions, draft) = stored(&agents, ready);
  assert_eq!((versions.len(), draft), (1, None), "nothing was stored");
  // Save is off again: pressing it stores nothing.
  agents.click(SAVE);
  agents.until("a pass", |_| true);
  assert_eq!(stored(&agents, ready).0.len(), 1);
}

#[test]
fn pipeline_save_the_flow_draws_the_edit_not_the_stored_version() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  let before = agents.text("pipeline-hub-flow-end");
  assert!(before.contains("READY FOR REVIEW"), "{before}");

  choose(&mut agents, "pipeline-hub-delivers-pick", "A report");

  agents.until("the flow ends in the report", |s| {
    s.text("pipeline-hub-flow-end").contains("ITS REPORT")
  });
  assert_eq!(
    stored(&agents, ready).0[0].delivers(),
    Deliverable::Changes,
    "nothing stored yet"
  );
  agents.click(CANCEL);
  agents.until("the stored flow again", |s| {
    s.text("pipeline-hub-flow-end").contains("READY FOR REVIEW")
  });
}

#[test]
fn pipeline_save_a_choice_of_what_a_run_delivers_is_published() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));

  choose(&mut agents, "pipeline-hub-delivers-pick", "A report");
  agents.press_until(SAVE, "the next version", |s| stored(s, ready).0.len() == 2);

  assert_eq!(stored(&agents, ready).0[1].delivers(), Deliverable::Report);
}

#[test]
fn pipeline_save_refuses_a_change_that_cannot_be_published_and_stores_nothing() {
  let (mut agents, [_, update, _]) = project();
  open(&mut agents, PipelinesFilter::All, Some(update));
  agents.until("the panel", |s| s.exists(SAVE));

  // Build runs a profile that is gone: a save is refused and stores nothing (a pipeline holds no draft, D-032).
  describe(&mut agents, "Waiting for profiles");
  agents.click(SAVE);
  agents.until("the refusal", |s| s.exists("pipeline-hub-refused"));
  let said = agents.text("pipeline-hub-refused");
  assert!(said.contains("Couldn't save Dependency update"), "{said}");
  assert!(said.contains("Build"), "it names the step: {said}");
  let (versions, draft) = stored(&agents, update);
  assert_eq!(versions.len(), 1, "nothing was published");
  assert_eq!(draft, None, "nothing was stored as a draft");
  assert_eq!(
    agents.value(DESCRIPTION),
    "Waiting for profiles",
    "what was typed stays"
  );
}

#[test]
fn pipeline_save_refused_because_the_pipeline_changed_says_so_and_keeps_what_was_typed() {
  let (mut agents, [ready, ..]) = project();
  open(&mut agents, PipelinesFilter::All, Some(ready));
  agents.until("the panel", |s| s.exists(SAVE));
  describe(&mut agents, "Mine");

  // Another editor saves the next version behind the page.
  let before = OpenedPipeline::load_in(&agents.root, ready).expect("pipeline");
  let theirs = before.body.clone().with_description("Theirs".into());
  save_here_in(
    &agents.root,
    &before,
    &theirs,
    &orchester_core::name::Name::new(&before.name).expect("a stored name"),
    &Allowed::default(),
  )
  .expect("their version");

  agents.click(SAVE);
  agents.until("the refusal", |s| s.exists("pipeline-hub-bar-notice"));

  let said = agents.text("pipeline-hub-bar-notice");
  assert!(said.contains("Couldn't save Feature delivery"), "{said}");
  let (versions, draft) = stored(&agents, ready);
  assert_eq!(versions.len(), 2, "only their version was published");
  assert_eq!(versions[1].description(), "Theirs");
  assert_eq!(draft, None, "a pipeline holds no draft");
  assert_eq!(agents.value(DESCRIPTION), "Mine", "what was typed stays");
}

#[test]
fn pipeline_save_takes_a_source_off_the_pipeline() {
  let (mut agents, _) = setup();
  let builder = agents.profile(true);
  let sourced = pipeline(&agents, "Sourced pipeline", Some(builder));
  let folder = std::env::temp_dir();
  let path = SourcePath::new(&folder.to_string_lossy()).expect("an absolute path");
  {
    let mut db = ProjectDb::open(&agents.root).expect("database");
    let opened = OpenedPipeline::load(&db, sourced).expect("pipeline");
    let body = opened
      .body
      .clone()
      .with_sources(PipelineSources::new(vec![path.clone()], BTreeMap::new()))
      .expect("sources");
    db.publish_pipeline_edit(sourced, &body, opened.version, None, Timestamp::now())
      .expect("v2 with a source");
  }
  open(&mut agents, PipelinesFilter::All, Some(sourced));
  agents.until("the source's pill", |s| s.exists("pipeline-hub-source-0"));
  assert_eq!(agents.text("pipeline-hub-source-0"), path.as_str());

  agents.click("pipeline-hub-source-0-remove");
  agents.until("the pill is gone", |s| !s.exists("pipeline-hub-source-0"));
  assert_eq!(stored(&agents, sourced).0.len(), 2, "nothing is stored before Save");
  agents.click(SAVE);
  // What the sources list differs from the published version is a person's decision, which the bar asks.
  agents.until("the question", |s| s.exists("pipeline-hub-confirm-confirm"));
  assert_eq!(
    stored(&agents, sourced).0.len(),
    2,
    "nothing is published before the answer"
  );
  agents.press_until("pipeline-hub-confirm-confirm", "the next version", |s| {
    stored(s, sourced).0.len() == 3
  });

  assert!(stored(&agents, sourced).0[2].sources().pipeline().is_empty());
  assert_eq!(stored(&agents, sourced).0[1].sources().pipeline(), [path]);
}
