//! `orchester-fake-runner <project-root>`: a stand-in for `orchester-runner` in the runner start
//! tests. It does what the project's `fake-runner` file says:
//!
//! - `delay <seconds>`: opens its endpoint after that many seconds, then serves;
//! - `exit-first`: exits with a failure on its first start in the project, serves on the next;
//! - `exit`: always exits with a failure;
//! - `locked`: exits as a runner does when another one holds the project.
//!
//! Serving means accepting the desktop, answering the handshake and holding the connection until
//! the desktop closes it. Every start appends a line to the project's `starts` file.

use std::{
  fs::{self, OpenOptions},
  io::Write,
  path::{Path, PathBuf},
  process::ExitCode,
  time::Duration,
};

use orchester_ipc::{
  EXIT_PROJECT_LOCKED, Endpoint, IpcError, Request,
  frame::{FrameReader, FrameWriter},
  handshake,
};

fn main() -> ExitCode {
  let root = PathBuf::from(std::env::args_os().nth(1).expect("the project root argument"));
  OpenOptions::new()
    .create(true)
    .append(true)
    .open(root.join("starts"))
    .and_then(|mut starts| starts.write_all(b"start\n"))
    .expect("note the start");
  let plan = fs::read_to_string(root.join("fake-runner")).expect("read the fake-runner file");
  let mut words = plan.split_whitespace();
  let delay = match (words.next(), words.next()) {
    (Some("delay"), Some(seconds)) => Duration::from_secs(seconds.parse().expect("seconds")),
    (Some("exit-first"), None) if first_start(&root) => return ExitCode::FAILURE,
    (Some("exit-first"), None) => Duration::ZERO,
    (Some("exit"), None) => return ExitCode::FAILURE,
    (Some("locked"), None) => return ExitCode::from(EXIT_PROJECT_LOCKED),
    _ => panic!("unknown fake-runner plan: {plan}"),
  };
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .expect("runtime");
  runtime.block_on(serve(&root, delay)).expect("serve the desktop");
  ExitCode::SUCCESS
}

/// `true` on the first start in the project, which leaves a marker for the next one.
fn first_start(root: &Path) -> bool {
  let marker = root.join("started-once");
  if marker.exists() {
    return false;
  }
  fs::write(marker, b"").expect("leave the marker");
  true
}

async fn serve(root: &Path, delay: Duration) -> Result<(), IpcError> {
  tokio::time::sleep(delay).await;
  let endpoint = Endpoint::for_project(root)?;
  let stream = endpoint.accept().await?;
  let (read, write) = tokio::io::split(stream);
  let mut reader = FrameReader::new(read);
  let mut writer = FrameWriter::new(write);
  handshake(&mut reader, &mut writer).await?;
  while reader.read::<Request>().await?.is_some() {}
  Ok(())
}
