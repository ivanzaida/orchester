//! Linking a team's shared project to an existing checkout from the start page (ORC-289): a chosen folder is checked
//! against the project's repository hint — the same repository in its https and ssh forms, another one, none, not a
//! repository, unreadable, linked to this project or to another — and linked only as the check allows, a different
//! `origin` only anyway. The server link itself reuses Join's and is proven against a real server elsewhere; here the
//! entry is no account's, so a link that gets past the check stops at the server.
//!
//! `cargo test -p orchester-workspace --test startup_link startup_link`

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
};

use orchester_api::{SharedProject, client::ServerAddress};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, ServerId},
  name::Name,
  project::{Project, ProjectKey},
  repository::RepositoryIdentity,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, SharingLink, StoredServer};
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    ServerEntry, Servers, SharingError,
    project::{
      ShareError,
      folder::{FolderCheck, FolderLinkError, FolderLinked, check_folder},
    },
  },
  workspace::Workspace,
};
use tempfile::TempDir;
use tokio::runtime::Runtime;

const SERVER: &str = "https://orchester.example.com";

fn git(root: &Path, args: &[&str]) {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

fn identity(text: &str) -> RepositoryIdentity {
  RepositoryIdentity::parse(text).expect("identity")
}

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// Folders under the repository's `.tmp/`, a server project named Kontur whose hint is `github.com/acme/kontur`, and
/// a workspace on its own application database.
struct Setup {
  scratch: TempDir,
  server: ServerId,
  project: SharedProject,
  workspace: Workspace,
  runtime: Runtime,
}

impl Setup {
  fn new() -> Self {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/startup-link-tests");
    fs::create_dir_all(&root).expect("scratch");
    let scratch = tempfile::Builder::new()
      .prefix("startup-link-")
      .tempdir_in(&root)
      .expect("scratch");
    let workspace = Workspace::new(AppDb::open(&scratch.path().join("orchester.db")).expect("database"));
    Self {
      scratch,
      server: ServerId::generate(),
      project: SharedProject {
        id: ProjectId::generate(),
        slug: "kontur".to_owned(),
        address: format!("{SERVER}/kontur"),
        key: "KON".to_owned(),
        name: "Kontur".to_owned(),
        remote_hint: Some(identity("github.com/acme/kontur")),
        role: Some(MemberRole::Member),
        next_number: 1,
      },
      workspace,
      runtime: Runtime::new().expect("runtime"),
    }
  }

  /// A folder `name` without anything in it.
  fn folder(&self, name: &str) -> PathBuf {
    let folder = self.scratch.path().join(name);
    fs::create_dir_all(&folder).expect("folder");
    folder
  }

  /// A git repository `name` whose `origin` is `origin`, or with no remote.
  fn repository(&self, name: &str, origin: Option<&str>) -> PathBuf {
    let folder = self.folder(name);
    git(&folder, &["init", "-b", "main"]);
    if let Some(origin) = origin {
      git(&folder, &["remote", "add", "origin", origin]);
    }
    folder
  }

  /// A repository whose project, named `name`, is linked to `project` on `server`.
  fn linked(&self, folder: &str, name: &str, (server, project): (ServerId, ProjectId)) -> PathBuf {
    let root = self.repository(folder, Some("https://github.com/acme/kontur.git"));
    let local = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new(name).expect("name"),
    );
    let mut db = ProjectDb::create(&root, local).expect("project");
    let link = SharingLink {
      server,
      project,
      key: ProjectKey::new("KON").expect("key"),
      role: Some(MemberRole::Member),
      joined_at: Timestamp::from_unix_millis(100),
      cursor: 1,
      last_synced_at: Timestamp::from_unix_millis(100),
      removed_at: None,
      address: Some(format!("{SERVER}/billing")),
    };
    db.start_sharing(&link, &[], (&[], &Default::default()))
      .expect("linked");
    root
  }

  fn check(&self, folder: &Path) -> FolderCheck {
    self.check_for(&self.project, folder)
  }

  fn check_for(&self, project: &SharedProject, folder: &Path) -> FolderCheck {
    check_folder(&Programs::processes(), (self.server, project), folder).expect("checked")
  }

  /// The server, as an entry no account of this computer is signed in to.
  fn entry(&self) -> ServerEntry {
    ServerEntry {
      stored: StoredServer {
        id: self.server,
        url: SERVER.to_owned(),
        name: "Acme".to_owned(),
        plain_http_allowed: false,
        certificate_pin: None,
        added_at: Timestamp::from_unix_millis(100),
        joined_with: None,
        account: None,
      },
      address: ServerAddress::parse(SERVER).expect("address"),
      connection: None,
      last_reached: None,
      certificate_changed: false,
    }
  }

  fn link(&self, folder: &Path, anyway: bool) -> Result<FolderLinked, FolderLinkError> {
    let servers = Servers::new(self.workspace.clone(), Vault::with(Arc::new(NoVault)));
    self.runtime.block_on(servers.link_folder(
      &self.entry(),
      &self.project,
      folder,
      &Programs::processes(),
      (anyway, IdempotencyKey::generate()),
    ))
  }

  fn registered(&self, folder: &Path) -> bool {
    self
      .workspace
      .projects()
      .expect("projects")
      .iter()
      .any(|project| project.repo_root.file_name() == folder.file_name())
  }
}

