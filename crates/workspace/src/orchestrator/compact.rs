//! *Compact* of an Orchestrator conversation's session (ORC-309, `docs/design/context-meter.md`): the project's runner
//! has the CLI summarize the session and go on from the summary. The runner keeps it as an event of the conversation
//! while it runs and once it ended; what the person reads when it could not start is here.

use orchester_core::id::ChatId;
use orchester_ipc::{ChatTools, Rejection};

use crate::{
  messages::Message,
  runner::{RequestError, RunRequestError, Runners},
  workspace::OpenProject,
};

/// Asks `project`'s runner to compact the session of `chat`, its CLI given the domain tools at `tools`.
pub async fn compact(runners: &Runners, project: &OpenProject, chat: ChatId, tools: ChatTools) -> Result<(), Message> {
  if let Err(error) = runners.chat_runner(project).await {
    return Err(not_compacted(&RunRequestError::Request(error)));
  }
  runners
    .compact_chat(project, chat, tools)
    .await
    .map_err(|error| not_compacted(&error))
}

/// The app could not serve the conversation's tools, so nothing was compacted.
pub fn tools_unavailable() -> Message {
  Message::new("orchestrator.compact.toolsUnavailable")
}

/// What the person reads when the runner did not compact.
fn not_compacted(error: &RunRequestError) -> Message {
  let key = match error {
    RunRequestError::Request(RequestError::NotConnected | RequestError::Lost) => {
      "orchestrator.compact.runnerUnavailable"
    }
    RunRequestError::Rejected(Rejection::NothingToCompact) => "orchestrator.compact.nothing",
    RunRequestError::Rejected(Rejection::ChatBusy) => "orchestrator.compact.busy",
    RunRequestError::Rejected(Rejection::NoOrchestrator) => "orchestrator.compact.notReady",
    RunRequestError::Rejected(Rejection::DailyBudgetUsed { .. }) => "orchestrator.compact.dailyBudgetUsed",
    RunRequestError::Rejected(Rejection::ShuttingDown) => "orchestrator.compact.shuttingDown",
    RunRequestError::Rejected(_) => "orchestrator.compact.failed",
  };
  Message::new(key)
}
