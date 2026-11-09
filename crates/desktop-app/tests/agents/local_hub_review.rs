//! The Agents hub of a local project, after its review (ORC-842): the archive that could not stop a run says so where
//! it survives, and an archived server's name stays taken. `cargo test -p orchester-app --test agents
//! local_hub_review`.

use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::time::Timestamp;
use orchester_store::ProjectDb;

use super::{
  active::{pipeline, running},
  harness::Agents,
  local_hub::{open, server},
  support::project_id,
};

#[test]
fn local_hub_review_a_run_that_could_not_be_stopped_is_said_before_the_lists_read_the_archive() {
  let mut agents = Agents::new(true);
  let profile = agents.profile(true);
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let snapshot = pipeline(&mut db, profile);
  running(&mut db, snapshot, Timestamp::from_unix_millis(1_700_000_100_000));
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists("local-hub-more"));
  agents.click("local-hub-more");
  agents.click("local-hub-archive");
  agents.until("the confirmation", |s| s.exists("team-hub-archive-confirm"));
  agents.until("it says how many runs stop", |s| {
    s.text("team-hub-archive-confirm").contains("stop 1 run")
  });

  // No runner is attached in the test window, so the run cannot be stopped: the archive is written all the same.
  agents.press_until("team-hub-archive-confirm", "the notice", |s| {
    s.exists("team-hub-archive-left-running")
  });

  let said = agents.text("team-hub-archive-left-running");
  assert!(
    said.contains("1 of the runs") && said.contains("could not be stopped"),
    "{said}"
  );
  assert!(agents.text("team-hub-archive-title").contains("is archived"));
  assert!(
    agents.listed().profile(profile).is_none(),
    "it is archived: no list of the profiles holds it"
  );
  assert!(
    agents.exists(&format!("local-hub-row-{profile}")),
    "the list has not read the archive yet"
  );

  // Something else changes meanwhile and the lists read again: the profile is archived, so its row and its panel go,
  // and the notice stays, because the hub holds it and not the panel.
  agents
    .window
    .projects()
    .feed(project_id())
    .outside
    .update(|counts| counts.connections += 1);
  agents.until("the entry leaves the list", |s| {
    !s.exists(&format!("local-hub-row-{profile}")) && !s.exists("local-hub-more")
  });
  assert!(
    agents.exists("team-hub-archive-left-running"),
    "the notice outlives the panel it was asked from"
  );

  agents.click("team-hub-archive-close");
  agents.until("the notice closes", |s| !s.exists("team-hub-archive-left-running"));
}

#[test]
fn local_hub_review_an_archived_servers_name_stays_taken() {
  let mut agents = Agents::new(true);
  let docs = server(&agents, "docs");
  ProjectDb::open(&agents.root)
    .expect("database")
    .archive_mcp_server(docs, Timestamp::now())
    .expect("archived");
  agents.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::hub(HubView::adding(HubFilter::McpServers)),
  ));
  agents.until("the form", |s| s.exists("team-new-mcp-name"));

  agents.set("team-new-mcp-name", "docs");

  agents.until("the name is refused", |s| {
    s.text("team-new-mcp-name-row-sub").contains("already has this name")
  });
}
