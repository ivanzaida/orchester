//! Serves the desktop over the project's IPC endpoint: answers its requests and forwards the
//! scheduler's change notifications.

use std::{
  future::Future,
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{ApprovalAnswer, InterruptCause, PlanDecision, PlanVersion, RaisedLimits, StuckDecision},
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
  shutdown_signal::ShutdownSignal,
};

/// After [`Event::Stopped`] the runner waits this long for the desktop to close the connection,
/// so the event is read before the connection goes away.
const DISCONNECT_GRACE: Duration = Duration::from_secs(5);

type Incoming = Result<Option<Request>, IpcError>;

/// Accepts the desktop's connection and serves it until the desktop disconnects, asks the
/// runner to stop, or `shutdown` is signalled. Runs still executing then are interrupted: their
/// agents are ended and the runs keep their last checkpoint.
pub async fn serve(project_root: &Path, mut shutdown: ShutdownSignal) -> Result<(), IpcError> {
  // Made before the desktop can connect: it notes the groups queued before this runner started.
  let (notifications, outgoing) = mpsc::unbounded_channel();
  // Replies of requests answered later travel with the notifications.
  let replies = notifications.clone();
  let relay = std::env::current_exe()
    .inspect_err(
      |error| tracing::error!(%error, "cannot find the runner's own program for Claude Code's permission prompt"),
    )
    .ok();
  let scheduler = Scheduler::new(
    project_root,
    Notifier::new(notifications),
    Arc::new(KeychainVault),
    relay,
  );
  let endpoint = Endpoint::for_project(project_root)?;
  tracing::info!("listening on {}", endpoint.name());
  let stream = tokio::select! {
    stream = endpoint.accept() => stream?,
    () = shutdown.received() => {
      tracing::info!("stopping before the desktop connected");
      return Ok(());
    }
  };
  let (read, write) = tokio::io::split(stream);
  let mut reader = FrameReader::new(read);
  let mut writer = FrameWriter::new(write);
  handshake(&mut reader, &mut writer).await?;
  tracing::info!("desktop connected");
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
    in_flight: InFlight::default(),
    shutdown,
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
  in_flight: InFlight,
  shutdown: ShutdownSignal,
}

/// Requests about a run still being carried out after the request was read ([`Later`]): work the
/// runner has, since each may execute a run again.
#[derive(Clone, Default)]
struct InFlight(Arc<AtomicUsize>);

impl InFlight {
  fn start(&self) -> InFlightRequest {
    self.0.fetch_add(1, Ordering::AcqRel);
    InFlightRequest(self.clone())
  }

  fn any(&self) -> bool {
    self.0.load(Ordering::Acquire) > 0
  }
}

/// One request in flight until dropped.
struct InFlightRequest(InFlight);

impl Drop for InFlightRequest {
  fn drop(&mut self) {
    (self.0).0.fetch_sub(1, Ordering::AcqRel);
  }
}

