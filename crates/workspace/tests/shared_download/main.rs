//! A shared project's first download after a join (ORC-282), against the test-only loopback responder ([`responder`]):
//! joining a team server with a code marks the checkout linked to a project there as downloading, the next round of
//! the shared projects' work reads the project's tags, modules and tasks into its cache, and the screens' own reading
//! of the project (`ProjectTasks::load`) has them without the workspace being opened again. A download that fails
//! says why, and *Try again* downloads again; a checkout the round cannot reach at all fails rather than staying
//! downloading. What the download reports as it runs is in [`first_download`].
//!
//! `cargo test -p orchester-workspace --test shared_download shared_download`

mod first_download;
mod responder;

use std::{
  collections::HashMap,
  fs,
  path::PathBuf,
  sync::Arc,
  time::{Duration, Instant},
};

use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
};
use orchester_core::{
  id::{ProjectId, ServerId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, SharingLink};
use orchester_workspace::{
  secrets::Vault,
  sharing::{CodeRequest, Servers, Trust, project::FirstDownload},
  tasks::data::ProjectTasks,
  workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use self::responder::{KEY, MODULE, NAME, Responder, TAG, TITLES, TasksAnswer};

/// The name the responder's server gives itself.
const SERVER: &str = "Kontur team";

/// A keychain in memory.
#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// A checkout of Kontur without tasks, linked to the responder's project with an empty cache, on a server in the list
/// that this computer is not signed in to yet: as after signing out, or on a computer whose checkout was linked before.
struct Linked {
  runtime: Runtime,
  servers: Servers,
  server: ServerId,
  root: PathBuf,
  database: PathBuf,
  responder: Responder,
  _dir: TempDir,
}

impl Linked {
  fn new(tasks: TasksAnswer) -> Self {
    Self::on(Responder::start(tasks))
  }

  fn on(responder: Responder) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-282-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("shared-download-")
      .tempdir_in(&scratch)
      .expect("data");
    let root = dir.path().join("repo-kontur");
    fs::create_dir_all(root.join(".git")).expect("a checkout");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new(KEY).expect("key"),
      Name::new(NAME).expect("name"),
    );
    ProjectDb::create(&root, project).expect("Kontur");
    let database = dir.path().join("orchester.db");
    let workspace = Workspace::new(AppDb::open(&database).expect("database"));
    workspace.open(&root).expect("register Kontur");
    let servers = Servers::new(workspace, Vault::with(Arc::new(MemoryVault::default())));
    let runtime = Runtime::new().expect("runtime");
    let address = ServerAddress::parse(responder.url()).expect("address");
    let server = runtime
      .block_on(servers.add(&address, SERVER.to_owned(), PlainHttp::NotAllowed))
      .expect("the server is in the list");
    let link = SharingLink {
      server,
      project: Responder::project(),
      key: ProjectKey::new(KEY).expect("key"),
      role: Some(MemberRole::Owner),
      joined_at: Timestamp::now(),
      cursor: 0,
      last_synced_at: Timestamp::now(),
      removed_at: None,
      address: Some(responder.address()),
    };
    ProjectDb::open(&root)
      .expect("Kontur")
      .start_sharing(&link, &[], (&[], &HashMap::new()))
      .expect("linked with an empty cache");
    Self {
      runtime,
      servers,
      server,
      root,
      database,
      responder,
      _dir: dir,
    }
  }

  /// Joins the server with a code, as Settings › Orchester does.
  fn join(&self) {
    let address = ServerAddress::parse(self.responder.url()).expect("address");
    let request = CodeRequest {
      code: Credential::new("ABCD-EFGH".to_owned()),
      username: None,
      display_name: None,
      proof: None,
    };
    let joined = self.runtime.block_on(self.servers.join_with_code(
      &address,
      Trust::System(PlainHttp::NotAllowed),
      request,
    ));
    assert_eq!(
      joined.expect("the code joins"),
      self.server,
      "the server in the list is joined"
    );
  }

  /// The server goes from the list, as no screen does: only a damaged list loses a server its checkouts link to.
  fn server_leaves_the_list(&self) {
    let removed = rusqlite::Connection::open(&self.database)
      .expect("database")
      .execute("DELETE FROM servers WHERE id = ?1", [self.server.to_string()])
      .expect("removed");
    assert_eq!(removed, 1, "the server left the list");
  }

  /// One round of the shared projects' work, as the request the join made starts.
  fn round(&self) {
    self.runtime.block_on(self.servers.sync_shared(&Programs::processes()));
  }

  fn state(&self) -> Option<FirstDownload> {
    self.servers.download_state(&self.root)
  }

  fn tasks(&self) -> ProjectTasks {
    ProjectTasks::load(&self.root, &Programs::processes()).expect("Kontur's tasks read")
  }
}

