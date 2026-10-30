//! What a shared project's first download reports (ORC-291), against the paged responder: the tasks stored of how many
//! the server says, the latest to arrive and each part's state as the pages come, done or a typed failure at the end,
//! and a retry that continues at the page that failed. A checkout without tasks is linked before any page is read.
//!
//! `cargo test -p orchester-workspace --test shared_download startup_first_download`

use std::process::Command;

use orchester_api::{ErrorCode, client::ClientError};
use orchester_core::{id::IdempotencyKey, secret::VaultError, task::organization::WorkflowStatus};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::{
  sharing::{
    SharingError,
    project::{
      Arrival, DownloadFailure, DownloadProgress, DownloadState, FirstDownload, PartState, folder::FolderLinked,
    },
  },
  tasks::{data::ProjectTasks, detail::TaskDetail},
};

use crate::{
  Linked, SERVER,
  responder::{KEY, PageAnswer, Responder, SLUG, TITLES, TasksAnswer},
  until,
};

impl Linked {
  fn progress(&self) -> DownloadProgress {
    self
      .servers
      .download_progress(&self.root)
      .expect("a download was asked for")
  }

  fn latest(&self) -> Vec<String> {
    self.progress().latest.into_iter().map(|arrival| arrival.key).collect()
  }

  fn failure(&self) -> (DownloadFailure, &'static str) {
    match self.progress().state {
      DownloadState::Failed { failure, reason } => (failure, reason.key),
      state => panic!("the download failed: {state:?}"),
    }
  }

  /// A round of the shared projects' work, under way until it is awaited.
  fn round_under_way(&self) -> tokio::task::JoinHandle<()> {
    let servers = self.servers.clone();
    self
      .runtime
      .spawn(async move { servers.sync_shared(false, &Programs::processes()).await })
  }

  /// Reconnects the server, as the monitor's next check does once it answers.
  fn reconnect(&self) {
    let entries = self.servers.entries().expect("the servers");
    let entry = entries.first().expect("the server");
    self.runtime.block_on(self.servers.refresh_account(entry));
  }
}

#[test]
fn startup_first_download_counts_advance_with_the_latest_arrivals() {
  let linked = Linked::on(Responder::paged(6, 2, Some(6)));
  linked.responder.page(2, PageAnswer::Hold);
  linked.responder.page(4, PageAnswer::Hold);
  linked.join();
  assert_eq!(
    linked.progress(),
    DownloadProgress {
      server: SERVER.to_owned(),
      state: DownloadState::Downloading,
      total: None,
      stored: 0,
      latest: Vec::new(),
      tags: PartState::Pending,
      modules: PartState::Pending,
      team_settings: PartState::NotAvailable,
    },
    "nothing was read yet"
  );
  let mut changes = linked.servers.subscribe_shared();
  changes.borrow_and_update();
  let round = linked.round_under_way();
  until("the second page asked for", || linked.responder.pages_asked() == [0, 2]);
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Downloading);
  assert_eq!((progress.stored, progress.total), (2, Some(6)));
  assert_eq!(
    progress.latest,
    [("KON-2", "Task 2"), ("KON-1", "Task 1")].map(|(key, title)| Arrival {
      key: key.to_owned(),
      title: title.to_owned(),
      workflow: WorkflowStatus::Todo,
    }),
    "newest first"
  );
  assert_eq!((progress.tags, progress.modules), (PartState::Done, PartState::Done));
  assert_eq!(
    progress.team_settings,
    PartState::NotAvailable,
    "the sync does not read them"
  );
  assert_eq!(linked.tasks().tasks.len(), 2, "readable as they arrive");
  assert!(changes.has_changed().expect("open"), "the screens are told");
  changes.borrow_and_update();
  linked.responder.page(2, PageAnswer::Serve);
  until("the third page asked for", || {
    linked.responder.pages_asked() == [0, 2, 4]
  });
  assert!(changes.has_changed().expect("open"), "and of each page");
  assert_eq!(linked.progress().stored, 4);
  assert_eq!(linked.latest(), ["KON-4", "KON-3", "KON-2"], "only the latest three");
  assert_eq!(linked.tasks().tasks.len(), 4);
  linked.responder.page(4, PageAnswer::Serve);
  linked.runtime.block_on(round).expect("the round ends");
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!((progress.stored, progress.total), (6, Some(6)));
  assert_eq!(linked.latest(), ["KON-6", "KON-5", "KON-4"]);
  assert_eq!(linked.state(), None, "the Tasks list follows it as ended");
  assert_eq!(linked.tasks().tasks.len(), 6);
}

