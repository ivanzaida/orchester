//! The team hub's *Archived* (`Agents · Archived · admin · Restore` A36 `qvMZD`, `· member` M7 `Ml7HN`) and what
//! opens it (A37 `AFbIb`, A22 `pUu78`): what the team archived, read-only; an admin's *Restore*, which the stand-in
//! team server takes as `PATCH` with `archived: false`, stays pending until the hub reads it again, and a refusal says
//! why in place; a member has no Restore, only the line that says who restores it; *Open →* on Builder's *Ready here?*
//! row, its pinned *Open pencil-design* and the stopped card's pencil-design row land on it there by route. On the QA
//! states `team-archived` (a member) and `team-archived-admin`.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_hub_archived`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  test_support::Window,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::ResourceKind,
  store::ResourceStore,
};
use orchester_store::ResourceSharing;

use super::{
  hub_edit::{db, opened},
  hub_keep::{loading, reads},
  not_ready::on_task,
  team::State,
};

const RESTORE: &str = "team-hub-restore";
const ADMIN: (&str, State) = ("team-archived-admin", State::TeamArchivedAdmin);
const MEMBER: (&str, State) = ("team-archived", State::TeamArchived);

/// The seed's team MCP server `pencil-design`, which the project's lists leave out once archived.
fn pencil(window: &Window) -> ResourceId {
  rusqlite::Connection::open(window.data.repo("repo-kontur").join(".orchester/orchester.db"))
    .expect("the project database")
    .query_row("SELECT id FROM resources WHERE name = 'pencil-design'", [], |row| {
      row.get::<_, String>(0)
    })
    .expect("the seed's pencil-design")
    .parse()
    .expect("a resource id")
}

/// The team's Builder: the catalogue has a local one of the same name.
fn builder(window: &Window) -> ResourceId {
  let db = db(window);
  db.resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| {
      resource.name().as_str() == "Builder"
        && matches!(db.resource_sharing(resource.id()), Ok(Some(ResourceSharing::Shared(_))))
    })
    .expect("the team's Builder")
    .id()
}

/// The window on `pencil-design` in the hub's Archived in `state`, once its panel shows.
fn on_archived(state: (&str, State)) -> (Window, TeamServer, (ProjectId, ResourceId)) {
  let (window, server, (project, pencil)) = opened(state, "team-hub-chip-archived", |window| {
    let pencil = pencil(window);
    (HubView::new(HubFilter::Archived, Some(pencil)), Some(pencil))
  });
  (window, server, (project, pencil.expect("pencil-design")))
}

/// Whether the stand-in holds `pencil-design` archived.
fn archived_on(server: &TeamServer) -> bool {
  server
    .resources("kontur")
    .iter()
    .any(|(resource, _)| resource["name"] == "pencil-design" && resource["archived"] == true)
}

/// The route's view of the hub.
fn view(window: &Window) -> HubView {
  view_now(window).unwrap_or_else(|| panic!("the hub, not {:?}", window.navigation().route_now()))
}

/// Presses `id`, a fix of Builder's panel, until it opens the hub's Archived: a fix stands inert while the hub is read
/// again behind the panel.
fn open_archived(window: &mut Window, id: &str) {
  window.until("the fix to open Archived", |window| {
    window.press(id);
    view_now(window).is_some_and(|view| view.filter == HubFilter::Archived)
  });
}

/// The route's view of the hub, while it is the hub.
fn view_now(window: &Window) -> Option<HubView> {
  match window.navigation().route_now() {
    Route::Project(_, ProjectPage::AgentsHub(view)) => Some(view),
    _ => None,
  }
}

/// The hub shows `pencil` in Archived, picked, by route.
fn shown_in_archived(window: &mut Window, pencil: ResourceId) {
  window.until("pencil-design in Archived", |window| {
    window
      .texts("team-hub-chip-archived")
      .is_some_and(|chip| chip == ["Archived"])
  });
  assert_eq!(view(window), HubView::new(HubFilter::Archived, Some(pencil)));
  assert_eq!(window.text("team-hub-panel-title"), "pencil-design");
}

#[test]
fn team_qa_hub_archived_an_admin_reads_what_the_team_archived_with_restore() {
  let (mut window, _server, (_, pencil)) = on_archived(ADMIN);
  let row = format!("team-hub-row-{pencil}");
  assert_eq!(window.text(&format!("{row}-name")), "pencil-design");
  let line = window.text(&format!("{row}-line"));
  assert!(line.starts_with("Archived by dana-k · "), "{line}");
  assert_eq!(window.text(&format!("{row}-tag")), "mcp");
  assert_eq!(window.text("team-hub-section-archived-title"), "Archived");
  assert_eq!(window.text("team-hub-panel-kind"), "MCP server");
  assert!(
    window
      .text("team-hub-panel-version")
      .starts_with("Archived by dana-k · "),
    "{}",
    window.text("team-hub-panel-version")
  );
  assert_eq!(window.text("team-hub-chips-end"), "was used by Builder");
  let notice = window.texts("team-hub-archived-notice").expect("the notice").join(" ");
  assert!(notice.contains("Archived: hidden from lists and pickers"), "{notice}");
  assert!(notice.contains("KON-12 · Add retry to webhook sender"), "{notice}");
  assert!(
    notice.contains("Until then Builder, which chooses it, is not ready on any computer."),
    "{notice}"
  );
  assert_eq!(
    window.text("team-hub-from-team-note"),
    "· read-only until it is restored"
  );
  assert!(window.texts("team-hub-from-team-program-value").is_some());
  assert_eq!(window.text(RESTORE), "Restore pencil-design");
  assert!(
    window.texts("team-hub-member-lock").is_none(),
    "an admin reads no lock line"
  );
}

