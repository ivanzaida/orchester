//! Serves the desktop over the project's IPC endpoint: answers its requests and forwards the
//! scheduler's change notifications.

use std::{future::Future, path::Path, time::Duration};

use orchester_core::{
  id::RunId,
  run::{InterruptCause, RaisedLimits},
};
use orchester_ipc::{
  Command, Endpoint, Event, IpcError, Rejection, Reply, Request, RequestId, ShutdownMode,
  frame::{FrameReader, FrameWriter},
  handshake,
};
use tokio::sync::mpsc;

use crate::{
  execution::{Notifier, Scheduler},
  notify::coalesce,
};

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
  // Replies of requests answered later travel with the notifications.
  let replies = notifications.clone();
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
        Some(Ok(Some(request))) => match answer(&scheduler, &replies, &request) {
          Answer::Reply(event) => writer.write(&event).await?,
          Answer::Later => {}
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
      Some(event) = outgoing.recv() => {
        let mut waiting = vec![event];
        while let Ok(event) = outgoing.try_recv() {
          waiting.push(event);
        }
        for event in coalesce(waiting) {
          writer.write(&event).await?;
        }
      }
    }
  };
  reading.abort();
  result
}

enum Answer {
  Reply(Event),
  /// The reply follows on the notification channel once the work is done.
  Later,
  Shutdown(ShutdownMode),
}

fn answer(scheduler: &Scheduler, replies: &mpsc::UnboundedSender<Event>, request: &Request) -> Answer {
  let id = request.id;
  let reply = match request.command {
    Command::Attach => Reply::Attached {
      active_runs: scheduler.active_runs(),
    },
    Command::Ping { nonce } => Reply::Pong { nonce },
    Command::StartRun { key, task } => match scheduler.start_run(key, task) {
      Ok(run) => Reply::RunStarted { run },
      Err(rejection) => Reply::Rejected(rejection),
    },
    Command::PauseRun { run } => {
      let scheduler = scheduler.clone();
      return later(replies, id, run, async move { scheduler.pause_run(run).await });
    }
    Command::CancelRun { run } => {
      let scheduler = scheduler.clone();
      return later(replies, id, run, async move { scheduler.cancel_run(run).await });
    }
    Command::ResumeRun { run } => accepted(run, scheduler.resume_run(run)),
    Command::RaiseLimit {
      run,
      budget,
      max_duration_ms,
    } => {
      let limits = RaisedLimits {
        budget,
        max_duration: Duration::from_millis(max_duration_ms),
      };
      accepted(run, scheduler.raise_limit(run, limits))
    }
    Command::Shutdown { mode } => return Answer::Shutdown(mode),
  };
  Answer::Reply(Event::Reply { id, reply })
}

fn accepted(run: RunId, result: Result<(), Rejection>) -> Reply {
  match result {
    Ok(()) => Reply::Accepted { run },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// Answers request `id` when `work` for `run` is done, without holding up other requests.
fn later(
  replies: &mpsc::UnboundedSender<Event>,
  id: RequestId,
  run: RunId,
  work: impl Future<Output = Result<(), Rejection>> + Send + 'static,
) -> Answer {
  let replies = replies.clone();
  tokio::spawn(async move {
    let reply = accepted(run, work.await);
    if replies.send(Event::Reply { id, reply }).is_err() {
      tracing::debug!(%id, "the desktop left before the reply");
    }
  });
  Answer::Later
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
