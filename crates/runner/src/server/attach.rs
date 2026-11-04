//! Attaching ([`Command::Attach`](orchester_ipc::Command::Attach)) and the computer's Settings › Browser the desktop
//! passes with it and again when they change ([`Command::SetBrowser`](orchester_ipc::Command::SetBrowser)): the
//! runner never reads them from the application database itself (`docs/architecture/remote-runners.md`).

use orchester_ipc::{Command, Reply, browser::ComputerBrowser};

use crate::execution::Scheduler;

/// The reply to `attach`, a [`Command::Attach`]. The project's browser has the Settings › Browser it carries before any
/// run continues, so a run that uses it at once starts it with them.
pub(super) async fn attached(scheduler: &Scheduler, attach: &Command) -> Reply {
  let Command::Attach {
    session,
    agent_slots,
    ref browser,
  } = *attach
  else {
    unreachable!("answer routes only Attach here");
  };
  scheduler.browser().set_computer(browser.clone());
  match scheduler.attach(session, agent_slots).await {
    Ok(attached) => Reply::Attached {
      active_runs: scheduler.active_runs(),
      interrupted: attached.interrupted,
      continued: attached.continued,
    },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to `Command::SetBrowser`: the project's browser starts with `browser` from its next start.
pub(super) fn browser_set(scheduler: &Scheduler, browser: ComputerBrowser) -> Reply {
  scheduler.browser().set_computer(browser);
  Reply::BrowserSet
}