#[test]
fn startup_first_download_a_failure_midway_is_typed_and_a_retry_continues() {
  let linked = Linked::on(Responder::paged(6, 2, Some(6)));
  linked.responder.page(2, PageAnswer::Fail);
  linked.join();
  linked.round();
  assert_eq!(linked.failure(), (DownloadFailure::Refused, "servers.errors.refused"));
  assert_eq!(linked.progress().stored, 2, "what was stored stays");
  assert_eq!(linked.tasks().tasks.len(), 2);
  assert!(matches!(linked.state(), Some(FirstDownload::Failed { .. })));
  linked.responder.page(2, PageAnswer::Serve);
  linked.servers.retry_download(&linked.root);
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Downloading);
  assert_eq!(progress.stored, 2, "a retry keeps the counts");
  assert_eq!(progress.tags, PartState::Done, "and the parts");
  linked.round();
  assert_eq!(
    linked.responder.pages_asked(),
    [0, 2, 2, 4],
    "earlier pages are not read again"
  );
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!((progress.stored, progress.total), (6, Some(6)));
  assert_eq!(linked.tasks().tasks.len(), 6);
}

#[test]
fn startup_first_download_the_next_round_continues_a_failed_download() {
  let linked = Linked::on(Responder::paged(4, 2, Some(4)));
  linked.responder.page(2, PageAnswer::Fail);
  linked.join();
  linked.round();
  assert_eq!(linked.failure().0, DownloadFailure::Refused);
  linked.responder.page(2, PageAnswer::Serve);
  linked.round();
  assert_eq!(linked.responder.pages_asked(), [0, 2, 2]);
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!(progress.stored, 4);
}

#[test]
fn startup_first_download_a_broken_connection_is_offline_and_a_retry_continues() {
  let linked = Linked::on(Responder::paged(4, 2, Some(4)));
  linked.responder.page(2, PageAnswer::Drop);
  linked.join();
  linked.round();
  assert_eq!(
    linked.failure(),
    (DownloadFailure::Offline, "servers.check.unreachable")
  );
  assert_eq!(linked.progress().stored, 2);
  linked.responder.page(2, PageAnswer::Serve);
  linked.servers.retry_download(&linked.root);
  linked.round();
  assert_eq!(
    linked.failure(),
    (DownloadFailure::Offline, "sharing.errors.offline"),
    "nothing is asked of a server that is not back yet"
  );
  assert_eq!(linked.responder.pages_asked(), [0, 2]);
  linked.reconnect();
  linked.servers.retry_download(&linked.root);
  linked.round();
  assert_eq!(linked.responder.pages_asked(), [0, 2, 2]);
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!((progress.stored, progress.total), (4, Some(4)));
}

#[test]
fn startup_first_download_a_server_without_pages_is_one_page() {
  let linked = Linked::new(TasksAnswer::Hold);
  linked.join();
  let round = linked.round_under_way();
  until("the tasks asked for", || linked.responder.tasks_asked() == 1);
  let progress = linked.progress();
  assert_eq!((progress.stored, progress.total), (0, None), "the server says no total");
  assert_eq!(progress.tags, PartState::Done);
  linked.responder.tasks(TasksAnswer::Serve);
  linked.runtime.block_on(round).expect("the round ends");
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!((progress.stored, progress.total), (TITLES.len(), Some(TITLES.len())));
  assert_eq!(
    linked.latest(),
    [format!("{KEY}-3"), format!("{KEY}-2"), format!("{KEY}-1")]
  );
}

#[test]
fn startup_first_download_tasks_a_server_repeats_are_not_counted_again() {
  let linked = Linked::on(Responder::paged(6, 2, None));
  linked.responder.page(2, PageAnswer::Fail);
  linked.join();
  linked.round();
  assert_eq!(linked.progress().stored, 2);
  linked.responder.page(2, PageAnswer::Whole);
  linked.servers.retry_download(&linked.root);
  linked.round();
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!((progress.stored, progress.total), (6, Some(6)));
  assert_eq!(linked.latest(), ["KON-6", "KON-5", "KON-4"]);
}

