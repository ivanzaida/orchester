//! Platforms whose stuck thread's stack is not read yet: macOS (it needs `thread_get_state` on the main thread's Mach
//! port and a frame-pointer walk) and Windows on ARM. A hang report then says the stack was not captured, rather than
//! leaving it out.

use super::UiStack;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ThreadId;

pub(super) fn current() -> ThreadId {
  ThreadId
}

pub(super) fn stack(_: ThreadId) -> UiStack {
  UiStack::NotCaptured(NOT_CAPTURED)
}

#[cfg(target_os = "macos")]
const NOT_CAPTURED: &str = "not captured on macOS yet";
#[cfg(not(target_os = "macos"))]
const NOT_CAPTURED: &str = "not captured on this platform yet";