impl<W: tokio::io::AsyncWrite + Unpin> Session<W> {
  async fn run(mut self) -> Result<(), IpcError> {
    let mut groups = tokio::time::interval(Duration::from_secs(1));
    loop {
      tokio::select! {
        _ = groups.tick(), if self.finishing.is_none() => {
          if let Err(error) = self.scheduler.advance_task_groups().await {
            tracing::error!(?error, "cannot advance reviewed task groups");
          }
        }
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
        () = self.shutdown.received() => {
          self.scheduler.stop_all(InterruptCause::QuitNow).await;
          return Ok(());
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
    match answer(&self.scheduler, (&self.replies, &self.in_flight), &request).await {
      Answer::Reply(event) => self.writer.write(&event).await?,
      Answer::Later => {}
      Answer::StopIfIdle(id) => return self.stop_if_idle(id).await,
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

  /// Stops when the runner has no work; otherwise answers that it is busy and goes on.
  async fn stop_if_idle(&mut self, id: RequestId) -> Result<Option<Result<(), IpcError>>, IpcError> {
    let busy = if self.finishing.is_some() {
      Some(Rejection::ShuttingDown)
    } else if !self.scheduler.is_idle() || self.in_flight.any() {
      Some(Rejection::Busy)
    } else {
      None
    };
    if let Some(rejection) = busy {
      let reply = Reply::Rejected(rejection);
      self.writer.write(&Event::Reply { id, reply }).await?;
      return Ok(None);
    }
    tracing::info!("stopping: no tab shows the project and it has no work");
    self
      .writer
      .write(&Event::Reply {
        id,
        reply: Reply::Stopping,
      })
      .await?;
    Ok(Some(self.stopped(ShutdownMode::FinishAtCheckpoints).await))
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
  /// [`Command::StopIfIdle`], answered by the session, which knows the requests in flight.
  StopIfIdle(RequestId),
}

/// The reply to a request that starts a run.
fn started(result: Result<RunId, Rejection>) -> Reply {
  match result {
    Ok(run) => Reply::RunStarted { run },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

async fn answer(
  scheduler: &Scheduler,
  (replies, in_flight): (&mpsc::UnboundedSender<Event>, &InFlight),
  request: &Request,
) -> Answer {
  let id = request.id;
  // A request about a run is answered once it is carried out, with a scheduler handle of its own.
  let handle = scheduler.clone();
  let later = Later { replies, in_flight, id };
  let reply = match request.command {
    Command::Attach { session, agent_slots } => match scheduler.attach(session, agent_slots).await {
      Ok(attached) => Reply::Attached {
        active_runs: scheduler.active_runs(),
        interrupted: attached.interrupted,
        continued: attached.continued,
      },
      Err(rejection) => Reply::Rejected(rejection),
    },
    Command::ContinueRuns { ref runs } => {
      let (continued, refused) = scheduler.continue_runs(runs).await;
      Reply::Continued { continued, refused }
    }
    Command::Ping { nonce } => Reply::Pong { nonce },
    Command::StartRun { key, task } => started(scheduler.start_requested_run(key, task).await),
    Command::RunAgain {
      key,
      task,
      after,
      ref remove_worktrees,
    } => started(scheduler.run_again(key, task, after, remove_worktrees.clone()).await),
    Command::PauseRun { run } => return later.answer(run, async move { handle.pause_run(run).await }),
    Command::CancelRun { run } => return later.answer(run, async move { handle.cancel_run(run).await }),
    Command::ResumeRun { run } => return later.answer(run, async move { handle.resume_run(run).await }),
    Command::RaiseLimit {
      run,
      budget,
      max_duration_ms,
    } => {
      let limits = RaisedLimits {
        budget,
        max_duration: Duration::from_millis(max_duration_ms),
      };
      return later.answer(run, async move { handle.raise_limit(run, limits).await });
    }
    Command::RaiseStepBudget { run, budget } => {
      return later.answer(run, async move { handle.raise_step_budget(run, budget).await });
    }
    Command::ChangeRunPermissions {
      run,
      profile,
      ref permissions,
      apply,
    } => {
      let permissions = permissions.clone();
      return later.answer(run, async move {
        handle.change_permissions(run, profile, permissions, apply).await
      });
    }
    Command::KeepPermissionsForNextInvocation { run, profile } => {
      return later.answer(run, async move {
        handle.keep_permissions_for_next_invocation(run, profile).await
      });
    }
    Command::DecidePlan {
      run,
      execution,
      version,
      ref decision,
    } => return answer_plan(&later, handle, (run, execution, version), decision.clone()),
    Command::DecideStuckRun { run, ref decision } => return answer_stuck(&later, handle, run, decision.clone()),
    Command::DecideToolApproval {
      run,
      ref approval,
      ref answer,
    } => return answer_tool_approval(&later, handle, run, approval, answer.clone()),
    Command::DeliverMessages { run } => return later.answer(run, async move { handle.deliver_messages(run).await }),
    Command::Shutdown { mode } => return Answer::Shutdown(mode),
    Command::StopIfIdle => return Answer::StopIfIdle(id),
  };
  Answer::Reply(Event::Reply { id, reply })
}

fn answer_tool_approval(
  later: &Later<'_>,
  scheduler: Scheduler,
  run: RunId,
  approval: &str,
  answer: ApprovalAnswer,
) -> Answer {
  let approval = approval.to_owned();
  later.answer(run, async move {
    scheduler.decide_tool_approval(run, approval, answer).await
  })
}

fn answer_plan(
  later: &Later<'_>,
  scheduler: Scheduler,
  (run, execution, version): (RunId, StepExecutionId, PlanVersion),
  decision: PlanDecision,
) -> Answer {
  later.answer(run, async move {
    scheduler.decide_plan(run, execution, version, decision).await
  })
}

fn answer_stuck(later: &Later<'_>, scheduler: Scheduler, run: RunId, decision: StuckDecision) -> Answer {
  later.answer(run, async move { scheduler.decide_stuck(run, decision).await })
}

fn accepted(run: RunId, result: Result<(), Rejection>) -> Reply {
  match result {
    Ok(()) => Reply::Accepted { run },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// Answers request `id` once the work it asked for is done, without holding up other requests.
/// The request counts as in flight until then.
struct Later<'a> {
  replies: &'a mpsc::UnboundedSender<Event>,
  in_flight: &'a InFlight,
  id: RequestId,
}

impl Later<'_> {
  fn answer(&self, run: RunId, work: impl Future<Output = Result<(), Rejection>> + Send + 'static) -> Answer {
    let (replies, id) = (self.replies.clone(), self.id);
    let request = self.in_flight.start();
    tokio::spawn(async move {
      let reply = accepted(run, work.await);
      drop(request);
      if replies.send(Event::Reply { id, reply }).is_err() {
        tracing::debug!(%id, "the desktop left before the reply");
      }
    });
    Answer::Later
  }
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
