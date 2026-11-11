//! Shares a project with a stand-in team, its owner being the person here: a server of this computer's, the project's
//! sharing link, and the team's configuration cached, so the app reads the project as shared without a server.

use std::{collections::HashMap, path::Path};

use orchester_core::{
  id::{ProjectId, ServerId},
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb, SharingLink, StoredServer};

use super::{Window, team_cache::keep_cached_team};

/// Shares the project at `repo`, open in `window`'s session, with a team as its owner.
pub fn share_as_owner(window: &Window, repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("database");
  let server = ServerId::generate();
  AppDb::open(&window.data.database())
    .expect("application database")
    .add_server(
      &StoredServer {
        id: server,
        url: "https://team.test".to_owned(),
        name: "Team".to_owned(),
        plain_http_allowed: false,
        certificate_pin: None,
        added_at: Timestamp::from_unix_millis(1),
        joined_with: None,
        account: None,
      },
      None,
    )
    .expect("server");
  let link = SharingLink {
    server,
    project: ProjectId::generate(),
    key: db.project().key().clone(),
    role: Some(MemberRole::Owner),
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("linked");
  keep_cached_team(window.launch.workspace().expect("the session"), repo);
}
