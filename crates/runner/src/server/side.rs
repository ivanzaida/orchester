//! The side question commands (ORC-337): an answer is told once the side agent ended, without holding up the runner's
//! other requests; a stop at once.

use orchester_ipc::{Command, Event, Reply};

use super::{Answer, Later};
use crate::execution::Scheduler;

/// Routes [`Command::AnswerSideQuestion`] and [`Command::StopSideQuestion`].
pub(super) fn answer_side(later: &Later<'_>, handle: Scheduler, command: &Command) -> Answer {
  match *command {
    Command::AnswerSideQuestion { question, ref input } => {
      let input = (**input).clone();
      later.reply(async move {
        let outcome = handle.answer_side_question(question, input).await;
        Reply::SideAnswered { question, outcome }
      })
    }
    Command::StopSideQuestion { question } => {
      handle.stop_side_question(question);
      let reply = Reply::SideStopped { question };
      Answer::Reply(Event::Reply { id: later.id, reply })
    }
    _ => unreachable!("answer_side routes only the two side question commands"),
  }
}