#[test]
fn team_qa_hub_archived_restore_stays_pending_until_the_hub_reads_it_again() {
  let (mut window, server, (project, pencil)) = on_archived(ADMIN);
  let row = format!("team-hub-row-{pencil}");
  let hold = reads(&window);
  hold.set(true);
  window.press(RESTORE);
  window.until("the restore on the server", |_| !archived_on(&server));
  // The server took it; the hub's read again is held: the entry stays in Archived, Restore pending.
  let mut passes = 0;
  window.until("the hub's read held", |window| {
    assert!(
      window.texts(&row).is_some(),
      "the entry left before the hub read it again"
    );
    assert!(loading(window, RESTORE), "Restore shows it is pending");
    passes += 1;
    passes == 40
  });
  hold.set(false);
  window.until("the entry gone from Archived", |window| window.texts(&row).is_none());
  assert!(window.texts(RESTORE).is_none());
  // Restored, it is back among the team's MCP servers.
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::new(HubFilter::McpServers, Some(pencil))),
  ));
  window.until("pencil-design among the MCP servers", |window| {
    window.texts(&row).is_some() && window.texts("team-hub-chip-archived").is_none()
  });
}

#[test]
fn team_qa_hub_archived_a_refused_restore_says_why_and_keeps_it_archived() {
  let (mut window, server, (_, pencil)) = on_archived(ADMIN);
  server.refuse_writes_as_not_admin("kontur");
  window.press(RESTORE);
  window.until("the refusal", |window| window.texts("team-hub-restore-error").is_some());
  assert_eq!(
    window.text("team-hub-restore-error"),
    "Only team admins restore the team's resources: the team server refused it. Nothing was restored."
  );
  assert!(archived_on(&server), "nothing was restored");
  assert!(
    window.texts(&format!("team-hub-row-{pencil}")).is_some(),
    "it stays in Archived"
  );
  assert!(!loading(&mut window, RESTORE), "Restore can be pressed again");
}

#[test]
fn team_qa_hub_archived_a_member_reads_it_without_restore() {
  let (mut window, _server, (_, pencil)) = on_archived(MEMBER);
  assert_eq!(window.text(&format!("team-hub-row-{pencil}-name")), "pencil-design");
  assert!(window.texts(RESTORE).is_none(), "a member has no Restore");
  assert!(window.texts("team-hub-actions").is_none(), "nor an action bar");
  let lock = window.text("team-hub-member-lock");
  assert!(lock.starts_with("Only team admins restore it: "), "{lock}");
  assert_eq!(
    window.text("team-hub-from-team-note"),
    "· read-only until it is restored"
  );
}

#[test]
fn team_qa_hub_archived_open_on_builders_ready_row_lands_on_it_in_archived() {
  let (mut window, _server, (project, builder)) = opened(ADMIN, "team-hub-readiness-count", |window| {
    let builder = builder(window);
    (HubView::new(HubFilter::Profiles, Some(builder)), Some(builder))
  });
  // Builder's *Ready here?* stands under its other sections: the window is tall enough to show it.
  window.tree.resize(1440, 2400);
  let pencil = pencil(&window);
  let builder = builder.expect("Builder");
  let ready = format!("team-hub-ready-mcp-{pencil}");
  window.until("pencil-design's row", |window| {
    window.texts(&format!("{ready}-words")).is_some()
  });
  assert_eq!(
    window.text(&format!("{ready}-words")),
    "The team archived it · Builder can't run until an admin restores it or Builder stops choosing it"
  );
  assert_eq!(window.text("team-hub-readiness-count"), "1 missing");
  assert_eq!(window.text("team-hub-fix-open-archived"), "Open pencil-design");
  open_archived(&mut window, &format!("{ready}-open-archived"));
  shown_in_archived(&mut window, pencil);
  window.navigation().back();
  window.until("Builder again", |window| {
    window.texts(&format!("{ready}-words")).is_some()
  });
  assert_eq!(view(&window), HubView::new(HubFilter::Profiles, Some(builder)));
  window.navigation().forward();
  shown_in_archived(&mut window, pencil);
  // The pinned action opens it too.
  window.navigation().go(Route::Project(
    project,
    ProjectPage::hub(HubView::new(HubFilter::Profiles, Some(builder))),
  ));
  window.until("the pinned action", |window| {
    window.texts("team-hub-fix-open-archived").is_some()
  });
  open_archived(&mut window, "team-hub-fix-open-archived");
  shown_in_archived(&mut window, pencil);
}

#[test]
fn team_qa_hub_archived_open_on_the_stopped_cards_resource_lands_on_it_in_archived() {
  let (mut window, _server, _) = on_task(MEMBER, "KON-12");
  window.until("the resource's Open", |window| {
    window.texts("task-archived-resource-open").is_some()
  });
  let pencil = pencil(&window);
  window.press("task-archived-resource-open");
  shown_in_archived(&mut window, pencil);
  window.navigation().back();
  window.until("the task page again", |window| window.texts("task-archived").is_some());
}
