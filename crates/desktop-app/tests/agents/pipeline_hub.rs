//! The Pipelines hub of a local project (`Pipelines · v2` `pCzIS`, P1; Pipelines v2 PV2-04), the Agents hub's sibling:
//! a filter (*All*, *Archived*) with counts over a list grouped by whether a pipeline can run as it is, the
//! panel of the chosen one, and the route that keeps the tab and the pipeline. A team project's hub is
//! `pipeline_hub_team`.
//! `cargo test -p orchester-app --test agents pipeline_hub`.

use orchester_app::{
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  ui::icon::Icon,
};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{Pipeline, PipelineEdit, RepositoryAccess, Slug, StepKind},
  store::TaskStore,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{harness::Agents, support::project_id};

pub(super) fn view(filter: PipelinesFilter, item: Option<ResourceId>) -> Route {
  Route::Project(project_id(), ProjectPage::pipelines(PipelinesView::new(filter, item)))
}

/// Another editor saves `body` as the next version of the pipeline `before` was opened as, behind the page.
pub(super) fn save_behind(
  agents: &Agents,
  before: &orchester_workspace::pipelines::data::OpenedPipeline,
  body: &Pipeline,
) {
  let servers = agents.window.launch.services().expect("the session").servers.clone();
  let name = Name::new(&before.name).expect("a stored name");
  let allowed = orchester_workspace::pipelines::write::Allowed::default();
  orchester_workspace::pipelines::write::save_named_in(&agents.root, &servers, before, body, &name, &allowed)
    .expect("their version");
}

/// Opens the hub on `filter` with `item` named and waits until the list is read.
pub(super) fn open(agents: &mut Agents, filter: PipelinesFilter, item: Option<ResourceId>) {
  agents.window.navigation().go(view(filter, item));
  agents.until("the hub", |s| {
    (s.exists("pipeline-hub-filter") && s.exists("pipeline-hub-master-detail")) || s.exists("pipeline-hub-empty")
  });
}

/// A pipeline called `name` whose steps run `profile`, created as its first version (`None`, a step with no agent
/// profile, is refused: D-032).
pub(super) fn pipeline(agents: &Agents, name: &str, profile: Option<ResourceId>) -> ResourceId {
  pipeline_of(agents, name, [profile, profile])
}

/// A pipeline called `name` with a step *Plan* and a step *Build* that run `profiles` in turn.
pub(super) fn pipeline_of(agents: &Agents, name: &str, profiles: [Option<ResourceId>; 2]) -> ResourceId {
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  for (step, profile) in ["Plan", "Build"].into_iter().zip(profiles) {
    let key = edit.add_step(Name::new(step).expect("name"), None);
    edit.update(&key, |step| {
      step.kind = StepKind::Agent {
        profile,
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      };
    });
  }
  let body = edit.validate().expect("a pipeline");
  let mut db = ProjectDb::open(&agents.root).expect("database");
  db.create_published(
    Slug::from_name(name).expect("slug"),
    Name::new(name).expect("name"),
    &body,
    Timestamp::now(),
  )
  .expect("the pipeline")
  .0
  .id()
}

pub(super) fn use_pipeline(agents: &Agents, pipeline: ResourceId, tasks: usize) {
  let details = (0..tasks)
    .map(|number| TaskDetails {
      title: Name::new(&format!("Task {number}")).expect("name"),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: Some(pipeline),
      limits: TaskLimits::NONE,
    })
    .collect();
  ProjectDb::open(&agents.root)
    .expect("database")
    .create_tasks(IdempotencyKey::generate(), details, Timestamp::now())
    .expect("tasks");
}

/// A project with a ready pipeline that three tasks use, *Dependency update* whose second step runs a profile that is
/// no longer available (a pipeline is stored only as published versions, D-032, D-034), and one whose profile does not
/// exist (so it cannot run), with the hub not opened yet.
pub(super) fn setup() -> (Agents, [ResourceId; 3]) {
  let agents = Agents::new(true);
  let builder = agents.profile(true);
  let ready = pipeline(&agents, "Feature delivery", Some(builder));
  let draft = pipeline_of(
    &agents,
    "Dependency update",
    [Some(builder), Some(ResourceId::generate())],
  );
  let stuck = pipeline(&agents, "Quick fix", Some(ResourceId::generate()));
  use_pipeline(&agents, ready, 3);
  (agents, [ready, draft, stuck])
}

