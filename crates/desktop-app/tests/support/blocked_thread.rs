//! A stand-in for a frozen window's thread: a thread that stops in a function of its own until the test releases it.

use std::{
  sync::mpsc::{self, Receiver, Sender},
  thread::JoinHandle,
};

use orchester_app::hang_reports::stack::UiThread;

/// A thread blocked on a channel until [`release`](Self::release); it tells which thread it is first.
pub struct BlockedThread {
  thread: UiThread,
  release: Sender<()>,
  handle: JoinHandle<()>,
}

impl BlockedThread {
  pub fn start() -> Self {
    let (told, which) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let handle = std::thread::Builder::new()
      .name("blocked-window".to_owned())
      .spawn(move || stuck_in_the_window(&told, &released))
      .expect("the OS starts a test thread");
    let thread = which.recv().expect("the blocked thread tells which thread it is");
    Self {
      thread,
      release,
      handle,
    }
  }

  pub fn thread(&self) -> UiThread {
    self.thread
  }

  /// Releases the thread and waits for it to end: it ends only if it was resumed after every stack read.
  pub fn release(self) {
    self.release.send(()).expect("the blocked thread waits for its release");
    self.handle.join().expect("the blocked thread ends without a panic");
  }
}

#[inline(never)]
fn stuck_in_the_window(told: &Sender<UiThread>, released: &Receiver<()>) {
  told.send(UiThread::current()).expect("the test waits to be told");
  released.recv().expect("the test releases the thread");
}
