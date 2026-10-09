//! Questions with suggested answers in the timeline (C·15–C·17, "Questions with options"): a card in the
//! asking agent's message, waiting or answered. A question an agent waits on in its turn is a card of its
//! own at the time it was asked; one that ended a native agent's step stands in for that agent's question
//! message. The person's message a step-end answer reached its agent in is not shown again: the answered
//! card shows the answer.

use std::{collections::HashSet, sync::Arc};

use orchester_core::{
  id::{MessageId, StepExecutionId},
  question::{AskedQuestions, QuestionTurn},
};

use super::{Item, ItemKind};
use crate::tasks::conversation::{Entry, EntryState};

/// A request's card: its questions, where it stands, and who asked in which step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceCard {
  pub asked: AskedQuestions,
  /// The asking agent's role.
  pub agent: Arc<str>,
  pub step: Arc<str>,
}

impl ChoiceCard {
  /// Whether it waits for the person's answer.
  pub const fn waiting(&self) -> bool {
    self.asked.waits()
  }
}

/// The key of request `id`'s card.
pub fn card_key(id: &str) -> Arc<str> {
  format!("question-{id}").into()
}

/// The person's messages the answers of `questions` reached their agents in.
pub(super) fn answer_messages(questions: &[AskedQuestions]) -> HashSet<MessageId> {
  questions.iter().filter_map(|asked| asked.message).collect()
}

/// The cards of `execution`'s requests among `questions`, and the agent's question messages they stand
/// for among `entries`.
pub(super) fn cards(
  (questions, entries): (&[AskedQuestions], &[Entry]),
  execution: StepExecutionId,
  (agent, step): (&Arc<str>, &Arc<str>),
) -> (Vec<Item>, HashSet<MessageId>) {
  let mut replaced = HashSet::new();
  let items = questions
    .iter()
    .filter(|asked| asked.request.execution() == execution)
    .map(|asked| {
      let at = match asked.request.turn() {
        QuestionTurn::InTurn => asked.request.asked_at(),
        QuestionTurn::StepEnd => match asked_in(entries, asked) {
          Some(entry) => {
            replaced.insert(entry.id);
            entry.at
          }
          None => asked.request.asked_at(),
        },
      };
      Item {
        key: card_key(asked.request.id()),
        execution: Some(execution),
        at,
        kind: ItemKind::Choice(ChoiceCard {
          asked: asked.clone(),
          agent: agent.clone(),
          step: step.clone(),
        }),
      }
    })
    .collect();
  (items, replaced)
}

/// The agent's question message a step-end request ended its step with: the first of its execution
/// written when the request was asked or after.
fn asked_in<'a>(entries: &'a [Entry], asked: &AskedQuestions) -> Option<&'a Entry> {
  entries
    .iter()
    .filter(|entry| entry.execution == Some(asked.request.execution()))
    .filter(|entry| matches!(entry.state, EntryState::Question { .. }))
    .find(|entry| entry.at >= asked.request.asked_at())
}