/// [`setup`] with the hub open on All.
pub(super) fn project() -> (Agents, [ResourceId; 3]) {
  let (mut agents, ids) = setup();
  open(&mut agents, PipelinesFilter::All, None);
  (agents, ids)
}

#[test]
fn pipeline_hub_groups_the_pipelines_and_counts_each_tab() {
  let (mut agents, [ready, draft, stuck]) = project();
  assert_eq!(agents.text("pipeline-hub-title"), "Pipelines");
  assert_eq!(agents.text("local-hub-local-label"), "Local");
  assert_eq!(agents.text("pipeline-hub-filter-all-count"), "3");
  assert_eq!(agents.text("pipeline-hub-filter-archived-count"), "0");
  assert_eq!(agents.text("pipeline-hub-section-needs-title"), "Needs you");
  assert_eq!(agents.text("pipeline-hub-section-needs-count"), "2");
  assert_eq!(agents.text("pipeline-hub-section-ready-title"), "Ready");
  assert_eq!(agents.text("pipeline-hub-section-ready-count"), "1");
  let row = |agents: &mut Agents, id: ResourceId, part: &str| agents.text(&format!("pipeline-hub-row-{id}-{part}"));
  assert_eq!(row(&mut agents, ready, "name"), "Feature delivery");
  assert_eq!(row(&mut agents, ready, "line"), "2 steps · used by 3 tasks");
  assert_eq!(row(&mut agents, ready, "version"), "v1");
  assert_eq!(row(&mut agents, draft, "version"), "v1");
  let line = row(&mut agents, draft, "line");
  assert!(
    line.starts_with("02 Build"),
    "it names the step whose profile is gone: {line}"
  );
  assert_eq!(row(&mut agents, stuck, "version"), "v1");
  assert_ne!(
    row(&mut agents, stuck, "line"),
    "2 steps · not used yet",
    "a pipeline that cannot run says what it lacks, not its steps"
  );
}

#[test]
fn pipeline_hub_the_panel_follows_the_chosen_row_and_the_route_keeps_it() {
  let (mut agents, [ready, draft, _]) = project();
  // Nothing named: the first row of the list, *Needs you* first, is open.
  assert_eq!(agents.text("team-hub-panel-title"), "Dependency update");
  agents.click(&format!("pipeline-hub-row-{ready}"));
  agents.until("the panel", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(ready))
      && s.text("team-hub-panel-title") == "Feature delivery"
  });
  assert_eq!(agents.text("team-hub-panel-kind"), "Pipeline");
  assert!(agents.text("team-hub-panel-version").starts_with("v1 · saved "));
  assert_eq!(agents.text("pipeline-hub-chip-0-label"), "v1");
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "Ready");
  assert_eq!(agents.text("pipeline-hub-chips-end"), "used by 3 tasks");

  agents.click(&format!("pipeline-hub-row-{draft}"));
  agents.until("the other pipeline", |s| {
    s.text("team-hub-panel-title") == "Dependency update"
  });
  assert!(agents.text("team-hub-panel-version").starts_with("v1 · saved "));
  assert_eq!(agents.text("pipeline-hub-chip-0-label"), "v1");
  assert!(agents.text("pipeline-hub-chip-1-label").starts_with("02 Build"));
  assert_eq!(agents.text("pipeline-hub-chips-end"), "not used yet");
}

#[test]
fn pipeline_hub_a_tab_keeps_the_chosen_pipeline_when_it_holds_it_and_a_route_lands_on_the_tab_that_does() {
  let (mut agents, [ready, _, stuck]) = setup();
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(stuck, Timestamp::now())
    .expect("archived");
  open(&mut agents, PipelinesFilter::Archived, Some(stuck));
  assert!(agents.exists(&format!("pipeline-hub-row-{stuck}")));
  assert!(
    !agents.exists(&format!("pipeline-hub-row-{ready}")),
    "Archived holds archived pipelines only"
  );
  assert_eq!(
    agents.text("pipeline-hub-filter-all-count"),
    "2",
    "the counts do not follow the tab"
  );

  // A route that names a pipeline only the other tab holds opens that tab with it, and is corrected to it.
  open(&mut agents, PipelinesFilter::Archived, Some(ready));
  agents.until("the corrected route", |s| {
    s.window.navigation().route_now() == view(PipelinesFilter::All, Some(ready))
  });
  assert_eq!(agents.text("team-hub-panel-title"), "Feature delivery");
}

