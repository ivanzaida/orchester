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
  sharing::TaskRevision,
};
use orchester_ipc::{Command, Rejection};

use super::composer::Then;
use crate::{
  messages::Message,
  readiness::NotStarted,
  runner::{RunRequestError, Runners},
  secrets::Vault,
  sharing::Servers,
  tasks::{run::control_message, run_again::request_messages},
  workspace::{OpenProject, Workspace},
};

/// A stored message: the runner took it, it ran the task again with this run, it stays queued because the runner
/// could not be told or refused to go on, it stays queued because running the task again is not possible here
/// (the pipeline is not ready on this computer), or it waits for the person to raise the run's limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sent {
  Taken,
  Started(RunId),
  Queued(Message),
  /// Running the task again with the message started nothing: no claim was taken and no attempt counted.
  NotReady(NotStarted),
  AwaitsLimit,
}

impl Sent {
  /// A refused start as the queued message it left, for a place that shows one line of why a message waits.
  #[must_use]
  pub fn as_queued(self) -> Self {
    match self {
      Self::NotReady(refused) => Self::Queued(refused.short()),
      other => other,
    }
  }
}

/// What a start that runs the task again asks before it queues, claims or starts anything: whether the task's pipeline
/// is ready on this computer, read with the vault that holds the secrets' values.
pub type Readiness<'a> = (&'a Workspace, &'a Vault);

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

  async fn go_on_with(self, runners: &Runners, command: Command) -> Result<Sent, RunRequestError> {
    self.control(runners, command).await.map(|()| Sent::Taken)
  }

  async fn continue_run(self, runners: &Runners, run: RunId) -> Result<Sent, RunRequestError> {
    let continued = match self {
      Self::Displayed => runners.continue_run(run).await,
      Self::Project(project) => runners.continue_run_in(project, run).await,
    };
    continued.map(|()| Sent::Taken)
  }

  async fn run_again(
    self,
    runners: &Runners,
    stored: &Stored<'_>,
    revision: Option<TaskRevision>,
  ) -> Result<Sent, RunRequestError> {
    let started = match self {
      Self::Displayed => {
        runners
          .run_again_at(stored.key, (stored.task, stored.run, revision), None)
          .await
      }
      Self::Project(project) => {
        runners
          .run_again_in(project, stored.key, (stored.task, stored.run, revision))
          .await
      }
    };
    started.map(Sent::Started)
  }
}

/// What follows `stored` as `then` says, through `runner`, for a sender that saw no revision of the task
/// (automation): Run again runs the task as it is now.
pub async fn follow(
  runner: (&Runners, &Servers, RunnerOf<'_>),
  ready: Readiness<'_>,
  stored: &Stored<'_>,
  then: Then,
) -> Sent {
  follow_seen(runner, ready, stored, (then, None)).await
}

/// [`follow`] for a person who wrote while the page showed the task at `revision`: Run again is refused once
/// the task changed since. Run again starts a run, so it asks first whether the pipeline is ready here: one that is not
/// starts nothing, takes no claim and counts no attempt, and the message stays queued ([`Sent::NotReady`]).
pub async fn follow_seen(
  (runners, servers, runner): (&Runners, &Servers, RunnerOf<'_>),
  (workspace, vault): Readiness<'_>,
  stored: &Stored<'_>,
  (then, revision): (Then, Option<TaskRevision>),
) -> Sent {
  let claimed = (servers, stored.root, stored.task);
  match then {
    Then::Deliver => tell((runners, servers, runner), (stored.root, stored.task, stored.run)).await,
    Then::RaiseLimit => Sent::AwaitsLimit,
    Then::Resume => {
      let resume = || runner.go_on_with(runners, Command::ResumeRun { run: stored.run });
      go_on(claimed, resume, control_message).await
    }
    Then::ContinueInterrupted => go_on(claimed, || runner.continue_run(runners, stored.run), control_message).await,
    Then::RunAgain => {
      if let Err(refused) = workspace.ready_to_start(stored.root, vault, stored.task).await {
        return Sent::NotReady(refused);
      }
      let refused = |error: &RunRequestError| {
        request_messages(error, stored.pipeline)
          .into_iter()
          .next()
          .unwrap_or_else(|| control_message(error))
      };
      go_on(claimed, || runner.run_again(runners, stored, revision), refused).await
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
  F: Future<Output = Result<Sent, RunRequestError>>,
{
  let claim = match servers.claim_for_work(root, task).await {
    Ok(claim) => claim,
    Err(refusal) => return Sent::Queued(refusal.message()),
  };
  match request().await {
    Ok(sent) => sent,
    Err(error) => {
      tracing::warn!(%error, "the run did not go on with the message");
      claim.release(servers).await;
      Sent::Queued(refused(&error))
    }
  }
}

/// [`follow`] for a message to a team task's run. The run works under the claim the team server gave its work when it
/// started (`team_work`), so no claim of the old sharing stream is taken, which would also cache the team task's
/// definition here. Delivering and resuming go to the runner as they do elsewhere. Continuing an interrupted run and
/// running the task again would start work without the team server's claim, so they stay queued with why, as does a
/// message that would resume a run the team server fenced.
pub async fn follow_team(
  (runners, runner): (&Runners, RunnerOf<'_>),
  stored: &Stored<'_>,
  (then, fenced): (Then, bool),
) -> Sent {
  let command = match then {
    Then::RaiseLimit => return Sent::AwaitsLimit,
    Then::ContinueInterrupted | Then::RunAgain => {
      return Sent::Queued(
        Message::new("appMcp.errors.teamProjectUnsupported").with("tool", "send_message".to_owned()),
      );
    }
    Then::Resume if fenced => return Sent::Queued(Message::new("claims.refused.fenced")),
    Then::Deliver => Command::DeliverMessages { run: stored.run },
    Then::Resume => Command::ResumeRun { run: stored.run },
  };
  match runner.control(runners, command).await {
    Ok(()) => Sent::Taken,
    // A blocked run keeps the message; the runner delivers it once the blocking tasks finish.
    Err(RunRequestError::Rejected(Rejection::RunAgainBlocked {
      block: RunAgainBlock::TaskBlocked,
    })) => Sent::Queued(Message::new("tasks.blocked.messageKept")),
    Err(error) => {
      tracing::warn!(%error, "the runner did not take the message");
      Sent::Queued(control_message(&error))
    }
  }
}
