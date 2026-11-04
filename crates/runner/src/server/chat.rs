//! The replies to the Orchestrator chat's requests: answer a message, stop the answer, compact the session (ORC-309)
//! and hand a message sent now to the answer under way.

use orchester_core::id::{ChatId, ChatMessageId};
use orchester_ipc::{ChatTools, Command, Reply};

use crate::execution::Scheduler;

/// The reply to chat `command`, one of the four above.
pub(super) async fn reply(scheduler: &Scheduler, command: &Command) -> Reply {
  match *command {
    Command::AnswerInChat { message, ref tools } => answering(scheduler, message, tools.clone()).await,
    Command::StopChat { chat } => stopped(scheduler, chat),
    Command::CompactChat { chat, ref tools } => compacting(scheduler, chat, tools.clone()).await,
    Command::SteerChat { message } => steered(scheduler, message),
    _ => unreachable!("the server routes only the four chat commands here"),
  }
}

/// The reply to [`orchester_ipc::Command::AnswerInChat`].
async fn answering(scheduler: &Scheduler, message: ChatMessageId, tools: ChatTools) -> Reply {
  match scheduler.answer_in_chat(message, tools).await {
    Ok(()) => Reply::ChatAnswering { message },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`orchester_ipc::Command::StopChat`].
fn stopped(scheduler: &Scheduler, chat: ChatId) -> Reply {
  match scheduler.stop_chat(chat) {
    Ok(()) => Reply::ChatStopped { chat },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`orchester_ipc::Command::CompactChat`].
async fn compacting(scheduler: &Scheduler, chat: ChatId, tools: ChatTools) -> Reply {
  match scheduler.compact_chat(chat, tools).await {
    Ok(()) => Reply::ChatCompacting { chat },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`orchester_ipc::Command::SteerChat`].
fn steered(scheduler: &Scheduler, message: ChatMessageId) -> Reply {
  match scheduler.steer_chat(message) {
    Ok(()) => Reply::ChatSteered { message },
    Err(rejection) => Reply::Rejected(rejection),
  }
}
