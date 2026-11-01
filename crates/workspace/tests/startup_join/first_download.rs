//! A linked checkout's first download from the stand-in team server, as the QA app's team scenarios reach it: a server
//! in the list, the checkout linked, then the join. Served, it fills the cache; held, it stays downloading; refused, it
//! says so. `cargo test -p orchester-workspace --test startup_join startup_join_first_download`

use std::{
  path::PathBuf,
  time::{Duration, Instant},
};

use orchester_api::{
  client::PlainHttp,
  test_support::{
    account_cloud::AccountCloud,
    team_server::{NAME, TasksAnswer, TeamServer},
  },
};
use orchester_exec::programs::Programs;
use orchester_workspace::{sharing::project::FirstDownload, tasks::data::ProjectTasks};

use super::{Desk, address, join_by_code};

/// A signed-in desktop with Kontur linked to the team's Kontur and the team joined, before any round.
struct Linked {
  desk: Desk,
  server: TeamServer,
  root: PathBuf,
  _cloud: AccountCloud,
}

impl Linked {
  fn new(tasks: TasksAnswer) -> Self {
    let server = TeamServer::start();
    server.share(&[("Kontur", "kontur", None), ("Billing", "billing", None)]);
    server.answer_tasks(tasks);
    let cloud = AccountCloud::start();
    let desk = Desk::signed_in(&cloud);
    let team = desk
      .run(
        desk
          .servers
          .add(&address(&server), NAME.to_owned(), PlainHttp::NotAllowed),
      )
      .expect("the server is in the list");
    let kontur = server.project("kontur").expect("the team shares Kontur");
    let root = desk.linked_project("repo-kontur", "Kontur", (team, kontur)).repo_root;
    join_by_code(&desk, &server);
    Self {
      desk,
      server,
      root,
      _cloud: cloud,
    }
  }

  fn state(&self) -> Option<FirstDownload> {
    self.desk.servers.download_state(&self.root)
  }

  fn downloading() -> Option<FirstDownload> {
    Some(FirstDownload::Downloading {
      server: NAME.to_owned(),
    })
  }

  fn round(&self) {
    self.desk.run(self.desk.servers.sync_shared(&Programs::processes()));
  }
}

#[test]
fn startup_join_first_download_fills_the_cache_when_served() {
  let linked = Linked::new(TasksAnswer::Serve);
  assert_eq!(
    linked.state(),
    Linked::downloading(),
    "the join marks the checkout linked"
  );

  linked.round();

  assert_eq!(linked.state(), None, "the download ended");
  let tasks = ProjectTasks::load(&linked.root, &Programs::processes()).expect("Kontur's tasks");
  assert_eq!(tasks.tasks.len(), 3);
  assert_eq!((tasks.tags.len(), tasks.modules.len()), (1, 1));
  assert!(tasks.shared);
  assert_eq!(linked.server.tasks_asked(), 1);
}

#[test]
fn startup_join_first_download_stays_downloading_while_held() {
  let linked = Linked::new(TasksAnswer::Hold);
  let servers = linked.desk.servers.clone();
  let round = linked
    .desk
    .runtime
    .spawn(async move { servers.sync_shared(&Programs::processes()).await });
  let deadline = Instant::now() + Duration::from_secs(20);
  while linked.server.tasks_asked() == 0 {
    assert!(Instant::now() < deadline, "the tasks are asked for");
    std::thread::sleep(Duration::from_millis(10));
  }

  assert_eq!(
    linked.state(),
    Linked::downloading(),
    "downloading while the answer is held"
  );

  linked.server.answer_tasks(TasksAnswer::Serve);
  linked.desk.run(round).expect("the round ends");
  assert_eq!(linked.state(), None);
}

#[test]
fn startup_join_first_download_says_the_server_refused() {
  let linked = Linked::new(TasksAnswer::Fail);

  linked.round();

  let Some(FirstDownload::Failed { server, reason }) = linked.state() else {
    panic!("the download failed: {:?}", linked.state());
  };
  assert_eq!(server, NAME);
  assert_eq!(reason.key, "servers.errors.refused");
  let tasks = ProjectTasks::load(&linked.root, &Programs::processes()).expect("Kontur's tasks");
  assert!(tasks.tasks.is_empty());
}
