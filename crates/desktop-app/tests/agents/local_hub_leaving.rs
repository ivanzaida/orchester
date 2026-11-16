//! Leaving a local MCP server's panel while its edits are not saved (`Agents · MCP server · editing` L8, ORC-842):
//! every way out asks the same question, not only a press on another row. `cargo test -p orchester-app --test agents
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

/// The head's *New <kind>* leaves the panel for the kind's form: it asks first, as every way out does.
#[test]
fn local_hub_leaving_by_the_heads_new_with_unsaved_edits_asks_first() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  for (filter, made) in [
    (HubFilter::All, HubFilter::Profiles),
    (HubFilter::McpServers, HubFilter::McpServers),
  ] {
    let here = hub(HubView::on_server(filter, docs));
    open(&mut agents, HubView::on_server(filter, docs));
    agents.until("the server's panel", |s| s.exists("local-hub-mcp-server"));
    edit(&mut agents);
    asks_then_goes(
      &mut agents,
      &here,
      "local-hub-new",
      &Route::Project(project_id(), ProjectPage::hub(HubView::adding(made))),
    );
  }
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

/// Edits the server `docs`'s panel, opened after the Profiles filter, and returns where Back goes and where the panel
/// is.
fn editing_after_profiles(agents: &mut Agents) -> (Route, Route) {
  let docs = server(agents, "docs");
  open(agents, HubView::new(HubFilter::Profiles, None));
  let before = agents.window.navigation().route_now();
  show(agents, docs);
  edit(agents);
  (before, agents.window.navigation().route_now())
}

/// Leaves by `leave`, which the navigation holds: the question comes, nothing moves, *Keep editing* keeps what was
/// typed, and leaving again and *Discard changes* goes. Returns the route after.
fn held_then_goes(agents: &mut Agents, here: &Route, leave: impl Fn(&mut Agents)) -> Route {
  leave(agents);
  agents.until("the question", |s| s.exists(DISCARD));
  assert_eq!(
    &agents.window.navigation().route_now(),
    here,
    "nothing moved while it asks"
  );
  agents.click(KEEP);
  agents.until("kept", |s| !s.exists(DISCARD));
  assert_eq!(
    agents.value("connection-mcp-arguments"),
    "--stdio",
    "Keep editing keeps what was typed"
  );
  assert_eq!(&agents.window.navigation().route_now(), here);

  leave(agents);
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the place", |s| s.window.navigation().route_now() != *here);
  agents.window.navigation().route_now()
}

#[test]
fn local_hub_leaving_by_back_with_unsaved_edits_asks_first() {
  let mut agents = Agents::new(true);
  let (before, here) = editing_after_profiles(&mut agents);

  let after = held_then_goes(&mut agents, &here, |agents| agents.window.navigation().back());

  assert_eq!(after, before, "Back went where it goes");
}

#[test]
fn local_hub_leaving_by_another_tab_with_unsaved_edits_asks_first() {
  let mut agents = Agents::new(true);
  let (_, here) = editing_after_profiles(&mut agents);
  let tab = agents.window.navigation().tabs_now().active().id();

  held_then_goes(&mut agents, &here, |agents| agents.window.navigation().open_tab());

  assert_ne!(
    agents.window.navigation().tabs_now().active().id(),
    tab,
    "the new tab is shown after Discard changes"
  );
}

#[test]
fn local_hub_leaving_by_the_sidebar_with_unsaved_edits_asks_first() {
  let mut agents = Agents::new(true);
  let (_, here) = editing_after_profiles(&mut agents);
  assert!(agents.exists("nav-pipelines"), "the window has the sidebar");

  let after = held_then_goes(&mut agents, &here, |agents| agents.click("nav-pipelines"));

  assert!(
    matches!(after, Route::Project(_, ProjectPage::Pipelines)),
    "the sidebar's place: {after:?}"
  );
}