#[test]
fn pipeline_hub_lists_the_archived_pipelines_under_their_own_tab() {
  let (mut agents, [ready, draft, stuck]) = setup();
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_local_resource(stuck, Timestamp::now())
    .expect("archived");
  open(&mut agents, PipelinesFilter::All, None);
  assert_eq!(agents.text("pipeline-hub-filter-all-count"), "2");
  assert!(
    !agents.exists(&format!("pipeline-hub-row-{stuck}")),
    "an archived one leaves All"
  );

  agents.click("pipeline-hub-filter-archived");
  agents.until("Archived", |s| s.exists(&format!("pipeline-hub-row-{stuck}")));
  assert!(!agents.exists(&format!("pipeline-hub-row-{ready}")) && !agents.exists(&format!("pipeline-hub-row-{draft}")));
  assert!(
    agents
      .text(&format!("pipeline-hub-row-{stuck}-line"))
      .starts_with("Archived ")
  );
  assert_eq!(agents.text("pipeline-hub-chip-1-label"), "Archived");
  assert!(
    !agents.exists("pipeline-hub-edit-steps"),
    "an archived pipeline has no editor to open"
  );
}

#[test]
fn pipeline_hub_a_project_without_pipelines_says_so_and_offers_a_new_one() {
  let mut agents = Agents::new(true);
  open(&mut agents, PipelinesFilter::All, None);
  assert_eq!(agents.text("pipeline-hub-empty-title"), "No pipelines yet");
  assert_eq!(agents.text("pipeline-hub-filter-all-count"), "0");
  agents.click("pipeline-hub-empty-new");
  agents.until("the new pipeline in the panel", |s| s.exists("pipeline-new-panel"));

  open(&mut agents, PipelinesFilter::Archived, None);
  assert_eq!(agents.text("pipeline-hub-empty-title"), "Nothing archived");
  assert_eq!(
    agents.text("pipeline-hub-empty-detail"),
    "An archived pipeline leaves the lists and New task's picker, and the runs that use it stop."
  );
  assert!(
    !agents.exists("pipeline-hub-empty-new"),
    "only a project with none offers one in its body"
  );
  agents.click("pipeline-hub-new");
  agents.until("New pipeline from the head", |s| s.exists("pipeline-new-panel"));
}

#[derive(Clone, Copy, Debug)]
struct Box {
  y: f32,
  height: f32,
}

fn bounds(agents: &mut Agents, id: &str) -> Box {
  let rect = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|e| e.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  Box {
    y: rect.y,
    height: rect.height,
  }
}

#[test]
fn pipeline_hub_the_chips_stand_14_under_the_head() {
  let (mut agents, _) = project();
  let scroll = bounds(&mut agents, "pipeline-hub-panel-scroll");
  let chips = bounds(&mut agents, "pipeline-hub-chips");
  // P1 `KGBgh`: 20 above the head, the head 39 high, 14 to the chips (`oYa3m` at 73).
  assert!(
    (chips.y - scroll.y - 73.0).abs() <= 1.0,
    "chips at {} of the panel",
    chips.y - scroll.y
  );
}

#[test]
fn pipeline_hub_each_empty_tab_has_the_icon_that_says_what_it_is() {
  let mut agents = Agents::new(true);
  open(&mut agents, PipelinesFilter::All, None);
  assert_eq!(
    glyph_of(&mut agents, "pipeline-hub-empty-glyph"),
    Icon::Workflow.glyph().to_string()
  );
  open(&mut agents, PipelinesFilter::Archived, None);
  assert_eq!(
    glyph_of(&mut agents, "pipeline-hub-empty-glyph"),
    Icon::Archive.glyph().to_string()
  );
}

#[test]
fn pipeline_hub_the_empty_body_keeps_the_designs_rhythm_to_its_button() {
  let mut agents = Agents::new(true);
  open(&mut agents, PipelinesFilter::All, None);
  let (detail, button) = (
    bounds(&mut agents, "pipeline-hub-empty-detail"),
    bounds(&mut agents, "pipeline-hub-empty-new"),
  );
  // P6 `goPlX`: the line, 10, a 4 high gap, 10, the 28 high button.
  assert!(
    (button.y - (detail.y + detail.height) - 24.0).abs() <= 1.0,
    "line {detail:?} to button {button:?}"
  );
  assert!((button.height - 28.0).abs() <= 1.0, "the button is {button:?}");
  let (body, tile) = (
    bounds(&mut agents, "pipeline-hub-empty-body"),
    bounds(&mut agents, "pipeline-hub-empty-glyph"),
  );
  let block = button.y + button.height - tile.y;
  assert!(
    (tile.y - body.y - (body.height - block) / 2.0).abs() <= 1.0,
    "the block of {block} is centered in {body:?}"
  );
}

