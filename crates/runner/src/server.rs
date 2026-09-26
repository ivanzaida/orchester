//! Serves the desktop over the project's IPC endpoint: answers its requests and forwards the
//! scheduler's change notifications.

use std::{path::Path, time::Duration};

use orchester_core::run::InterruptCause;
use orchester_ipc::{
  Command, Endpoint, Event, IpcError, Reply, Request, ShutdownMode,
  frame::{FrameReader, FrameWriter},
  handshake,
};
use tokio::sync::mpsc;

use crate::execution::{Notifier, Scheduler};

/// After [`Event::Stopped`] the runner waits this long for the desktop to close the pipe, so
/// the event is read before the pipe goes away.
const DISCONNECT_GRACE: Duration = Duration::from_secs(5);

type Incoming = Result<Option<Request>, IpcError>;

/// Accepts the desktop's connection and serves it until the desktop disconnects or asks the
/// runner to stop. Runs still executing then are interrupted: their agents are ended and the
/// runs keep their last checkpoint.
pub async fn serve(project_root: &Path) -> Result<(), IpcError> {
  let endpoint = Endpoint::for_project(project_root)?;
  tracing::info!("listening on {}", endpoint.name());
  let stream = endpoint.accept().await?;
  let (read, write) = tokio::io::split(stream);
  let mut reader = FrameReader::new(read);
  let mut writer = FrameWriter::new(write);
  handshake(&mut reader, &mut writer).await?;
  tracing::info!("desktop connected");
  let (notifications, mut outgoing) = mpsc::unbounded_channel();
  let scheduler = Scheduler::new(project_root, Notifier::new(notifications));
  // A frame read cannot be interrupted without losing data, so requests are read on their own task.
  let (incoming, mut requests) = mpsc::unbounded_channel::<Incoming>();
  let reading = tokio::spawn(async move {
    loop {
      let request = reader.read::<Request>().await.map_err(IpcError::from);
      let last = !matches!(request, Ok(Some(_)));
      if incoming.send(request).is_err() || last {
        return;
      }
    }
  });
  let result = loop {
    tokio::select! {
      request = requests.recv() => match request {
        Some(Ok(Some(request))) => match answer(&scheduler, &request) {
          Answer::Reply(event) => writer.write(&event).await?,
          Answer::Shutdown(mode) => {
            tracing::info!("shutdown requested: {mode:?}");
            scheduler.stop_all(InterruptCause::QuitNow).await;
            while let Ok(event) = outgoing.try_recv() {
              writer.write(&event).await?;
            }
            writer.write(&Event::Stopped { mode }).await?;
            wait_for_disconnect(&mut requests).await;
            break Ok(());
          }
        },
        Some(Err(error)) => {
          tracing::warn!("desktop connection failed: {error}");
          scheduler.stop_all(InterruptCause::RunnerLost).await;
          break Err(error);
        }
        Some(Ok(None)) | None => {
          tracing::info!("desktop disconnected");
          scheduler.stop_all(InterruptCause::RunnerLost).await;
          break Ok(());
        }
      },
      Some(event) = outgoing.recv() => writer.write(&event).await?,
    }
  };
  reading.abort();
  result
}

enum Answer {
  Reply(Event),
  Shutdown(ShutdownMode),
}

fn answer(scheduler: &Scheduler, request: &Request) -> Answer {
  let reply = match request.command {
    Command::Attach => Reply::Attached {
      active_runs: scheduler.active_runs(),
    },
    Command::Ping { nonce } => Reply::Pong { nonce },
    Command::StartRun { key, task } => match scheduler.start_run(key, task) {
      Ok(run) => Reply::RunStarted { run },
      Err(rejection) => Reply::Rejected(rejection),
    },
    Command::Shutdown { mode } => return Answer::Shutdown(mode),
  };
  Answer::Reply(Event::Reply { id: request.id, reply })
}

async fn wait_for_disconnect(requests: &mut mpsc::UnboundedReceiver<Incoming>) {
  let drain = async {
    while let Some(Ok(Some(request))) = requests.recv().await {
      tracing::warn!("ignoring {:?} after shutdown", request.command);
    }
  };
  if tokio::time::timeout(DISCONNECT_GRACE, drain).await.is_err() {
    tracing::warn!("desktop kept the connection open after shutdown; exiting anyway");
  }
}
