//! The replies to the Orchestrator chat's requests: answer a message, stop the answer, compact the session (ORC-309)
//! and hand a message sent now to the answer under way.

use orchester_core::id::{ChatId, ChatMessageId};
use orchester_ipc::{ChatTools, Reply};

use crate::execution::Scheduler;

/// The reply to [`orchester_ipc::Command::AnswerInChat`].
pub(super) async fn answering(scheduler: &Scheduler, message: ChatMessageId, tools: ChatTools) -> Reply {
  match scheduler.answer_in_chat(message, tools).await {
    Ok(()) => Reply::ChatAnswering { message },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`orchester_ipc::Command::StopChat`].
pub(super) fn stopped(scheduler: &Scheduler, chat: ChatId) -> Reply {
  match scheduler.stop_chat(chat) {
    Ok(()) => Reply::ChatStopped { chat },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`orchester_ipc::Command::CompactChat`].
pub(super) async fn compacting(scheduler: &Scheduler, chat: ChatId, tools: ChatTools) -> Reply {
  match scheduler.compact_chat(chat, tools).await {
    Ok(()) => Reply::ChatCompacting { chat },
    Err(rejection) => Reply::Rejected(rejection),
  }
}

/// The reply to [`orchester_ipc::Command::SteerChat`].
pub(super) fn steered(scheduler: &Scheduler, message: ChatMessageId) -> Reply {
  match scheduler.steer_chat(message) {
    Ok(()) => Reply::ChatSteered { message },
    Err(rejection) => Reply::Rejected(rejection),
  }
}