/// Breaks the connections' settings so the hub can't read what its pipelines run on; the settings to put back.
fn break_connections(agents: &Agents) -> (rusqlite::Connection, String) {
  let db = rusqlite::Connection::open(agents.root.join(".orchester/orchester.db")).expect("fixture db");
  let settings: String = db
    .query_row("SELECT settings FROM connections", [], |row| row.get(0))
    .expect("settings");
  db.execute("UPDATE connections SET settings = 'broken'", [])
    .expect("corrupt disposable fixture");
  (db, settings)
}

#[test]
fn pipeline_hub_a_read_that_fails_says_so_with_its_reason_keeps_the_head_and_tries_again() {
  let (mut agents, _) = setup();
  let (db, settings) = break_connections(&agents);
  agents.window.navigation().go(view(PipelinesFilter::All, None));
  agents.until("the failure", |s| s.exists("pipeline-hub-load-error"));
  assert_eq!(
    agents.text("pipeline-hub-load-error-title"),
    "Couldn't read the pipelines"
  );
  assert_eq!(
    agents.text("pipeline-hub-load-error-detail"),
    "Nothing was changed. Try again."
  );
  assert_eq!(
    agents.text("pipeline-hub-load-error-reason"),
    "The data could not be read or written. The log has the details.",
    "the reason as the store gave it"
  );
  assert_eq!(
    glyph_of(&mut agents, "pipeline-hub-load-error-glyph"),
    Icon::CircleAlert.glyph().to_string()
  );
  assert!(
    !agents.exists("pipeline-hub-empty") && !agents.exists("pipeline-hub-list"),
    "nothing pretends to be empty"
  );
  for tab in ["all", "archived"] {
    assert!(
      agents.exists(&format!("pipeline-hub-filter-{tab}")),
      "the {tab} tab keeps its name"
    );
    // A count that is not read is a bone holding only the digit's clipped 0; the project has three pipelines.
    assert_eq!(
      agents
        .window
        .texts(&format!("pipeline-hub-filter-{tab}-count"))
        .map(|words| words.concat())
        .as_deref(),
      Some("0"),
      "the {tab} tab shows a bone, not a count"
    );
  }
  assert!(agents.exists("pipeline-hub-title"), "the title stays");
  let route = agents.window.navigation().route_now();
  agents.click("pipeline-hub-new");
  assert_eq!(
    agents.window.navigation().route_now(),
    route,
    "New pipeline waits for a read that succeeds"
  );

  db.execute("UPDATE connections SET settings = ?1", [&settings])
    .expect("restore fixture");
  agents.click("pipeline-hub-load-error-retry");
  agents.until("the pipelines in its place", |s| {
    s.exists("pipeline-hub-master-detail") && !s.exists("pipeline-hub-load-error")
  });
  assert_eq!(agents.text("pipeline-hub-filter-all-count"), "3");
  agents.click("pipeline-hub-new");
  agents.until("New pipeline once the pipelines are read", |s| {
    s.exists("pipeline-new-panel")
  });
}

#[test]
fn pipeline_hub_a_tab_with_nothing_in_it_says_so_while_the_others_keep_their_pipelines() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  pipeline(&agents, "Feature delivery", Some(builder));
  open(&mut agents, PipelinesFilter::Archived, None);
  assert_eq!(agents.text("pipeline-hub-empty-title"), "Nothing archived");
  assert_eq!(agents.text("pipeline-hub-filter-all-count"), "1");
  assert_eq!(agents.text("pipeline-hub-filter-archived-count"), "0");
  assert!(
    !agents.exists("pipeline-hub-empty-new") && !agents.exists("pipeline-hub-list"),
    "only a project with no pipelines at all offers New pipeline in its body"
  );
  open(&mut agents, PipelinesFilter::All, None);
  assert!(!agents.exists("pipeline-hub-empty"), "the pipeline is still under All");
}

/// The icon glyph inside the element `id`, which `Agents::text` leaves out.
fn glyph_of(agents: &mut Agents, id: &str) -> String {
  fn collect(element: lurq::node::element::ElementRef<'_>, into: &mut String) {
    into.extend(element.text_content().into_iter().flat_map(str::chars));
    for child in element.children() {
      collect(child, into);
    }
  }
  let element = agents
    .window
    .tree
    .get_element_by_id(id)
    .unwrap_or_else(|| panic!("{id} exists"));
  let mut glyph = String::new();
  collect(element, &mut glyph);
  glyph
}
