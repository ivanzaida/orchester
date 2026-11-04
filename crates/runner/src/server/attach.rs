//! Attaching ([`Command::Attach`]) and what the desktop passes with it and again when it changes or later: the agent
//! slots ([`Command::SetAgentSlots`]), the computer's Settings › Browser ([`Command::SetBrowser`]), which the runner
//! never reads from the application database itself (`docs/architecture/remote-runners.md`), and the interrupted runs
//! to continue ([`Command::ContinueRuns`]).

use std::num::NonZeroU32;

use orchester_core::id::RunId;
#[cfg(doc)]
use orchester_ipc::Command;
use orchester_ipc::{AttachSession, Reply, browser::ComputerBrowser};

use crate::execution::Scheduler;

/// The reply to a [`Command::Attach`] with `session`, `agent_slots` and `browser`. The project's browser has the
/// Settings › Browser it carries before any run continues, so a run that uses it at once starts it with them.
pub(super) async fn attached(
  scheduler: &Scheduler,
  session: AttachSession,
  agent_slots: NonZeroU32,
  browser: ComputerBrowser,
) -> Reply {
  scheduler.browser().set_computer(browser);
  match scheduler.attach(session, agent_slots).await {
    Ok(attached) => Reply::Attached {
      active_runs: scheduler.active_runs(),
      interrupted: attached.interrupted,
      continued: attached.continued,
    },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`Command::SetAgentSlots`].
pub(super) async fn slots_set(scheduler: &Scheduler, agent_slots: NonZeroU32) -> Reply {
  match scheduler.set_agent_slots(agent_slots).await {
    Ok(()) => Reply::AgentSlotsSet { agent_slots },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`Command::SetBrowser`]: the project's browser starts with `browser` from its next start.
pub(super) fn browser_set(scheduler: &Scheduler, browser: ComputerBrowser) -> Reply {
  scheduler.browser().set_computer(browser);
  Reply::BrowserSet
}

/// The reply to [`Command::ContinueRuns`] of `runs`.
pub(super) async fn continued(scheduler: &Scheduler, runs: &[RunId]) -> Reply {
  let (continued, refused) = scheduler.continue_runs(runs).await;
  Reply::Continued { continued, refused }
}
