//! The runner process of one project, as described in `docs/architecture/storage.md`: it holds
//! the project's runner lock, serves the desktop over IPC, executes runs (worktrees, agent CLIs,
//! checkpoints) and records them in the project database.

pub mod execution;
mod git;
pub mod lock;
pub mod log_file;
pub mod notify;
pub mod private_file;
pub mod server;
pub mod shutdown_signal;
pub mod supervisor;
pub mod worktree;
