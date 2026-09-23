//! The runner process of one project, as described in `docs/architecture/storage.md`: it holds
//! the project's runner lock, serves the desktop over IPC, supervises child processes and
//! manages the git worktrees of runs.
//!
//! Windows only for now: the transport and process containment use named pipes and Job Objects.

#![cfg(windows)]

pub mod lock;
pub mod server;
pub mod supervisor;
pub mod worktree;

/// The runner has no console; without this flag every console child would open a window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
