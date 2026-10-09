//! What follows a person's message once it is stored, as the composer's action says ([`Then`]),
//! for the Conversation tab and for automation alike: the runner is told, and gives a message sent
//! now to the agent at work and delivers the others at the run's next invocation boundary; a paused
//! run resumes, an interrupted one continues, an ended task runs again, each under the person's
//! claim; or the message waits for the person to raise the limit the run stopped at. What the runner
//! does not take leaves the message queued, with why.

use std::{future::Future, path::Path};

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  run::RunAgainBlock,
};
use orchester_ipc::{Command, Rejection};

use super::composer::Then;
use crate::{
  messages::Message,
  runner::{RunRequestError, Runners},
  sharing::Servers,
  tasks::{run::control_message, run_again::request_messages},
  workspace::OpenProject,
};

/// A stored message: the runner took it, it stays queued because the runner could not be told or
/// refused to go on, or it waits for the person to raise the run's limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sent {
  Taken,
  Queued(Message),
  AwaitsLimit,
}

/// Which project's runner hears about the message: the one the window shows, or the one automation
/// names, which starts if it has none.
#[derive(Clone, Copy)]
pub enum RunnerOf<'a> {
  Displayed,
  Project(&'a OpenProject),
}

/// The stored message's task and run, as what follows it needs them.
pub struct Stored<'a> {
  pub root: &'a Path,
  pub task: TaskId,
  /// The task's latest run.
  pub run: RunId,
  /// The message's idempotency key; running the task again with it starts one run per message.
  pub key: IdempotencyKey,
  /// The latest run's pipeline, which a refusal to run the task again names.
  pub pipeline: &'a str,
}

impl RunnerOf<'_> {
  async fn control(self, runners: &Runners, command: Command) -> Result<(), RunRequestError> {
    match self {
      Self::Displayed => runners.control_run(command).await,
      Self::Project(project) => runners.control_run_in(project, command).await,
    }
  }

  async fn continue_run(self, runners: &Runners, run: RunId) -> Result<(), RunRequestError> {
    match self {
      Self::Displayed => runners.continue_run(run).await,
      Self::Project(project) => runners.continue_run_in(project, run).await,
    }
  }

  async fn run_again(self, runners: &Runners, stored: &Stored<'_>) -> Result<(), RunRequestError> {
    let started = match self {
      Self::Displayed => runners.run_again(stored.key, stored.task, stored.run, None).await,
      Self::Project(project) => {
        runners
          .run_again_in(project, stored.key, (stored.task, stored.run))
          .await
      }
    };
    started.map(|_| ())
  }
}

/// What follows `stored` as `then` says, through `runner`.
pub async fn follow(
  (runners, servers, runner): (&Runners, &Servers, RunnerOf<'_>),
  stored: &Stored<'_>,
  then: Then,
) -> Sent {
  let claimed = (servers, stored.root, stored.task);
  match then {
    Then::Deliver => tell((runners, servers, runner), (stored.root, stored.task, stored.run)).await,
    Then::RaiseLimit => Sent::AwaitsLimit,
    Then::Resume => {
      let resume = || runner.control(runners, Command::ResumeRun { run: stored.run });
      go_on(claimed, resume, control_message).await
    }
    Then::ContinueInterrupted => go_on(claimed, || runner.continue_run(runners, stored.run), control_message).await,
    Then::RunAgain => {
      let refused = |error: &RunRequestError| {
        request_messages(error, stored.pipeline)
          .into_iter()
          .next()
          .unwrap_or_else(|| control_message(error))
      };
      go_on(claimed, || runner.run_again(runners, stored), refused).await
    }
  }
}

/// Tells the runner that the run has a message; what it could not take stays queued. A message
/// makes the agents of a shared task work, so the person's claim comes first (Send & continue, a
/// follow-up); without it the message stays queued, with who holds the task.
pub async fn tell(
  (runners, servers, runner): (&Runners, &Servers, RunnerOf<'_>),
  (root, task, run): (&Path, TaskId, RunId),
) -> Sent {
  let claim = match servers.claim_for_work(root, task).await {
    Ok(claim) => claim,
    Err(refusal) => return Sent::Queued(refusal.message()),
  };
  match runner.control(runners, Command::DeliverMessages { run }).await {
    Ok(()) => Sent::Taken,
    // A blocked run keeps the message; the runner delivers it once the blocking tasks finish.
    Err(RunRequestError::Rejected(Rejection::RunAgainBlocked {
      block: RunAgainBlock::TaskBlocked,
    })) => Sent::Queued(Message::new("tasks.blocked.messageKept")),
    Err(error) => {
      tracing::warn!(%error, "the runner did not take the message");
      claim.release(servers).await;
      Sent::Queued(control_message(&error))
    }
  }
}

/// Lets the run go on with `request` so that its agents read the stored message. It makes agents
/// work, so the person's claim comes first; what the runner refuses leaves the message queued, with
/// why (`refused`).
async fn go_on<F>(
  (servers, root, task): (&Servers, &Path, TaskId),
  request: impl FnOnce() -> F,
  refused: impl FnOnce(&RunRequestError) -> Message,
) -> Sent
where
  F: Future<Output = Result<(), RunRequestError>>,
{
  let claim = match servers.claim_for_work(root, task).await {
    Ok(claim) => claim,
    Err(refusal) => return Sent::Queued(refusal.message()),
  };
  match request().await {
    Ok(()) => Sent::Taken,
    Err(error) => {
      tracing::warn!(%error, "the run did not go on with the message");
      claim.release(servers).await;
      Sent::Queued(refused(&error))
    }
  }
}
