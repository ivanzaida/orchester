//! Serves the desktop over the project's IPC endpoint: answers its requests and forwards the
//! scheduler's change notifications.

use std::{future::Future, path::Path, sync::Arc, time::Duration};

use orchester_core::{
  id::RunId,
  run::{InterruptCause, RaisedLimits},
};
use orchester_ipc::{
  Command, Endpoint, Event, IpcError, Rejection, Reply, Request, RequestId, ShutdownMode,
  frame::{FrameReader, FrameWriter},
  handshake,
};
use orchester_vault::KeychainVault;
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
  let (notifications, outgoing) = mpsc::unbounded_channel();
  // Replies of requests answered later travel with the notifications.
  let replies = notifications.clone();
  let scheduler = Scheduler::new(project_root, Notifier::new(notifications), Arc::new(KeychainVault));
  // A frame read cannot be interrupted without losing data, so requests are read on their own task.
  let (incoming, requests) = mpsc::unbounded_channel::<Incoming>();
  let reading = tokio::spawn(async move {
    loop {
      let request = reader.read::<Request>().await.map_err(IpcError::from);
      let last = !matches!(request, Ok(Some(_)));
      if incoming.send(request).is_err() || last {
        return;
      }
    }
  });
  let result = Session {
    scheduler,
    replies,
    writer,
    outgoing,
    requests,
    finishing: None,
  }
  .run()
  .await;
  reading.abort();
  result
}

/// A served connection: requests in, replies and notifications out.
struct Session<W> {
  scheduler: Scheduler,
  replies: mpsc::UnboundedSender<Event>,
  writer: FrameWriter<W>,
  outgoing: mpsc::UnboundedReceiver<Event>,
  requests: mpsc::UnboundedReceiver<Incoming>,
  /// "Wait for agents to save" runs while events keep flowing; it ends the connection when done.
  finishing: Option<tokio::task::JoinHandle<()>>,
}

impl<W: tokio::io::AsyncWrite + Unpin> Session<W> {
  async fn run(mut self) -> Result<(), IpcError> {
    loop {
      tokio::select! {
        request = self.requests.recv() => {
          if let Some(end) = self.on_request(request).await? {
            return end;
          }
        }
        Some(event) = self.outgoing.recv() => self.write_waiting(vec![event]).await?,
        finished = async { self.finishing.as_mut().expect("polled only while finishing").await }, if self.finishing.is_some() => {
          if let Err(error) = finished {
            tracing::error!(%error, "finishing at checkpoints failed");
          }
          return self.stopped(ShutdownMode::FinishAtCheckpoints).await;
        }
      }
    }
  }

  /// Handles what the desktop sent; `Some` once the connection is over.
  async fn on_request(&mut self, request: Option<Incoming>) -> Result<Option<Result<(), IpcError>>, IpcError> {
    let request = match request {
      Some(Ok(Some(request))) => request,
      Some(Err(error)) => {
        tracing::warn!("desktop connection failed: {error}");
        self.scheduler.stop_all(InterruptCause::RunnerLost).await;
        return Ok(Some(Err(error)));
      }
      Some(Ok(None)) | None => {
        tracing::info!("desktop disconnected");
        self.scheduler.stop_all(InterruptCause::RunnerLost).await;
        return Ok(Some(Ok(())));
      }
    };
    match answer(&self.scheduler, &self.replies, &request) {
      Answer::Reply(event) => self.writer.write(&event).await?,
      Answer::Later => {}
      Answer::Shutdown(ShutdownMode::FinishAtCheckpoints) => {
        tracing::info!("shutdown requested: finishing at checkpoints");
        if self.finishing.is_none() {
          let scheduler = self.scheduler.clone();
          self.finishing = Some(tokio::spawn(async move { scheduler.finish_at_checkpoints().await }));
        }
      }
      Answer::Shutdown(ShutdownMode::Immediate) => {
        tracing::info!("shutdown requested: immediately");
        self.scheduler.stop_all(InterruptCause::QuitNow).await;
        return Ok(Some(self.stopped(ShutdownMode::Immediate).await));
      }
    }
    Ok(None)
  }

  /// Writes `waiting` and every event queued behind it, coalesced.
  async fn write_waiting(&mut self, mut waiting: Vec<Event>) -> Result<(), IpcError> {
    while let Ok(event) = self.outgoing.try_recv() {
      waiting.push(event);
    }
    for event in coalesce(waiting) {
      self.writer.write(&event).await?;
    }
    Ok(())
  }

  /// The runner has stopped all work: the events still waiting, then [`Event::Stopped`], then
  /// the desktop's disconnect.
  async fn stopped(&mut self, mode: ShutdownMode) -> Result<(), IpcError> {
    self.write_waiting(Vec::new()).await?;
    self.writer.write(&Event::Stopped { mode }).await?;
    wait_for_disconnect(&mut self.requests).await;
    Ok(())
  }
}

enum Answer {
  Reply(Event),
  /// The reply follows on the notification channel once the work is done.
  Later,
  Shutdown(ShutdownMode),
}

fn answer(scheduler: &Scheduler, replies: &mpsc::UnboundedSender<Event>, request: &Request) -> Answer {
  let id = request.id;
  // A request about a run is answered once it is carried out, with a scheduler handle of its own.
  let handle = scheduler.clone();
  let reply = match request.command {
    Command::Attach { session, agent_slots } => match scheduler.attach(session, agent_slots) {
      Ok(attached) => Reply::Attached {
        active_runs: scheduler.active_runs(),
        interrupted: attached.interrupted,
        continued: attached.continued,
      },
      Err(rejection) => Reply::Rejected(rejection),
    },
    Command::ContinueRuns { ref runs } => {
      let (continued, refused) = scheduler.continue_runs(runs);
      Reply::Continued { continued, refused }
    }
    Command::Ping { nonce } => Reply::Pong { nonce },
    Command::StartRun { key, task } => match scheduler.start_run(key, task) {
      Ok(run) => Reply::RunStarted { run },
      Err(rejection) => Reply::Rejected(rejection),
    },
    Command::PauseRun { run } => return later(replies, id, run, async move { handle.pause_run(run).await }),
    Command::CancelRun { run } => return later(replies, id, run, async move { handle.cancel_run(run).await }),
    Command::ResumeRun { run } => return later(replies, id, run, async move { handle.resume_run(run).await }),
    Command::RaiseLimit {
      run,
      budget,
      max_duration_ms,
    } => {
      let limits = RaisedLimits {
        budget,
        max_duration: Duration::from_millis(max_duration_ms),
      };
      return later(replies, id, run, async move { handle.raise_limit(run, limits).await });
    }
    Command::RaiseStepBudget { run, budget } => {
      return later(
        replies,
        id,
        run,
        async move { handle.raise_step_budget(run, budget).await },
      );
    }
    Command::ChangeRunPermissions {
      run,
      profile,
      ref permissions,
      apply,
    } => {
      let permissions = permissions.clone();
      return later(replies, id, run, async move {
        handle.change_permissions(run, profile, permissions, apply).await
      });
    }
    Command::KeepPermissionsForNextInvocation { run, profile } => {
      return later(replies, id, run, async move {
        handle.keep_permissions_for_next_invocation(run, profile).await
      });
    }
    Command::DecidePlan {
      run,
      execution,
      version,
      ref decision,
    } => {
      let decision = decision.clone();
      return later(replies, id, run, async move {
        handle.decide_plan(run, execution, version, decision).await
      });
    }
    Command::DeliverMessages { run } => {
      return later(replies, id, run, async move { handle.deliver_messages(run).await });
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
