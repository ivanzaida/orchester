//! The close flow of a window without a runner at work: a close request stops the runners at once and lets
//! the window close, ending the session clean; Wait for agents to save, Quit now and Cancel change nothing
//! unless the flow holds the close for the person. Closes held while agents work are covered with the real
//! runner by `orchester-app`'s `tests/lifecycle.rs`.

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::{Duration, Instant},
};

use orchester_ipc::SessionEnd as AttachEnd;
use orchester_store::SessionEnd;
use orchester_workspace::{
  activity::Activity,
  close::{CloseFlow, ClosePhase},
  runner::Runners,
};

fn flow(runtime: &tokio::runtime::Runtime) -> CloseFlow {
  let runners = Runners::new(
    PathBuf::from("no-runner"),
    runtime.handle().clone(),
    Activity::new(),
    AttachEnd::Clean,
  );
  CloseFlow::new(runners, runtime.handle().clone())
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
