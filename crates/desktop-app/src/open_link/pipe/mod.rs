//! One window takes a link: a program started with a link that finds the data folder's lock held hands the link to the
//! app holding it over a pipe of that data folder, and ends. The holder reads it (the program that hands it over never
//! does), answers, comes to the front and opens it ([`super::LinkHost`]). One line of at most [`super::MAX_LINE`]
//! bytes goes each way. A holder that does not answer within [`ANSWER_WAIT`] (an older build, one that is closing)
//! leaves the program that started to its *Orchester is already open* screen.
//!
//! Windows: a named pipe, `\\.\pipe\orchester-link-<hash of the folder>` ([`windows`]). macOS has no hand-over yet:
//! there a link comes to the running app as an Apple event, which is not built; [`hand_over`] and [`listen`] say so.

#[cfg(windows)]
mod windows;

use std::{io, path::Path, time::Duration};

use orchester_store::location::LockHolder;
use tokio::{runtime::Handle, task::JoinHandle};

#[cfg(windows)]
pub use self::windows::name;
use super::LinkInbox;

/// How long the program that hands a link over waits for the holder's answer, and the holder for the link.
pub const ANSWER_WAIT: Duration = Duration::from_secs(2);

/// Why a link was not handed over.
#[derive(Debug, thiserror::Error)]
pub enum HandOverError {
  /// The lock's record does not say which app holds it, so nothing can tell its pipe from another process's.
  #[error("the app that has the data folder open did not say which process it is")]
  NoHolder,
  /// Another process serves the pipe's name.
  #[error("process {served} serves the link pipe, not the app {holder} that has the data folder open")]
  NotTheHolder { served: u32, holder: u32 },
  #[error("the app that has the data folder open did not answer within {} seconds", ANSWER_WAIT.as_secs())]
  NoAnswer,
  #[error("the app that has the data folder open did not take the link")]
  NotTaken,
  #[error("the link pipe failed: {0}")]
  Pipe(#[from] io::Error),
  #[error("this platform hands no links over yet")]
  Unsupported,
}

/// Hands `link` to `holder`, the app that has the data folder `folder` open, waiting at most [`ANSWER_WAIT`] on
/// `runtime`.
pub fn hand_over(runtime: &Handle, folder: &Path, holder: Option<LockHolder>, link: &str) -> Result<(), HandOverError> {
  let holder = holder.ok_or(HandOverError::NoHolder)?;
  #[cfg(windows)]
  {
    let name = windows::name(folder);
    // The timer is made on the runtime, which has the clock it runs on.
    let sent =
      runtime.block_on(async { tokio::time::timeout(ANSWER_WAIT, windows::send(&name, holder.pid, link)).await });
    sent.map_err(|_| HandOverError::NoAnswer)?
  }
  #[cfg(not(windows))]
  {
    let _unused = (runtime, folder, holder, link);
    Err(HandOverError::Unsupported)
  }
}

/// Takes the links handed over for the data folder `folder` into `inbox`, on `runtime`, until the returned task is
/// ended. The pipe is made before this returns: a name another process already serves is refused.
pub fn listen(runtime: &Handle, folder: &Path, inbox: LinkInbox) -> io::Result<JoinHandle<()>> {
  #[cfg(windows)]
  {
    windows::listen(runtime, &windows::name(folder), inbox)
  }
  #[cfg(not(windows))]
  {
    let _unused = (runtime, folder, inbox);
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "this platform takes no links from other programs yet",
    ))
  }
}
