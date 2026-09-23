//! Serves the desktop over the project's IPC endpoint.

use std::{path::Path, time::Duration};

use orchester_ipc::{
  Command, Endpoint, Event, IpcError, ServerStream, ShutdownMode,
  frame::{FrameReader, FrameWriter},
  handshake,
};
use tokio::io::ReadHalf;

/// After [`Event::Stopped`] the runner waits this long for the desktop to close the pipe, so
/// the event is read before the pipe goes away.
const DISCONNECT_GRACE: Duration = Duration::from_secs(5);

/// Accepts the desktop's connection and handles commands until the desktop disconnects or asks
/// the runner to stop.
///
/// Nothing is scheduled yet, so there are no active steps: both shutdown modes stop at once.
pub async fn serve(project_root: &Path) -> Result<(), IpcError> {
  let endpoint = Endpoint::for_project(project_root)?;
  tracing::info!("listening on {}", endpoint.name());
  let stream = endpoint.accept().await?;
  let (read, write) = tokio::io::split(stream);
  let mut commands = FrameReader::new(read);
  let mut events = FrameWriter::new(write);
  handshake(&mut commands, &mut events).await?;
  tracing::info!("desktop connected");
  while let Some(command) = commands.read::<Command>().await? {
    match command {
      Command::Ping { nonce } => events.write(&Event::Pong { nonce }).await?,
      Command::Shutdown { mode } => {
        tracing::info!("shutdown requested: {mode:?}");
        events.write(&Event::Stopped { mode }).await?;
        wait_for_disconnect(&mut commands, mode).await;
        return Ok(());
      }
    }
  }
  tracing::info!("desktop disconnected");
  Ok(())
}

async fn wait_for_disconnect(commands: &mut FrameReader<ReadHalf<ServerStream>>, mode: ShutdownMode) {
  let drain = async {
    loop {
      match commands.read::<Command>().await {
        Ok(Some(command)) => tracing::warn!("ignoring {command:?} after {mode:?} shutdown"),
        Ok(None) => return,
        Err(error) => {
          tracing::warn!("connection failed while stopping: {error}");
          return;
        }
      }
    }
  };
  if tokio::time::timeout(DISCONNECT_GRACE, drain).await.is_err() {
    tracing::warn!("desktop kept the connection open after shutdown; exiting anyway");
  }
}
