//! `long-chats`: the `conversation` project with chats as long as a person's real ones, for profiling how they open
//! (ORC-71): two Orchestrator conversations of 300 turns each, every turn a short question answered with a long
//! Markdown reply (a heading, links, code, a list, a table and a code block) and every sixth turn with a card, a
//! release of new starts waiting for the person or a task the chat created, about 50 cards a conversation; and
//! KON-12's run, its transcript 1,500 entries longer: messages, reasoning, commands and file edits. The newer
//! conversation is the one the Orchestrator opens on; History holds the older one.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind},
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{
    ActionOutcome, Chat, ChatAction, TurnState,
    change::{Change, ProposedChange},
  },
  resource::ModelId,
  store::{ActivityStore, RunStore},
  time::Timestamp,
};

use super::conversation;
use crate::kontur::{Kontur, SeedError};

/// Turns in each Orchestrator conversation.
const TURNS: i64 = 300;
/// Every this many turns, the answer carries a card.
const CARD_EVERY: i64 = 6;
/// Rounds added to KON-12's transcript, each a message, a thought, a command and an edit.
const ROUNDS: i64 = 300;

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  conversation::seed(kontur)?;
  long_transcript(kontur)?;
  chat(kontur, "Older", 24 * 60)?;
  chat(kontur, "Newer", 5)?;
  Ok(())
}

/// An Orchestrator conversation of [`TURNS`] turns tagged `tag`, its newest `ended` minutes ago.
fn chat(kontur: &mut Kontur, tag: &str, ended: i64) -> Result<(), SeedError> {
  let model = ModelId::new("claude-sonnet-4-5").map_err(|error| SeedError::Invalid(error.to_string()))?;
  let chat = ChatId::generate();
  for turn in 0..TURNS {
    let at = kontur.ago(ended + TURNS - turn);
    let message = ChatMessageId::generate();
    let body = MessageBody::new(&format!("{tag} {turn}: how do the runs stand?"))
      .map_err(|error| SeedError::Invalid(error.to_string()))?;
    kontur
      .db
      .keep_chat_message(&Chat::start(chat, &body, at), (message, &body), at)?;
    kontur
      .db
      .add_chat_reply(chat, (&reply(&format!("{tag} {turn}")), &model), at)?;
    if turn % CARD_EVERY == 0 {
      kontur.db.add_chat_action(chat, &card(turn), at)?;
    }
    kontur.db.end_chat_turn(message, &TurnState::Answered, at)?;
  }
  Ok(())
}

/// A long reply in Markdown, tagged `tag`.
fn reply(tag: &str) -> String {
  format!(
    "## {tag}: where the work stands\n\n\
     The review of **KON-12** failed twice on the retry test; the agent fixed the `backoff` helper and the \
     [pipeline run](https://example.invalid/runs/12) is green again. Two tasks wait for a decision.\n\n\
     - KON-14 waits for the person to approve its plan, since the plan widens its file access.\n\
     - KON-15 is queued behind the hold on new starts, which the person set this morning.\n\
     - KON-16 merged; its branch was removed and the base moved twice since.\n\n\
     | Task | State | Next |\n| --- | --- | --- |\n| KON-12 | Running | Review |\n| KON-14 | Waiting | Approve plan |\n\
     | KON-15 | Queued | Release starts |\n\n\
     ```rust\nfn backoff(attempt: u32) -> Duration {{\n  Duration::from_millis(250 * 2u64.pow(attempt.min(6)))\n}}\n```\n\n\
     {tag} closing note: the next check runs when the base moves again, and the summary lands in this conversation."
  )
}

/// The card of turn `turn`'s answer: a release of new starts waiting for the person, or a task the chat created.
fn card(turn: i64) -> ChatAction {
  if turn % (2 * CARD_EVERY) == 0 {
    return ChatAction {
      change: Some(Box::new(ProposedChange {
        subject: String::new(),
        change: Change::Release { task: None },
      })),
      ..ChatAction::new("release_queue", None, ActionOutcome::Waits)
    };
  }
  ChatAction::new("create_task", Some("KON-15".to_owned()), ActionOutcome::Done)
}

/// [`ROUNDS`] more rounds in the turn of KON-12's Builder, which still works: a message, a thought, a command and
/// an edit each, after what the `conversation` scenario left.
fn long_transcript(kontur: &mut Kontur) -> Result<(), SeedError> {
  let task = kontur.find("KON-12")?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .ok_or(SeedError::NotAtWork)?;
  let invocation = kontur
    .db
    .invocations(run.id())?
    .into_iter()
    .rev()
    .find(|invocation| invocation.ended_at.is_none())
    .ok_or(SeedError::NotAtWork)?;
  let start = kontur.now.unix_millis() - ROUNDS * 5 * 1_000;
  let at = |round: i64, offset: i64| Timestamp::from_unix_millis(start + round * 5 * 1_000 + offset * 1_000);
  let events: Vec<(Timestamp, AgentEvent)> = (0..ROUNDS)
    .flat_map(|round| {
      let item = format!("long-{round}");
      let command = format!("cargo test -p webhooks case_{round}");
      [
        (
          at(round, 0),
          AgentEvent::Message {
            text: format!("Round {round}: the jitter bound holds for attempt {round}; checking the next case."),
          },
        ),
        (
          at(round, 1),
          AgentEvent::Reasoning {
            text: format!("Case {round} retries a 503 with Retry-After; the delay must stay under the budget."),
          },
        ),
        (
          at(round, 2),
          AgentEvent::CommandStarted {
            item: item.clone(),
            command: command.clone(),
          },
        ),
        (
          at(round, 3),
          AgentEvent::CommandFinished {
            item,
            command,
            output: format!("test webhooks::retry::case_{round} ... ok\ntest result: ok. 1 passed"),
            exit_code: Some(0),
            status: CommandStatus::Completed,
          },
        ),
        (
          at(round, 4),
          AgentEvent::FileChanged {
            item: format!("edit-long-{round}"),
            changes: vec![FileChange {
              path: format!("tests/webhooks/case_{round}.rs"),
              kind: FileChangeKind::Added,
            }],
          },
        ),
      ]
    })
    .collect();
  kontur.db.append_activity(&invocation, &events)?;
  Ok(())
}
