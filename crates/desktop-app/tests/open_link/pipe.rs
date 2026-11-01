//! Handing a link to the app that holds the data folder, over the folder's pipe: it arrives in that app's inbox as the
//! holder reads it; a pipe served by another process, a holder that does not answer in 2 s, no holder, or no pipe at
//! all leaves the link with the program that started, for its *already open* screen.

use std::{
  path::Path,
  time::{Duration, Instant},
};

use orchester_app::open_link::{
  LinkInbox, MAX_LINE, OpenLink,
  pipe::{self, ANSWER_WAIT, HandOverError},
};
use orchester_core::time::Timestamp;
use orchester_store::location::LockHolder;
use tempfile::TempDir;
use tokio::{net::windows::named_pipe::ServerOptions, runtime::Runtime};

use super::INVITE;

/// A data folder of its own in the worktree's scratch folder.
fn folder() -> TempDir {
  let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/open-link-tests");
  std::fs::create_dir_all(&scratch).expect("scratch folder");
  tempfile::tempdir_in(scratch).expect("data folder")
}

/// This test's process as the app that holds the data folder.
fn this_process() -> Option<LockHolder> {
  Some(LockHolder::running(std::process::id(), Timestamp::now()))
}

/// Waits until `inbox` has links, and takes them.
fn arrived(inbox: &LinkInbox) -> Vec<OpenLink> {
  let deadline = Instant::now() + Duration::from_secs(5);
  loop {
    let links = inbox.take();
    if !links.is_empty() || Instant::now() > deadline {
      return links;
    }
    std::thread::sleep(Duration::from_millis(10));
  }
}

#[test]
fn open_link_pipe_hands_a_link_to_the_holder() {
  let (runtime, data) = (Runtime::new().expect("runtime"), folder());
  let inbox = LinkInbox::default();
  let _listening = pipe::listen(runtime.handle(), data.path(), inbox.clone()).expect("the pipe is made");
  let link = format!("orchester://invite/{INVITE}");
  pipe::hand_over(runtime.handle(), data.path(), this_process(), &link).expect("the holder takes it");
  assert_eq!(arrived(&inbox), [OpenLink::Invite(INVITE.parse().expect("an id"))]);
  // The holder keeps taking links, one after another.
  pipe::hand_over(runtime.handle(), data.path(), this_process(), "orchester://open").expect("a second link");
  assert_eq!(arrived(&inbox), [OpenLink::Open]);
}

#[test]
fn open_link_pipe_carries_what_is_no_link_as_unknown() {
  let (runtime, data) = (Runtime::new().expect("runtime"), folder());
  let inbox = LinkInbox::default();
  let _listening = pipe::listen(runtime.handle(), data.path(), inbox.clone()).expect("the pipe is made");
  let too_long = format!("orchester://invite/{INVITE}?{}", "x".repeat(MAX_LINE));
  let two_lines = format!("orchester://open\norchester://invite/{INVITE}");
  for link in ["", "orchester://settings", too_long.as_str(), two_lines.as_str()] {
    pipe::hand_over(runtime.handle(), data.path(), this_process(), link).expect("the holder takes it");
    assert_eq!(arrived(&inbox), [OpenLink::Unknown], "{link:?}");
  }
}

#[test]
fn open_link_pipe_is_one_per_data_folder() {
  let (runtime, data) = (Runtime::new().expect("runtime"), folder());
  let _listening = pipe::listen(runtime.handle(), data.path(), LinkInbox::default()).expect("the pipe is made");
  assert!(
    pipe::listen(runtime.handle(), data.path(), LinkInbox::default()).is_err(),
    "a second app never serves a pipe that another serves"
  );
  let other = folder();
  assert_ne!(pipe::name(data.path()), pipe::name(other.path()));
  let spelled = data.path().join(".").join("");
  assert_eq!(
    pipe::name(data.path()),
    pipe::name(&spelled),
    "two spellings of one folder"
  );
}

#[test]
fn open_link_pipe_without_a_holder_hands_nothing_over() {
  let (runtime, data) = (Runtime::new().expect("runtime"), folder());
  let inbox = LinkInbox::default();
  let _listening = pipe::listen(runtime.handle(), data.path(), inbox.clone()).expect("the pipe is made");
  let refused = pipe::hand_over(runtime.handle(), data.path(), None, "orchester://open");
  assert!(matches!(refused, Err(HandOverError::NoHolder)), "{refused:?}");
  let other = Some(LockHolder::running(std::process::id() + 1, Timestamp::now()));
  let refused = pipe::hand_over(runtime.handle(), data.path(), other, "orchester://open");
  assert!(
    matches!(refused, Err(HandOverError::NotTheHolder { .. })),
    "a pipe another process serves gets no link: {refused:?}"
  );
  assert!(inbox.take().is_empty());
  let empty = folder();
  let refused = pipe::hand_over(runtime.handle(), empty.path(), this_process(), "orchester://open");
  assert!(matches!(refused, Err(HandOverError::Pipe(_))), "no pipe: {refused:?}");
}

#[test]
fn open_link_pipe_gives_up_on_a_holder_that_does_not_answer() {
  let (runtime, data) = (Runtime::new().expect("runtime"), folder());
  let name = pipe::name(data.path());
  // A holder that takes the connection and never reads it, as an older build would.
  let silent = {
    let _entered = runtime.enter();
    ServerOptions::new()
      .first_pipe_instance(true)
      .create(&name)
      .expect("a pipe")
  };
  let _held = runtime.spawn(async move {
    silent.connect().await.expect("connected");
    tokio::time::sleep(Duration::from_secs(30)).await;
    drop(silent);
  });
  let started = Instant::now();
  let refused = pipe::hand_over(runtime.handle(), data.path(), this_process(), "orchester://open");
  assert!(matches!(refused, Err(HandOverError::NoAnswer)), "{refused:?}");
  let waited = started.elapsed();
  assert!(
    waited >= ANSWER_WAIT && waited < ANSWER_WAIT + Duration::from_secs(2),
    "{waited:?}"
  );
}