#[test]
fn startup_first_download_a_page_that_does_not_move_on_fails() {
  for answer in [PageAnswer::Stuck, PageAnswer::Empty, PageAnswer::Repeats] {
    let linked = Linked::on(Responder::paged(6, 2, Some(6)));
    linked.responder.page(2, answer);
    linked.join();
    linked.round();
    assert_eq!(
      linked.failure(),
      (DownloadFailure::InvalidAnswer, "sharing.errors.invalidAnswer"),
      "{answer:?}"
    );
    assert_eq!(linked.responder.pages_asked(), [0, 2], "the round ended: {answer:?}");
    assert_eq!(
      linked.progress().stored,
      2,
      "the refused page is not stored: {answer:?}"
    );
    assert_eq!(linked.tasks().tasks.len(), 2, "{answer:?}");
  }
}

#[test]
fn startup_first_download_a_task_opens_before_its_blocker_arrives() {
  let linked = Linked::on(Responder::paged(4, 2, Some(4)));
  linked.responder.blocked_by(1, 3);
  linked.responder.page(2, PageAnswer::Hold);
  linked.join();
  let round = linked.round_under_way();
  until("the second page asked for", || linked.responder.pages_asked() == [0, 2]);
  let blocked = linked
    .tasks()
    .tasks
    .iter()
    .find(|task| task.key().to_string() == "KON-1")
    .expect("the first page arrived")
    .id();
  let detail = TaskDetail::load_in(&linked.root, blocked).expect("the task page opens");
  assert_eq!(
    detail.blocked_by,
    Vec::<String>::new(),
    "its blocker has no key here yet"
  );
  let db = ProjectDb::open(&linked.root).expect("Kontur");
  assert!(db.check_task_readiness(blocked).is_err(), "it cannot run all the same");
  linked.responder.page(2, PageAnswer::Serve);
  linked.runtime.block_on(round).expect("the round ends");
  let detail = TaskDetail::load_in(&linked.root, blocked).expect("the task page opens");
  assert_eq!(detail.blocked_by, ["KON-3"], "its blocker arrived");
}

#[test]
fn startup_first_download_the_latest_arrivals_do_not_follow_the_order_of_the_answer() {
  let linked = Linked::on(Responder::paged(5, 2, None));
  linked.responder.page(0, PageAnswer::NewestFirst);
  linked.join();
  linked.round();
  let progress = linked.progress();
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!((progress.stored, progress.total), (5, Some(5)));
  assert_eq!(linked.latest(), ["KON-5", "KON-4", "KON-3"]);
}

#[test]
fn startup_first_download_a_round_with_no_download_tells_the_screens_once() {
  let linked = Linked::on(Responder::paged(6, 2, Some(6)));
  linked.join();
  linked.round();
  assert_eq!(linked.progress().state, DownloadState::Done);
  let changes = linked.servers.subscribe_shared();
  let before = *changes.borrow();
  linked.round();
  assert_eq!(linked.responder.pages_asked(), [0, 2, 4, 0, 2, 4], "it read every page");
  assert_eq!(*changes.borrow() - before, 1, "at its end, not for each page");
}

#[test]
fn startup_first_download_a_server_gone_from_the_list_fails_typed() {
  let linked = Linked::on(Responder::paged(2, 2, Some(2)));
  linked.join();
  linked.server_leaves_the_list();
  linked.round();
  assert_eq!(
    linked.failure(),
    (DownloadFailure::ServerMissing, "sharing.errors.serverMissing")
  );
}

