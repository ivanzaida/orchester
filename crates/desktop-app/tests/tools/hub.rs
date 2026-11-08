//! A local project's Agents hub hosts the tool's editor as its panel (`Agents · v2` L1, rule 11): the list beside it, none
//! of the old page around it, and the draft, Publish and Archive as the editor has them.
use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::{resource::Tool, store::ResourceStore};
use orchester_store::ProjectDb;

use super::{harness::Tools, support::project_id};

#[test]
fn tools_are_edited_in_the_hubs_panel_with_their_draft() {
  let mut t = Tools::new();
  let id = t.seed("Deploy", true);
  t.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::AgentsHub(HubView::new(HubFilter::Tools, Some(id))),
  ));
  t.ready();
  assert!(t.exists("local-hub-filter"), "the hub's list stands beside the editor");
  for page in ["tools-library", "tool-crumbs", "agents-hub-title"] {
    assert!(!t.exists(page), "{page} belongs to the old page");
  }
  t.set("tool-description", "Deploys the change.");
  let repo = t.repo.clone();
  t.window.until("the draft saved", move |w| {
    w.tree.tick_futures();
    w.pass();
    ProjectDb::open(&repo)
      .expect("database")
      .draft::<Tool>(id)
      .expect("draft read")
      .is_some()
  });
  assert!(t.exists("tool-publish"), "Publish is the editor's own");
  assert!(t.exists("tool-more"), "so is its Archive menu");
}
