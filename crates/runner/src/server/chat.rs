//! The Orchestrator chat's requests ([`Command::AnswerInChat`], [`Command::StopChat`], [`Command::SteerChat`]),
//! answered at once.

use orchester_ipc::{Command, Reply};

use crate::execution::Scheduler;

/// The reply to chat `command`, one of the three above.
pub(super) async fn reply(scheduler: &Scheduler, command: &Command) -> Reply {
  let done = match *command {
    Command::AnswerInChat { message, ref tools } => scheduler
      .answer_in_chat(message, tools.clone())
      .await
      .map(|()| Reply::ChatAnswering { message }),
    Command::StopChat { chat } => scheduler.stop_chat(chat).map(|()| Reply::ChatStopped { chat }),
    Command::SteerChat { message } => scheduler.steer_chat(message).map(|()| Reply::ChatSteered { message }),
    _ => unreachable!("the server routes only the three chat commands here"),
  };
  done.unwrap_or_else(Reply::Rejected)
}
