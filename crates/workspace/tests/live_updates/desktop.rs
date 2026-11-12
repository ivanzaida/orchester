//! A desktop signed in to the fake team server as one member, with the app's own [`Servers`] over its application
//! database and a keychain that holds the member's token, and one project linked to the server's project.

use std::{
  collections::HashMap,
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use orchester_api::SharedTask;
use orchester_core::{
  id::ServerId,
  project::ProjectKey,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{AppDb, ServerAccount, SharingLink, StoredServer};
use orchester_workspace::{
  secrets::Vault,
  sharing::{Servers, project::convert},
  workspace::Workspace,
};
use tempfile::TempDir;

use crate::{server::TeamServer, support::Fixture};

/// A keychain that holds the member's token under every key and notes the keys that were removed. It can be told to
/// refuse removals.
pub struct Keychain {
  token: String,
  pub removed: parking_lot::Mutex<Vec<VaultKey>>,
  refuse_deletes: AtomicBool,
}

impl Keychain {
  pub fn refuse_deletes(&self) {
    self.refuse_deletes.store(true, Ordering::SeqCst);
  }
}

impl SecretVault for Keychain {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Ok(())
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Ok(SecretValue::new(self.token.clone()).expect("a token"))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    if self.refuse_deletes.load(Ordering::SeqCst) {
      return Err(VaultError::AccessDenied("refused by the test".to_owned()));
    }
    self.removed.lock().push(key.clone());
    Ok(())
  }
}

pub struct Desktop {
  _data: TempDir,
  /// The project, with its tasks and runs.
  pub fixture: Fixture,
  pub servers: Servers,
  /// The workspace the servers hold, whose memory holds the team's configuration.
  pub workspace: Workspace,
  pub server: ServerId,
  pub keychain: Arc<Keychain>,
}

impl Desktop {
  /// The desktop of `token`'s member, whose project is linked to `team`'s and has `tasks` cached as the server has
  /// them; the project's own tasks and runs are made in [`Self::fixture`] before.
  pub fn signed_in(team: &TeamServer, token: &str, fixture: Fixture, tasks: &[SharedTask]) -> Self {
    Self::signed_in_as(team, (token, MemberRole::Member), fixture, tasks)
  }

  /// [`Self::signed_in`], the person's role in the project `role`: an owner changes the team's configuration.
  pub fn signed_in_as(
    team: &TeamServer,
    (token, role): (&str, MemberRole),
    fixture: Fixture,
    tasks: &[SharedTask],
  ) -> Self {
    let data = tempfile::tempdir().expect("application data");
    let mut app = AppDb::open(&data.path().join("orchester.db")).expect("the application database");
    let member = team.member(token);
    let stored = StoredServer {
      id: ServerId::generate(),
      url: team.url.clone(),
      name: "Kontur team".to_owned(),
      plain_http_allowed: true,
      certificate_pin: None,
      added_at: Timestamp::from_unix_millis(1),
      joined_with: None,
      account: Some(ServerAccount {
        user: member.id,
        username: "member".to_owned(),
        display_name: member.display_name,
        admin: false,
        signed_in_at: Timestamp::from_unix_millis(1),
      }),
    };
    app.add_server(&stored, None).expect("the server");
    let mut fixture = fixture;
    let link = SharingLink {
      server: stored.id,
      project: team.project,
      key: ProjectKey::new("ORC").expect("key"),
      role: Some(role),
      joined_at: Timestamp::from_unix_millis(1),
      cursor: 0,
      last_synced_at: Timestamp::from_unix_millis(1),
      removed_at: None,
      address: None,
    };
    let cached: Vec<_> = tasks
      .iter()
      .map(|task| convert::cached(task).expect("a task the cache takes"))
      .collect();
    fixture
      .db
      .start_sharing(&link, &[], (&cached, &HashMap::new()))
      .expect("linked");
    let workspace = Workspace::new(app);
    workspace.open(fixture.repo.path()).expect("the project");
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let keychain = Arc::new(Keychain {
      token: token.to_owned(),
      removed: parking_lot::Mutex::default(),
      refuse_deletes: AtomicBool::new(false),
    });
    let servers = Servers::new(workspace.clone(), Vault::with(keychain.clone()), version);
    Self {
      _data: data,
      fixture,
      servers,
      workspace,
      server: stored.id,
      keychain,
    }
  }

  pub fn root(&self) -> &Path {
    self.fixture.repo.path()
  }
}
