//! Holding Windows awake: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`. The state belongs to the
//! thread that set it and ends with that thread, so the hold starts a thread that sets it, waits to be told to let
//! go, and clears it. windows-sys has no safe wrapper for the call, so this module calls it directly.

#![allow(unsafe_code)]

use std::{
  sync::mpsc::{self, Sender},
  thread::{self, JoinHandle},
};

use windows_sys::Win32::System::Power::{ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState};

use super::AwakeError;

/// The thread that holds the system awake, and the way to tell it to let go.
#[derive(Debug)]
pub(super) struct Hold {
  release: Option<Sender<()>>,
  thread: Option<JoinHandle<()>>,
}

impl Hold {
  pub(super) fn take() -> Result<Self, AwakeError> {
    let (release, released) = mpsc::channel::<()>();
    let (held, was_held) = mpsc::channel::<bool>();
    let thread = thread::Builder::new()
      .name("orchester-awake".to_owned())
      .spawn(move || {
        // SAFETY: the call takes flags only and changes this thread's execution state; it has no pointers.
        let before = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
        // The starter went away before it heard: there is nobody to hold for.
        if held.send(before != 0).is_err() || before == 0 {
          return;
        }
        // Waits until the hold is dropped: the closed channel is that signal, so the receive's error is the answer.
        let _dropped = released.recv();
        // SAFETY: as above; `ES_CONTINUOUS` alone clears what this thread asked for.
        unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
      })
      .map_err(|error| AwakeError::Refused(error.to_string()))?;
    match was_held.recv() {
      Ok(true) => Ok(Self {
        release: Some(release),
        thread: Some(thread),
      }),
      Ok(false) | Err(_) => Err(AwakeError::Refused(
        "SetThreadExecutionState did not take the request".to_owned(),
      )),
    }
  }
}

impl Drop for Hold {
  fn drop(&mut self) {
    // Dropping the sender wakes the thread, which clears the state and ends.
    drop(self.release.take());
    if let Some(thread) = self.thread.take()
      && thread.join().is_err()
    {
      tracing::warn!("the thread that kept the computer awake panicked");
    }
  }
}