fn downloading() -> Option<FirstDownload> {
  Some(FirstDownload::Downloading {
    server: SERVER.to_owned(),
  })
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
  let deadline = Instant::now() + Duration::from_secs(20);
  while !done() {
    assert!(Instant::now() < deadline, "timed out waiting for {what}");
    std::thread::sleep(Duration::from_millis(10));
  }
}

/// `tasks`' titles, each with its tags' and its module's names.
fn described(tasks: &ProjectTasks) -> Vec<(String, Vec<String>, Option<String>)> {
  let mut described: Vec<_> = tasks
    .tasks
    .iter()
    .map(|task| {
      let tags = tasks
        .tags
        .iter()
        .filter(|tag| task.organization().tags().contains(&tag.id()))
        .map(|tag| tag.name().as_str().to_owned())
        .collect();
      let module = tasks
        .modules
        .iter()
        .find(|module| Some(module.id()) == task.module())
        .map(|module| module.name().as_str().to_owned());
      (task.details().title.as_str().to_owned(), tags, module)
    })
    .collect();
  described.sort();
  described
}

fn expected() -> Vec<(String, Vec<String>, Option<String>)> {
  let mut expected: Vec<_> = TITLES
    .iter()
    .map(|title| ((*title).to_owned(), vec![TAG.to_owned()], Some(MODULE.to_owned())))
    .collect();
  expected.sort();
  expected
}

#[test]
fn shared_download_a_join_reads_the_linked_checkouts_tasks_tags_and_modules_as_the_screens_read_them() {
  let linked = Linked::new(TasksAnswer::Hold);
  let mut changes = linked.servers.subscribe_shared();
  changes.borrow_and_update();
  assert_eq!(linked.state(), None, "nothing is downloading before the join");
  linked.join();
  assert_eq!(linked.state(), downloading(), "the join starts the download");
  assert!(changes.has_changed().expect("open"), "the screens are told");
  let servers = linked.servers.clone();
  let round = linked
    .runtime
    .spawn(async move { servers.sync_shared(&Programs::processes()).await });
  until("the tasks asked for", || linked.responder.tasks_asked() == 1);
  assert_eq!(linked.state(), downloading(), "downloading while the server answers");
  assert!(linked.tasks().tasks.is_empty(), "nothing is cached yet");
  linked.responder.tasks(TasksAnswer::Serve);
  linked.runtime.block_on(round).expect("the round ends");
  assert_eq!(linked.state(), None, "the download ended");
  let tasks = linked.tasks();
  assert_eq!(described(&tasks), expected(), "every task with its tag and its module");
  assert!(tasks.shared, "read as a shared project");
}

#[test]
fn shared_download_a_failed_download_says_why_and_try_again_downloads() {
  let linked = Linked::new(TasksAnswer::Fail);
  linked.join();
  linked.round();
  let Some(FirstDownload::Failed { server, reason }) = linked.state() else {
    panic!("the download failed: {:?}", linked.state());
  };
  assert_eq!(server, SERVER);
  assert_eq!(reason.key, "servers.errors.refused", "the server's refusal");
  assert!(linked.tasks().tasks.is_empty());
  linked.responder.tasks(TasksAnswer::Serve);
  linked.servers.retry_download(&linked.root);
  assert_eq!(linked.state(), downloading(), "Try again downloads again");
  linked.round();
  assert_eq!(linked.state(), None);
  assert_eq!(described(&linked.tasks()), expected());
}

#[test]
fn shared_download_a_checkout_whose_server_left_the_list_fails_instead_of_staying_downloading() {
  let linked = Linked::new(TasksAnswer::Serve);
  linked.join();
  assert_eq!(linked.state(), downloading());
  linked.server_leaves_the_list();
  linked.round();
  let Some(FirstDownload::Failed { reason, .. }) = linked.state() else {
    panic!("the download failed: {:?}", linked.state());
  };
  assert_eq!(reason.key, "sharing.errors.serverMissing");
  assert_eq!(linked.responder.tasks_asked(), 0, "nothing was read");
  linked.servers.retry_download(&linked.root);
  linked.round();
  assert!(
    matches!(linked.state(), Some(FirstDownload::Failed { .. })),
    "Try again fails again while the server is gone"
  );
}

#[test]
fn shared_download_try_again_changes_nothing_for_a_checkout_that_is_not_failing() {
  let linked = Linked::new(TasksAnswer::Serve);
  linked.servers.retry_download(&linked.root);
  assert_eq!(linked.state(), None);
  linked.join();
  linked.servers.retry_download(&linked.root);
  assert_eq!(linked.state(), downloading(), "a download under way stays as it is");
}
