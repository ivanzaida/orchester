//! The *Pipeline* settings of a local pipeline's panel with every field of the old Settings tab (`Pipelines · v2` P17
//! `LzGvn`, PV2-25): the slug is fixed text with a lock after the description, the stored one, kept by a rename and by
//! a saved version; *Who can use it* is not there. `cargo test -p orchester-app --test agents pipeline_settings`.

use orchester_app::{routes::PipelinesFilter, ui::icon::Icon};
use orchester_core::{id::ResourceId, resource::Pipeline, store::ResourceStore};
use orchester_store::ProjectDb;
use orchester_workspace::pipelines::rename::rename_in;

use super::{
  harness::Agents,
  pipeline_hub::{open, pipeline, project},
  pipeline_hub_menu::{key, names, start_rename},
};

const SLUG: &str = "pipeline-hub-slug";

/// Opens the panel of `id` and waits for its settings.
fn settings_of(agents: &mut Agents, id: ResourceId) {
  open(agents, PipelinesFilter::All, Some(id));
  agents.until("the settings", |s| {
    s.exists(SLUG) && s.exists(&format!("pipeline-hub-row-{id}-name"))
  });
}

/// Where the element `id` stands from the top.
fn top(agents: &mut Agents, id: &str) -> f32 {
  agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .y
}

/// The glyph the icon `id` draws.
fn glyph(agents: &mut Agents, id: &str) -> String {
  agents
    .window
    .tree
    .get_element_by_id(id)
    .and_then(|element| element.text_content().map(str::to_owned))
    .unwrap_or_else(|| panic!("{id} draws a glyph"))
}

#[test]
fn pipeline_settings_show_the_slug_as_fixed_text_with_a_lock_after_the_description() {
  let (mut agents, [ready, ..]) = project();
  settings_of(&mut agents, ready);
  assert_eq!(agents.text("pipeline-hub-slug-row-label"), "Slug");
  assert_eq!(
    agents.text("pipeline-hub-slug-row-sub"),
    "Fixed once saved. Renaming keeps it."
  );
  assert_eq!(agents.text(SLUG), "feature-delivery");
  assert_eq!(
    glyph(&mut agents, "pipeline-hub-slug-locked"),
    Icon::Lock.glyph().to_string()
  );
  assert!(
    agents
      .window
      .tree
      .get_element_by_id_mut(SLUG)
      .and_then(|element| element.as_text_input())
      .is_none(),
    "the slug is text, not a field"
  );
  let (description, slug, starts) = (
    top(&mut agents, "pipeline-hub-description-row"),
    top(&mut agents, "pipeline-hub-slug-row"),
    top(&mut agents, "pipeline-hub-starts"),
  );
  assert!(
    description < slug && slug < starts,
    "the slug stands between the description and Starts: {description} {slug} {starts}"
  );
  let section = agents.text("pipeline-hub-settings");
  assert!(
    !section.contains("Who can use it") && !section.contains("This project"),
    "the old tab's access row is not drawn: {section}"
  );
}

#[test]
fn pipeline_settings_show_the_stored_slug_not_the_one_the_name_would_make() {
  let mut agents = Agents::new(true);
  let builder = agents.profile(true);
  let renamed = pipeline(&agents, "Legacy flow", Some(builder));
  let servers = agents.window.launch.services().expect("the session").servers.clone();
  rename_in((&agents.root, &servers), renamed, "Release train").expect("renamed");
  settings_of(&mut agents, renamed);
  assert_eq!(agents.text("team-hub-panel-title"), "Release train");
  assert_eq!(agents.text(SLUG), "legacy-flow");
}

#[test]
fn pipeline_settings_keep_the_slug_when_the_pipeline_is_renamed_in_the_hub() {
  let (mut agents, [ready, ..]) = project();
  start_rename(&mut agents, ready);
  agents.set("pipeline-hub-rename", "Feature flow");
  key(&mut agents, "Enter");
  agents.until("renamed", |s| {
    s.text(&format!("pipeline-hub-row-{ready}-name")) == "Feature flow" && s.exists(SLUG)
  });
  assert_eq!(agents.text(SLUG), "feature-delivery");
  assert_eq!(
    names(&agents, ready),
    ("Feature flow".to_owned(), "feature-delivery".to_owned())
  );
}

#[test]
fn pipeline_settings_keep_the_slug_when_a_new_version_is_saved() {
  let (mut agents, [ready, ..]) = project();
  settings_of(&mut agents, ready);
  agents.set("pipeline-hub-description", "Plan, then build");
  agents.press_until("pipeline-hub-save", "v2 is published", |s| {
    let db = ProjectDb::open(&s.root).expect("database");
    db.versions::<Pipeline>(ready).expect("versions").len() == 2
  });
  agents.until("the panel shows v2", |s| {
    s.text("pipeline-hub-chip-0") == "v2" && s.exists(SLUG)
  });
  assert_eq!(agents.text(SLUG), "feature-delivery");
  assert_eq!(names(&agents, ready).1, "feature-delivery");
}
