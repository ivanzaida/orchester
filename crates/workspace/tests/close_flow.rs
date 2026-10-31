//! The close flow of a window without a runner at work: a close request stops the runners at once and lets
//! the window close, ending the session clean; Wait for agents to save, Quit now and Cancel change nothing
//! unless the flow holds the close for the person. Closes held while agents work are covered with the real
//! runner by `orchester-app`'s `tests/lifecycle.rs`. What the panel lists says where the close stands: the
//! window closes once every run is saved and git is done.

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::{Duration, Instant},
};

use orchester_core::{connection::AgentProvider, id::RunId};
use orchester_ipc::SessionEnd as AttachEnd;
use orchester_store::SessionEnd;
use orchester_workspace::{
  activity::Activity,
  close::{Board, CloseFlow, ClosePhase, RunStands, Stage},
  messages::Message,
  runner::{ActiveWork, ActiveWorkFields, Runners},
  tasks::integration_jobs::{BusyJob, ProjectJobs},
};

fn flow(runtime: &tokio::runtime::Runtime) -> CloseFlow {
  let runners = Runners::new(
    PathBuf::from("no-runner"),
    runtime.handle().clone(),
    Activity::new(),
    AttachEnd::Clean,
  );
  CloseFlow::new(runners, ProjectJobs::default(), runtime.handle().clone())
}

#[test]
fn close_flow_without_work_stops_quietly_and_lets_the_window_close_once() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let close = flow(&runtime);
  let let_close = Arc::new(AtomicUsize::new(0));
  for _ in 0..2 {
    let counted = let_close.clone();
    close.request(move || {
      counted.fetch_add(1, Ordering::AcqRel);
    });
  }
  let deadline = Instant::now() + Duration::from_secs(10);
  while close.session_end().is_none() {
    assert!(Instant::now() < deadline, "the window is let close");
    std::thread::sleep(Duration::from_millis(10));
  }
  assert_eq!(close.phase(), ClosePhase::Stopping);
  assert_eq!(close.session_end(), Some(SessionEnd::Clean));
  assert_eq!(let_close.load(Ordering::Acquire), 1, "a second request is dropped");
}

/// ORC-414: once the update handed over to its installer, which stopped the runners at checkpoints itself, the next
/// close request lets the window close without asking, and the session ends as the update said.
#[test]
fn update_install_close_flow_after_a_handover_closes_without_asking() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let close = flow(&runtime);
  close.hand_over(SessionEnd::RunsPaused);
  let let_close = Arc::new(AtomicUsize::new(0));
  let counted = let_close.clone();
  close.request(move || {
    counted.fetch_add(1, Ordering::AcqRel);
  });
  let deadline = Instant::now() + Duration::from_secs(10);
  while close.session_end().is_none() {
    assert!(Instant::now() < deadline, "the window is let close");
    std::thread::sleep(Duration::from_millis(10));
  }
  assert_eq!(close.phase(), ClosePhase::Stopping);
  assert_eq!(close.session_end(), Some(SessionEnd::RunsPaused));
  assert_eq!(let_close.load(Ordering::Acquire), 1);
}

#[test]
fn close_flow_choices_do_nothing_while_no_close_is_held() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let close = flow(&runtime);
  close.wait_for_agents();
  close.quit_now();
  close.cancel();
  assert_eq!(close.phase(), ClosePhase::Open);
  assert_eq!(close.session_end(), None);
}

fn work(task_key: &str) -> ActiveWork {
  ActiveWork::new(&ActiveWorkFields {
    project: "Kontur",
    run: RunId::generate(),
    task_key,
    task_title: "Typed errors per module",
    step: "Implement",
    agent: "Builder",
    provider: AgentProvider::CodexCli,
    model: "gpt-5.5",
  })
}

fn merge() -> BusyJob {
  BusyJob {
    project: "Kontur".into(),
    what: Message::new("statusBar.operations.runMerge.running")
      .with("task", "KON-3".to_owned())
      .with("run", "4".to_owned())
      .with("branch", "main".to_owned()),
    merge: true,
  }
}

#[test]
fn close_flow_board_closes_only_once_every_run_is_saved_and_git_is_done() {
  let saved = || RunStands::Saved { checkpoint: None };
  let confirming = Board::new(vec![(work("KON-1"), RunStands::Running)], Vec::new(), false);
  assert_eq!((confirming.stage(), confirming.left()), (Stage::Confirm, 1));
  let saving = Board::new(
    vec![(work("KON-1"), RunStands::Saving), (work("KON-2"), saved())],
    Vec::new(),
    true,
  );
  assert_eq!((saving.stage(), saving.left()), (Stage::Waiting, 1));
  let merging = Board::new(vec![(work("KON-1"), saved())], vec![merge()], true);
  assert_eq!(
    (merging.stage(), merging.left()),
    (Stage::Waiting, 1),
    "git still works"
  );
  let done = Board::new(
    vec![(work("KON-1"), saved()), (work("KON-2"), saved())],
    Vec::new(),
    true,
  );
  assert_eq!((done.stage(), done.left()), (Stage::Saved, 0));
}
