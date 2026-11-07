//! A message that runs the task again starts a run, so it asks first whether the task's pipeline is ready on this
//! computer (`not_ready_here`): one that is not starts nothing, takes no claim and counts no attempt, and the message
//! stays queued with why. The task's page lists what is missing when Run asks again.

use orchester_core::sharing::TaskRevision;
use orchester_workspace::{
  runner::Runners,
  sharing::Servers,
  tasks::conversation::{
    composer::Then,
    follow::{RunnerOf, Sent, Stored, follow_seen},
  },
};

use crate::services::Services;

/// [`follow_seen`], once a Run again's pipeline is known to be ready here.
pub async fn follow(
  services: &Services,
  runner: (&Runners, &Servers, RunnerOf<'_>),
  stored: &Stored<'_>,
  (then, revision): (Then, Option<TaskRevision>),
) -> Sent {
  if then == Then::RunAgain
    && let Err(refused) = services
      .workspace
      .ready_to_start(stored.root, &services.vault, stored.task)
      .await
  {
    return Sent::Queued(refused.short());
  }
  follow_seen(runner, stored, (then, revision)).await
}
