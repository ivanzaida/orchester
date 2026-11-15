//! Project settings has no Connections tab and no page of an agent connection (`Agents on accounts` D1 `E6kvpm`;
//! `docs/design/agent-accounts.md`, "Agents run on accounts", decision 7): the tab's old address opens General, an
//! agent connection's old address says where connections went, and GitHub's page opens from General's *GitHub
//! account* row, with its crumb back to General.

use orchester_app::routes::{ConnectionsView, ProjectPage};
use orchester_core::{connection::ConnectionKind, store::ConnectionStore};

use super::harness::Seeded;

#[test]
fn connections_gone_the_tabs_address_opens_general() {
  let mut seeded = Seeded::new("connections");
  seeded.go(ProjectPage::Connections(ConnectionsView::default()));
  seeded.until("Project settings on General", |seeded| {
    seeded.exists("settings-tab-general") && !seeded.exists("connections-moved")
  });
  assert!(!seeded.exists("connection-inspector"));
  assert!(
    !seeded.exists("settings-tab-connections"),
    "Connections is no tab of Project settings"
  );
  assert!(
    !seeded.exists("connections-add-agent"),
    "no list of connections, and nothing adds an agent connection"
  );
}

#[test]
fn connections_gone_an_agent_connections_address_says_where_they_went() {
  let mut seeded = Seeded::new("connections");
  let agent = seeded
    .db()
    .connections()
    .expect("connections")
    .into_iter()
    .find(|connection| matches!(connection.kind(), ConnectionKind::Agent(_)))
    .expect("the seed has an agent connection")
    .id();
  seeded.go(ProjectPage::Connections(ConnectionsView::of(agent)));
  seeded.until("what became of the page", |seeded| seeded.exists("connections-moved"));
  let page = seeded.text("connections-moved");
  assert!(page.contains("Settings › Accounts"), "{page}");
  assert!(
    !seeded.exists("connection-inspector"),
    "an agent connection has no page"
  );
  assert!(seeded.exists("connections-moved-accounts") && seeded.exists("connections-moved-general"));
}

#[test]
fn connections_gone_githubs_page_opens_from_generals_account_row() {
  let mut seeded = Seeded::new("connections");
  let github = seeded
    .db()
    .connections()
    .expect("connections")
    .into_iter()
    .find(|connection| matches!(connection.kind(), ConnectionKind::GitHub { .. }))
    .expect("the seed's GitHub connection")
    .id();
  seeded.go(ProjectPage::Connections(ConnectionsView::of(github)));
  seeded.until("GitHub's page", |seeded| seeded.exists("connection-inspector"));
  assert_eq!(seeded.selected(), Some(github));
  assert_eq!(
    seeded.text("connection-crumb-back"),
    "General",
    "GitHub's page is General's"
  );
}