#[test]
fn startup_first_download_every_error_has_its_failure() {
  let refused = |code| {
    SharingError::Client(ClientError::Api {
      status: 400,
      code,
      retry_after: None,
      details: None,
    })
  };
  let errors = [
    (SharingError::Offline, DownloadFailure::Offline),
    (SharingError::Client(ClientError::TimedOut), DownloadFailure::Offline),
    (SharingError::Client(ClientError::Unreachable), DownloadFailure::Offline),
    (SharingError::SignedOut, DownloadFailure::SignedOut),
    (refused(ErrorCode::Unauthorized), DownloadFailure::SignedOut),
    (SharingError::NotMember, DownloadFailure::NotMember),
    (refused(ErrorCode::NotMember), DownloadFailure::NotMember),
    (SharingError::ServerMissing, DownloadFailure::ServerMissing),
    (refused(ErrorCode::Forbidden), DownloadFailure::Refused),
    (SharingError::KeptChanging, DownloadFailure::Refused),
    (SharingError::InvalidAnswer("task page"), DownloadFailure::InvalidAnswer),
    (
      SharingError::Client(ClientError::NotOrchester { status: 200 }),
      DownloadFailure::InvalidAnswer,
    ),
    (
      SharingError::Client(ClientError::Redirect { location: None }),
      DownloadFailure::InvalidAnswer,
    ),
    (SharingError::CertificateChanged, DownloadFailure::Certificate),
    (
      SharingError::Client(ClientError::UntrustedCertificate),
      DownloadFailure::Certificate,
    ),
    (SharingError::Store(StoreError::NotShared), DownloadFailure::Local),
    (SharingError::Vault(VaultError::NotFound), DownloadFailure::Local),
    (SharingError::Interrupted, DownloadFailure::Local),
    (SharingError::Client(ClientError::Setup), DownloadFailure::Local),
  ];
  for (error, failure) in errors {
    assert_eq!(DownloadFailure::from(&error), failure, "{error:?}");
  }
}

#[test]
fn startup_first_download_a_project_gone_from_the_server_fails() {
  let linked = Linked::on(Responder::paged(2, 2, Some(2)));
  linked.responder.project_gone();
  linked.join();
  linked.round();
  assert_eq!(
    linked.failure(),
    (DownloadFailure::NotMember, "sharing.errors.notMember")
  );
  assert_eq!(linked.responder.pages_asked(), Vec::<u32>::new());
  assert!(linked.tasks().tasks.is_empty());
}

#[test]
fn startup_first_download_a_later_round_reads_from_the_start_and_stays_done() {
  let linked = Linked::on(Responder::paged(4, 2, Some(4)));
  linked.join();
  linked.round();
  let done = linked.progress();
  assert_eq!(done.state, DownloadState::Done);
  linked.round();
  assert_eq!(linked.responder.pages_asked(), [0, 2, 0, 2]);
  assert_eq!(linked.progress(), done, "the counts do not change");
  assert_eq!(linked.state(), None);
}

#[test]
fn startup_first_download_linking_a_folder_opens_before_the_tasks() {
  let linked = Linked::on(Responder::paged(3, 2, Some(3)));
  linked.responder.page(0, PageAnswer::Hold);
  linked.join();
  let folder = linked.root.with_file_name("repo-picked");
  std::fs::create_dir_all(&folder).expect("a folder");
  let git = Command::new("git")
    .args(["init", "-b", "main"])
    .current_dir(&folder)
    .output()
    .expect("git");
  assert!(git.status.success(), "{}", String::from_utf8_lossy(&git.stderr));
  let entries = linked.servers.entries().expect("the servers");
  let entry = entries.first().expect("the server");
  let opened = linked.runtime.block_on(async {
    let project = linked.servers.find_project(entry, SLUG).await.expect("the project");
    linked
      .servers
      .link_folder(
        entry,
        &project,
        &folder,
        &Programs::processes(),
        (false, IdempotencyKey::generate()),
      )
      .await
      .expect("linked")
  });
  assert!(matches!(opened, FolderLinked::Open(_)), "{opened:?}");
  assert_eq!(linked.responder.pages_asked(), Vec::<u32>::new(), "no page was read");
  let cached = || {
    ProjectTasks::load(&folder, &Programs::processes())
      .expect("read")
      .tasks
      .len()
  };
  let progress = linked
    .servers
    .download_progress(&folder)
    .expect("the link starts a download");
  assert_eq!(progress.state, DownloadState::Downloading);
  assert_eq!((progress.stored, progress.total), (0, None));
  assert_eq!(cached(), 0, "nothing is cached yet");
  linked.responder.page(0, PageAnswer::Serve);
  linked.round();
  let progress = linked.servers.download_progress(&folder).expect("the download");
  assert_eq!(progress.state, DownloadState::Done);
  assert_eq!((progress.stored, progress.total), (3, Some(3)));
  assert_eq!(cached(), 3);
}
