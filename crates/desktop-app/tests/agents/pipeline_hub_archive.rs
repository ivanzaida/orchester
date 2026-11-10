//! The Pipelines hub's archive confirmation and archived panel after their design verification (PV2-12, round 2): the
//! confirmation names the tasks that use the pipeline and what happens to them, the archived panel says the runs it
//! stopped only when it stopped some, and the ⋯ menu opens under its button with no tooltip beside it. `cargo test -p
//! orchester-app --test agents pipeline_hub_archive`.

use std::time::{Duration, Instant};

use lurq::app::events::MouseButton;
use orchester_app::{routes::PipelinesFilter, ui::tooltip::TOOLTIP_DELAY};
use orchester_core::{id::ResourceId, name::Name, resource::ResourceKind, store::ResourceStore, time::Timestamp};
use orchester_store::{ArchiveStop, Archived, ProjectDb};

use super::{
  active::{pipeline as pinned_pipeline, running},
  harness::Agents,
  pipeline_hub::{open, pipeline, project, use_pipeline},
  pipeline_hub_menu::{archived, key, names, open_menu, start_rename},
};

/// Opens the confirmation of archiving `id` from its ⋯ menu.
fn ask(agents: &mut Agents, id: ResourceId) {
  open_menu(agents, id);
  agents.click("pipeline-hub-menu-archive");
  agents.until("the confirmation", |s| s.exists("team-hub-archive-confirm"));
  agents.until("what uses it is read", |s| s.exists("team-hub-archive-used-heading"));
}

#[test]
fn pipeline_hub_archive_names_the_tasks_that_use_the_pipeline_and_what_happens_to_them() {
  let (mut agents, [ready, ..]) = project();
  use_pipeline(&agents, ready, 4);
  agents.window.pass();
  ask(&mut agents, ready);
  assert_eq!(agents.text("team-hub-archive-used-heading"), "USED BY · 7");
  for index in 0..5 {
    let row = agents.text(&format!("team-hub-archive-task-{index}-name"));
    assert!(row.contains(" · Task "), "task {index}: {row}");
  }
  assert!(
    !agents.exists("team-hub-archive-task-5"),
    "only the first five are named"
  );
  assert_eq!(agents.text("team-hub-archive-tasks-more-name"), "and 2 more");
  assert_eq!(
    agents.text("team-hub-archive-tasks-effect"),
    "Those tasks cannot start a run until they use another pipeline or Feature delivery is restored."
  );
  assert!(
    agents.exists("team-hub-archive-runs-none"),
    "the runs that stop are still listed, none here"
  );
  assert!(archived(&agents).is_empty(), "asking archives nothing");
}

#[test]
fn pipeline_hub_archive_a_pipeline_no_task_uses_says_so_and_promises_nothing_about_tasks() {
  let (mut agents, [_, draft, _]) = project();
  ask(&mut agents, draft);
  assert_eq!(agents.text("team-hub-archive-used-heading"), "USED BY · 0");
  assert_eq!(agents.text("team-hub-archive-used-none"), "No task uses it.");
  assert!(!agents.exists("team-hub-archive-tasks-effect"));
}

#[test]
fn pipeline_hub_archive_one_task_is_named_in_the_singular() {
  let (mut agents, [_, _, stuck]) = project();
  use_pipeline(&agents, stuck, 1);
  agents.window.pass();
  ask(&mut agents, stuck);
  assert_eq!(agents.text("team-hub-archive-used-heading"), "USED BY · 1");
  assert!(!agents.exists("team-hub-archive-tasks-more"));
  assert_eq!(
    agents.text("team-hub-archive-tasks-effect"),
    "That task cannot start a run until it uses another pipeline or Quick fix is restored."
  );
}

/// Archives the pipeline called `name` directly in the database, recording that archiving stopped a run of it when
/// `stopped`.
fn archive_directly(agents: &Agents, name: &str, stopped: bool) -> ResourceId {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  if stopped {
    let snapshot = pinned_pipeline(&mut db, agents.profile(true));
    let run = running(&mut db, snapshot, Timestamp::from_unix_millis(1_700_000_100_000));
    let id = pipeline_id(&db, name);
    db.record_archive_stop(&ArchiveStop {
      run: run.id(),
      task: run.task(),
      resource: Name::new(name).expect("name"),
      stopped_at: Timestamp::now(),
      archived: Some(Archived {
        id,
        kind: ResourceKind::Pipeline,
        by: None,
      }),
    })
    .expect("recorded");
  }
  let id = pipeline_id(&db, name);
  db.archive_local_resource(id, Timestamp::now()).expect("archived");
  id
}

fn pipeline_id(db: &ProjectDb, name: &str) -> ResourceId {
  db.resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .into_iter()
    .find(|resource| resource.name().as_str() == name)
    .expect("the pipeline")
    .id()
}

