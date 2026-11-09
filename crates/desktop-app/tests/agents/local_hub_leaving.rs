//! Leaving a local MCP server's panel while its edits are not saved (`Agents · MCP server · editing` L8, ORC-842): every
//! way out asks the same question, not only a press on another row. `cargo test -p orchester-app --test agents
//! local_hub_leaving`.

use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};

use super::{
  harness::Agents,
  local_hub::{open, server, show},
  support::project_id,
};

const DISCARD: &str = "local-hub-leave-discard";
const KEEP: &str = "local-hub-leave-cancel";

fn hub(view: HubView) -> Route {
  Route::Project(project_id(), ProjectPage::AgentsHub(view))
}

/// Edits the server's arguments, so the panel has changes that are not saved.
fn edit(agents: &mut Agents) {
  agents.set("connection-mcp-arguments", "--stdio");
  agents.until("unsaved", |s| s.exists("local-hub-mcp-save"));
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
    agents.value("connection-mcp-arguments"),
    "--stdio",
    "{id}: Keep editing keeps what was typed"
  );

  agents.click(id);
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the place", |s| s.window.navigation().route_now() == *place);
}

#[test]
fn local_hub_leaving_a_filter_with_unsaved_edits_asks_first() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);
  edit(&mut agents);

  asks_then_goes(
    &mut agents,
    &hub(HubView::on_server(HubFilter::McpServers, docs)),
    "local-hub-filter-profiles",
    &hub(HubView::new(HubFilter::Profiles, None)),
  );
}

#[test]
fn local_hub_leaving_by_the_head_with_unsaved_edits_asks_first() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  let here = hub(HubView::on_server(HubFilter::All, docs));
  for (id, place) in [
    (
      "local-hub-share",
      ProjectPage::Settings(orchester_app::routes::ProjectSettingsTab::Sharing),
    ),
    ("local-hub-memory", ProjectPage::Memory(None)),
    (
      "local-hub-import",
      ProjectPage::Import(orchester_app::routes::ImportTab::All),
    ),
  ] {
    open(&mut agents, HubView::on_server(HubFilter::All, docs));
    agents.until("the server's panel", |s| s.exists("local-hub-mcp-server"));
    edit(&mut agents);
    asks_then_goes(&mut agents, &here, id, &Route::Project(project_id(), place));
  }
}

#[test]
fn local_hub_new_menu_with_unsaved_edits_asks_before_it_opens_a_page() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  open(&mut agents, HubView::on_server(HubFilter::All, docs));
  agents.until("the server's panel", |s| s.exists("local-hub-mcp-server"));
  edit(&mut agents);
  agents.click("local-hub-new");
  agents.until("the menu", |s| s.exists("local-hub-new-profiles"));

  agents.click("local-hub-new-profiles");
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(
    agents.window.navigation().route_now(),
    hub(HubView::on_server(HubFilter::All, docs)),
    "no page opened under the question"
  );
  agents.click(DISCARD);
  agents.until("the page that makes a profile", |s| {
    s.window.navigation().route_now()
      == Route::Project(
        project_id(),
        ProjectPage::NewAgent(orchester_app::routes::AgentTab::Profile),
      )
  });
}

#[test]
fn local_hub_leaving_without_edits_goes_at_once() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  show(&mut agents, docs);

  agents.click("local-hub-filter-profiles");
  agents.until("the filter", |s| {
    s.window.navigation().route_now() == hub(HubView::new(HubFilter::Profiles, None))
  });
  assert!(!agents.exists(DISCARD), "nothing to lose, nothing asked");
}