#[test]
fn startup_link_matches_an_https_origin() {
  let setup = Setup::new();
  let folder = setup.repository("https", Some("https://GitHub.com/acme/kontur.git"));
  assert_eq!(setup.check(&folder), FolderCheck::Matches);
  let message = setup.check(&folder).message(&setup.project);
  assert_eq!(message.key, "sharing.folder.matches");
  assert_eq!(message.args, vec![("hint", "github.com/acme/kontur".to_owned())]);
}

#[test]
fn startup_link_matches_an_ssh_origin() {
  let setup = Setup::new();
  let scp = setup.repository("scp", Some("git@github.com:acme/kontur.git"));
  assert_eq!(setup.check(&scp), FolderCheck::Matches);
  let ssh = setup.repository("ssh", Some("ssh://git@github.com/acme/Kontur"));
  assert_eq!(setup.check(&ssh), FolderCheck::Matches);
}

#[test]
fn startup_link_reports_a_different_origin_with_both() {
  let setup = Setup::new();
  let folder = setup.repository("billing", Some("https://github.com/acme/billing.git"));
  let check = setup.check(&folder);
  assert_eq!(
    check,
    FolderCheck::Differs {
      origin: identity("github.com/acme/billing"),
      hint: identity("github.com/acme/kontur"),
    }
  );
  let message = check.message(&setup.project);
  assert_eq!(message.key, "sharing.folder.differs");
  assert!(message.args.contains(&("origin", "github.com/acme/billing".to_owned())));
  assert!(message.args.contains(&("hint", "github.com/acme/kontur".to_owned())));
  assert!(message.args.contains(&("project", "Kontur".to_owned())));
}

#[test]
fn startup_link_reports_no_origin() {
  let setup = Setup::new();
  let none = setup.repository("none", None);
  assert_eq!(setup.check(&none), FolderCheck::NoOrigin);
  let local = setup.repository("local", Some("../none"));
  assert_eq!(setup.check(&local), FolderCheck::NoOrigin);
  let unnamed = SharedProject {
    remote_hint: None,
    ..setup.project.clone()
  };
  let any = setup.repository("any", Some("https://github.com/acme/billing.git"));
  assert_eq!(setup.check_for(&unnamed, &any), FolderCheck::NoHint);
}

#[test]
fn startup_link_reports_a_folder_that_is_not_a_repository() {
  let setup = Setup::new();
  let plain = setup.folder("plain");
  fs::write(plain.join("README.md"), "Kontur\n").expect("file");
  assert_eq!(setup.check(&plain), FolderCheck::NotRepository);
  assert_eq!(
    setup.check(&plain).message(&setup.project).key,
    "sharing.folder.notRepository"
  );
}

#[test]
fn startup_link_reports_an_unreadable_folder() {
  let setup = Setup::new();
  let missing = setup.scratch.path().join("missing");
  assert_eq!(setup.check(&missing), FolderCheck::Unreadable);
  let broken = setup.folder("broken");
  fs::write(broken.join(".git"), "not a gitdir line\n").expect("junk .git");
  assert_eq!(setup.check(&broken), FolderCheck::Unreadable);
}

#[test]
fn startup_link_refuses_a_different_origin_without_anyway() {
  let setup = Setup::new();
  let folder = setup.repository("billing", Some("https://github.com/acme/billing.git"));
  let refused = setup.link(&folder, false).expect_err("refused");
  assert!(
    matches!(refused, FolderLinkError::Refused(FolderCheck::Differs { .. })),
    "{refused:?}"
  );
  assert!(!folder.join(".orchester").exists(), "nothing was set up");
  assert!(!setup.registered(&folder));
}

#[test]
fn startup_link_anyway_goes_past_a_different_origin() {
  let setup = Setup::new();
  let folder = setup.repository("billing", Some("https://github.com/acme/billing.git"));
  let stopped = setup.link(&folder, true).expect_err("the entry is no account's");
  assert!(
    matches!(
      stopped,
      FolderLinkError::Share(ShareError::Sharing(SharingError::ServerMissing))
    ),
    "{stopped:?}"
  );
  assert!(folder.join(".orchester").exists(), "the folder was set up");
  assert!(setup.registered(&folder));
}

#[test]
fn startup_link_refuses_a_folder_linked_to_another_project() {
  let setup = Setup::new();
  let folder = setup.linked("billing", "Billing", (setup.server, ProjectId::generate()));
  let check = setup.check(&folder);
  assert_eq!(
    check,
    FolderCheck::LinkedElsewhere {
      name: Name::new("Billing").expect("name"),
      address: Some(format!("{SERVER}/billing")),
    }
  );
  let message = check.message(&setup.project);
  assert_eq!(message.key, "sharing.folder.linkedElsewhere");
  assert_eq!(message.args, vec![("name", "Billing".to_owned())]);
  let refused = setup.link(&folder, true).expect_err("refused");
  assert!(
    matches!(refused, FolderLinkError::Refused(FolderCheck::LinkedElsewhere { .. })),
    "{refused:?}"
  );
  assert!(!setup.registered(&folder));
}

#[test]
fn startup_link_opens_a_folder_linked_to_this_project() {
  let setup = Setup::new();
  let folder = setup.linked("kontur", "Kontur", (setup.server, setup.project.id));
  assert_eq!(setup.check(&folder), FolderCheck::LinkedHere);
  // The entry is no account's: any request to the server would fail.
  let linked = setup.link(&folder, false).expect("opened without the server");
  let FolderLinked::Open(open) = linked else {
    panic!("expected the project opened, got {linked:?}");
  };
  assert_eq!(open.project.name().as_str(), "Kontur");
  assert!(setup.registered(&folder));
}