#[test]
fn pipeline_hub_archived_panel_does_not_claim_runs_were_stopped_when_none_were() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let kept = pipeline(&agents, "Release notes", Some(builder), true);
  let id = archive_directly(&agents, "Release notes", false);
  assert_eq!(id, kept);
  open(&mut agents, PipelinesFilter::Archived, Some(id));
  agents.until("the archived panel", |s| s.exists("pipeline-hub-archived-notice"));
  let notice = agents.text("pipeline-hub-archived-notice");
  assert!(
    notice.contains("Restore puts Release notes back in the lists and New task as it was."),
    "{notice}"
  );
  assert!(!notice.contains("stopped") && !notice.contains("stay"), "{notice}");
  assert_eq!(
    agents.text("team-hub-actions-hint"),
    "Back in the lists and New task at once"
  );
}

#[test]
fn pipeline_hub_archived_panel_names_the_runs_its_archive_stopped_with_their_count() {
  let mut agents = Agents::new(true);
  let id = archive_directly(&agents, "Delivery", true);
  open(&mut agents, PipelinesFilter::Archived, Some(id));
  agents.until("the stopped run is named", |s| {
    s.exists("pipeline-hub-archived-notice") && s.text("pipeline-hub-archived-notice").contains("stopped")
  });
  let notice = agents.text("pipeline-hub-archived-notice");
  assert!(
    notice.contains("Archiving stopped the run that used it, KON-1 · Test the pinned profile."),
    "{notice}"
  );
  assert!(notice.contains("that run stays stopped"), "{notice}");
  assert_eq!(
    agents.text("team-hub-actions-hint"),
    "Back in the lists and New task at once · stopped runs stay stopped"
  );
}

struct Rect {
  x: f32,
  y: f32,
  width: f32,
  height: f32,
}

fn rect(agents: &mut Agents, id: &str) -> Rect {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|e| e.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  Rect {
    x: found.x,
    y: found.y,
    width: found.width,
    height: found.height,
  }
}

/// Lets the pointer rest long enough for a tooltip.
fn wait_for_tooltip(agents: &mut Agents) {
  agents
    .window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  agents.window.pass();
}

#[test]
fn pipeline_hub_menu_opens_under_its_button_4_below_and_right_aligned_with_no_tooltip_beside_it() {
  let (mut agents, [ready, ..]) = project();
  agents.click(&format!("pipeline-hub-row-{ready}"));
  agents.until("the panel", |s| s.exists("pipeline-hub-more"));

  // Resting on the button shows its tooltip while the menu is closed.
  let button = rect(&mut agents, "pipeline-hub-more");
  let scale = agents.window.tree.scale_factor();
  let (x, y) = (
    (button.x + button.width / 2.0) * scale,
    (button.y + button.height / 2.0) * scale,
  );
  agents.window.tree.mouse_move(x, y);
  agents.window.pass();
  wait_for_tooltip(&mut agents);
  assert!(
    agents.exists("pipeline-hub-more-tooltip"),
    "the closed button has its tooltip"
  );

  agents.click("pipeline-hub-more");
  agents.until("the menu", |s| s.exists("pipeline-hub-menu"));
  wait_for_tooltip(&mut agents);
  assert!(
    !agents.exists("pipeline-hub-more-tooltip"),
    "the open menu's button has no tooltip beside it"
  );

  let menu = rect(&mut agents, "pipeline-hub-menu");
  let button = rect(&mut agents, "pipeline-hub-more");
  assert!(
    (menu.y - (button.y + button.height) - 4.0).abs() <= 0.5,
    "4 under the button: menu at {}, button ends at {}",
    menu.y,
    button.y + button.height
  );
  assert!(
    ((menu.x + menu.width) - (button.x + button.width)).abs() <= 0.5,
    "right-aligned with the button: menu ends at {}, button at {}",
    menu.x + menu.width,
    button.x + button.width
  );
}

#[test]
fn pipeline_hub_rename_shows_the_new_name_the_moment_the_field_closes() {
  let (mut agents, [ready, ..]) = project();
  start_rename(&mut agents, ready);
  agents.set("pipeline-hub-rename", "Feature flow");
  key(&mut agents, "Enter");
  agents.until("the field closes", |s| !s.exists("pipeline-hub-rename"));
  assert_eq!(
    agents.text("team-hub-panel-title"),
    "Feature flow",
    "the title does not go back to the old name until the hub reads again"
  );
}

#[test]
fn pipeline_hub_rename_leaving_the_field_keeps_the_name() {
  let (mut agents, [ready, ..]) = project();
  start_rename(&mut agents, ready);
  agents.set("pipeline-hub-rename", "Something else");
  agents
    .window
    .tree
    .get_element_by_id_mut("pipeline-hub-rename")
    .expect("the field")
    .focus();
  // Moving the focus to another control blurs the field.
  let away = rect(&mut agents, "pipeline-hub-title");
  let scale = agents.window.tree.scale_factor();
  let (x, y) = (
    (away.x + away.width / 2.0) * scale,
    (away.y + away.height / 2.0) * scale,
  );
  agents.window.tree.mouse_move(x, y);
  agents.window.tree.mouse_down(x, y, MouseButton::Left);
  agents.window.tree.mouse_up(x, y, MouseButton::Left);
  agents.window.pass();
  agents.until("the field closes", |s| !s.exists("pipeline-hub-rename"));
  assert_eq!(agents.text("team-hub-panel-title"), "Feature delivery");
  assert_eq!(names(&agents, ready).0, "Feature delivery");
}
