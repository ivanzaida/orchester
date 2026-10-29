//! The stack of another thread of this process on Windows x86-64: the thread is suspended, its registers read
//! (`GetThreadContext`) and unwound frame by frame with the loaded modules' unwind tables (`RtlLookupFunctionEntry`,
//! `RtlVirtualUnwind`), then resumed. std reads no other thread's registers, and windows-sys has no safe wrapper for
//! these calls, so this module makes them directly.
//!
//! While the thread is suspended nothing here allocates or logs: it may be stopped holding the heap's or the log's
//! lock, which this thread would then wait on forever. The addresses go into a fixed array on this thread's stack.

#![allow(unsafe_code)]

use std::ptr;

use windows_sys::Win32::{
  Foundation::{CloseHandle, HANDLE},
  System::{
    Diagnostics::Debug::{
      CONTEXT, CONTEXT_CONTROL_AMD64, CONTEXT_INTEGER_AMD64, GetThreadContext, RtlLookupFunctionEntry,
      RtlVirtualUnwind, UNW_FLAG_NHANDLER,
    },
    Threading::{
      GetCurrentThreadId, OpenThread, ResumeThread, SuspendThread, THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION,
      THREAD_SUSPEND_RESUME,
    },
  },
};

use super::{MAX_FRAMES, UiStack};

/// A thread's id: the window's thread lives as long as the process, so its id names it whenever the stack is read.
pub(super) type ThreadId = u32;

/// What `SuspendThread` and `ResumeThread` answer when they fail.
const FAILED: u32 = u32::MAX;

pub(super) fn current() -> ThreadId {
  // SAFETY: takes no arguments and only reads the calling thread's id.
  unsafe { GetCurrentThreadId() }
}

pub(super) fn stack(thread: ThreadId) -> UiStack {
  if thread == current() {
    return UiStack::NotCaptured("the thread cannot read its own stack");
  }
  let Some(handle) = ThreadHandle::open(thread) else {
    tracing::warn!(error = %std::io::Error::last_os_error(), "cannot open the window's thread to read its stack");
    return UiStack::NotCaptured("the window's thread could not be opened");
  };
  let mut frames = [0_u64; MAX_FRAMES];
  match handle.capture(&mut frames) {
    Ok(count) => UiStack::Captured(frames[..count].to_vec()),
    Err(reason) => {
      tracing::warn!(error = %std::io::Error::last_os_error(), reason, "cannot read the window's thread's stack");
      UiStack::NotCaptured(reason)
    }
  }
}

/// `CONTEXT` on the 16-byte boundary `GetThreadContext` requires on x86-64; windows-sys declares it without one.
#[repr(C, align(16))]
struct AlignedContext(CONTEXT);

/// A thread opened to be suspended and its registers read; closed when dropped.
struct ThreadHandle(HANDLE);

impl ThreadHandle {
  fn open(thread: ThreadId) -> Option<Self> {
    let access = THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION;
    // SAFETY: plain values; a null handle (failure) is checked before use.
    let handle = unsafe { OpenThread(access, 0, thread) };
    (!handle.is_null()).then_some(Self(handle))
  }

  /// Suspends the thread, writes its frames' addresses into `frames` and resumes it; the number written, or why none
  /// could be read.
  fn capture(&self, frames: &mut [u64; MAX_FRAMES]) -> Result<usize, &'static str> {
    // SAFETY: the handle is open with THREAD_SUSPEND_RESUME, and it is not this thread (checked in `stack`).
    if unsafe { SuspendThread(self.0) } == FAILED {
      return Err("the window's thread could not be suspended");
    }
    let mut context = AlignedContext(CONTEXT {
      ContextFlags: CONTEXT_CONTROL_AMD64 | CONTEXT_INTEGER_AMD64,
      ..CONTEXT::default()
    });
    // SAFETY: the handle is open with THREAD_GET_CONTEXT and the thread is suspended, so its registers hold still;
    // `context` is aligned to 16 bytes and lives to the end of this frame. GetThreadContext also waits until the
    // suspension has taken effect.
    let read = unsafe { GetThreadContext(self.0, &raw mut context.0) } != 0;
    let count = if read { walk(&mut context.0, frames) } else { 0 };
    // SAFETY: the handle is open with THREAD_SUSPEND_RESUME, and the thread was suspended above.
    if unsafe { ResumeThread(self.0) } == FAILED {
      tracing::error!(error = %std::io::Error::last_os_error(), "the window's thread could not be resumed");
    }
    read
      .then_some(count)
      .ok_or("the window's thread's registers could not be read")
  }
}

impl Drop for ThreadHandle {
  fn drop(&mut self) {
    // SAFETY: the handle was opened by `open` and is closed only here. Closing an open handle does not fail, and
    // there is nothing to do if it did.
    unsafe { CloseHandle(self.0) };
  }
}

/// Unwinds the suspended thread's `context` caller by caller, writing each frame's instruction address into `frames`;
/// how many it wrote. It stops at the outermost frame, at a frame no unwind table describes below the first, or when
/// the stack pointer stops moving toward the callers.
fn walk(context: &mut CONTEXT, frames: &mut [u64; MAX_FRAMES]) -> usize {
  let mut count = 0;
  while count < MAX_FRAMES && context.Rip != 0 {
    frames[count] = context.Rip;
    count += 1;
    let stack_pointer = context.Rsp;
    let mut image_base = 0_u64;
    // SAFETY: looks the address up in the unwind tables of the modules loaded in this process; the history table is
    // optional (null), and `image_base` is a live `u64` of this frame.
    let entry = unsafe { RtlLookupFunctionEntry(context.Rip, &raw mut image_base, ptr::null_mut()) };
    if !entry.is_null() {
      let mut handler_data = ptr::null_mut();
      let mut establisher_frame = 0_u64;
      // SAFETY: `entry` is the function entry the lookup returned for this address in the module at `image_base`;
      // `context` holds the frame's registers, which the call moves to the caller's, reading the suspended thread's
      // stack of this process; the out pointers point to live locals, and no context pointers are asked for (null).
      unsafe {
        RtlVirtualUnwind(
          UNW_FLAG_NHANDLER,
          image_base,
          context.Rip,
          entry,
          context,
          &raw mut handler_data,
          &raw mut establisher_frame,
          ptr::null_mut(),
        )
      };
    } else if count == 1 {
      // A leaf function, which has no unwind entry and keeps no frame: its return address is at the stack pointer.
      // SAFETY: the stack pointer of a suspended thread of this process, read from its registers, points into that
      // thread's committed stack.
      context.Rip = unsafe { ptr::with_exposed_provenance::<u64>(context.Rsp as usize).read_unaligned() };
      context.Rsp += 8;
    } else {
      break;
    }
    if context.Rsp <= stack_pointer {
      break;
    }
  }
  count
}
