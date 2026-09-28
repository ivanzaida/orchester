//! The channel between the desktop and a project's runner process, as described in
//! `docs/architecture/storage.md`.
//!
//! Every connection starts with a [`handshake`] in which both sides send their
//! [`PROTOCOL_VERSION`]; after that the desktop sends [`Request`]s and the runner sends
//! [`Event`]s (replies and change notifications), each as one length-prefixed JSON frame (see
//! [`frame`]).
//!
//! The transport is a Windows named pipe or a macOS Unix domain socket (see `transport`); the
//! rest of the crate is written against `AsyncRead`/`AsyncWrite`, so both carry the same frames.

mod client;
mod error;
mod exit_code;
pub mod frame;
mod handshake;
#[cfg(windows)]
mod job;
mod log_path;
mod message;
#[cfg(windows)]
mod pipe_owner;
mod transport;
mod work;

pub use client::{CommandSender, EventReceiver, LaunchError, RunnerClient, RunnerProcess};
pub use error::{FrameError, IpcError};
pub use exit_code::EXIT_PROJECT_LOCKED;
pub use handshake::{Handshake, PROTOCOL_VERSION, handshake};
#[cfg(windows)]
pub use job::KillOnCloseJob;
pub use log_path::{runner_log_dir, runner_log_path, utc_date};
pub use message::{
  AttachSession, Command, Event, RefusedRun, Rejection, Reply, Request, RequestId, SessionEnd, ShutdownMode,
};
pub use transport::{ClientStream, Endpoint, ServerStream, server_process_id};
pub use work::{RunWork, WorkState, WorkSummary};
