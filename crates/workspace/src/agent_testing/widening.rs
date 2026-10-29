//! What a change to a service lets runs start: a service runs its program with the person's rights, network access
//! and no sandbox, in a run's worktree, so another program, other arguments, another working folder, other variables
//! or another seed are a person's decision (`docs/architecture/agent-testing.md`, "approval"). Its readiness, needs,
//! run data, UI driver and owned paths start nothing new.

use orchester_core::service::Service;

use crate::messages::Message;

/// A new service, which runs its program whenever a run starts it.
pub const NEW_SERVICE: &str = "appMcp.services.widens.newService";
/// A service's first version, which runs can start from then on.
pub const FIRST_VERSION: &str = "appMcp.services.widens.firstVersion";

/// What `after` runs that `before` did not, each as a reason a person decides on; none when it runs the same.
pub fn changes(before: &Service, after: &Service) -> Vec<Message> {
  let mut reasons = Vec::new();
  if before.program() != after.program() {
    reasons.push(
      Message::new("appMcp.services.widens.program")
        .with("program", after.program().as_str().to_owned())
        .with("before", before.program().as_str().to_owned()),
    );
  }
  if before.arguments() != after.arguments() {
    reasons.push(Message::new("appMcp.services.widens.arguments"));
  }
  if before.working_folder() != after.working_folder() {
    reasons.push(Message::new("appMcp.services.widens.workingFolder"));
  }
  if before.variables() != after.variables() {
    reasons.push(Message::new("appMcp.services.widens.variables"));
  }
  if before.seed() != after.seed() {
    reasons.push(Message::new("appMcp.services.widens.seed"));
  }
  reasons
}
