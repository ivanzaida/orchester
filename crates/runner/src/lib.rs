//! The runner process of one project, as described in `docs/architecture/storage.md`: it holds
//! the project's runner lock, serves the desktop over IPC, executes runs (worktrees, agent CLIs,
//! checkpoints) and records them in the project database.
//!
//! The IPC server exists on Windows only for now (named pipes; a known gap in `AGENTS.md`).
//! Execution, worktrees and process supervision build on both platforms.

pub mod execution;
mod git;
pub mod lock;
pub mod log_file;
pub mod notify;
pub mod private_file;
#[cfg(windows)]
pub mod server;
pub mod supervisor;
pub mod worktree;
