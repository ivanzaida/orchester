//! The conversation's size and its compaction (§6.3), and the segment a resumed conversation continues
//! (§2.2, §6.2).

use std::collections::{BTreeSet, HashSet};

use super::{
  Continuation, LoopAction, LoopEnd, LoopError, LoopFailure, LoopSettings, MAX_COMPACTIONS, NOT_RUN, NativeLoop, Phase,
};
use crate::{
  connection::stream::AgentEvent,
  native::{
    item::{Item, ToolCallId, UserOrigin},
    model::ModelResponse,
    tools::ToolName,
  },
  run::TokenUsage,
};

/// What the model is asked for before a compaction.
pub const COMPACTION_REQUEST: &str = "The conversation is about to be shortened to fit the context window. Write a \
                                      summary from which you can continue the step alone. Keep the task and its \
                                      acceptance criteria; the decisions made, the files changed and why; the \
                                      commands run and their outcomes; the current plan and the next action; open \
                                      problems; and the exact identifiers you will need. Answer with the summary \
                                      only and call no tool.";

/// Share of the context window, in tenths, at which the conversation is compacted.
const THRESHOLD_TENTHS: u64 = 8;

/// How many tokens the next request is estimated to take: the previous response's input as the provider
/// counted it, plus the items appended since at four bytes per token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ContextEstimate {
  last_input: Option<u64>,
  bytes_since: usize,
  /// The provider said the context is full.
  forced: bool,
}

impl ContextEstimate {
  /// An estimate for a segment no response has counted yet.
  pub(super) fn fresh(settings: &LoopSettings, items: &[Item]) -> Self {
    let tools: usize = settings
      .tools
      .tools()
      .iter()
      .map(|tool| tool.input_schema().len())
      .sum();
    let mut estimate = Self {
      last_input: None,
      bytes_since: settings.system.len() + tools,
      forced: false,
    };
    estimate.appended(items);
    estimate
  }

  pub(super) fn appended(&mut self, items: &[Item]) {
    self.bytes_since += items.iter().map(Item::estimated_bytes).sum::<usize>();
  }

  pub(super) const fn responded(&mut self, usage: TokenUsage) {
    self.last_input = Some(usage.input);
    self.bytes_since = 0;
  }

  pub(super) const fn force(&mut self) {
    self.forced = true;
  }

  pub(super) fn needs_compaction(&self, settings: &LoopSettings) -> bool {
    let appended = u64::try_from(self.bytes_since.div_ceil(4)).unwrap_or(u64::MAX);
    let tokens = self.last_input.unwrap_or(0).saturating_add(appended);
    self.forced || tokens.saturating_mul(10) >= settings.context_window.saturating_mul(THRESHOLD_TENTHS)
  }
}

impl NativeLoop {
  /// Asks the model for a summary of the segment, with the same system prompt and tools so that the
  /// provider's cache still applies.
  pub(super) fn compact(&mut self, actions: &mut Vec<LoopAction>) {
    if self.compactions >= MAX_COMPACTIONS {
      return self.end(LoopEnd::Failed(LoopFailure::ContextExhausted), actions);
    }
    self.phase = Phase::Compacting;
    actions.push(LoopAction::Emit(vec![AgentEvent::TurnStarted]));
    let request = self.request(vec![Item::user(COMPACTION_REQUEST, UserOrigin::Compaction)]);
    actions.push(LoopAction::CallModel(request));
  }

  /// Starts a new segment: the task, the summary, and a note naming the files changed so far.
  pub(super) fn on_summary(&mut self, response: &ModelResponse, actions: &mut Vec<LoopAction>) {
    let completed = LoopAction::Emit(vec![AgentEvent::TurnCompleted { usage: response.usage }]);
    let summary = response
      .items
      .iter()
      .filter_map(|item| match item {
        Item::Assistant { text } => Some(text.trim()),
        _ => None,
      })
      .filter(|text| !text.is_empty())
      .collect::<Vec<_>>()
      .join("\n\n");
    if summary.is_empty() {
      actions.push(completed);
      return self.end(LoopEnd::Failed(LoopFailure::NoSummary), actions);
    }
    self.compactions += 1;
    self.items = vec![self.task.clone()];
    self.context = ContextEstimate::fresh(&self.settings, &self.items);
    let note = Item::user(compaction_note(&self.changed), UserOrigin::Compaction);
    self.append(vec![Item::Compaction { summary }, note], actions);
    actions.push(completed);
    self.next_request(actions);
  }
}

/// What follows the summary: earlier turns are gone, so files must be read again before they are edited.
fn compaction_note(changed: &BTreeSet<String>) -> String {
  let files = if changed.is_empty() {
    "You have changed no files so far.".to_owned()
  } else {
    let list: Vec<&str> = changed.iter().map(String::as_str).collect();
    format!("Files you changed so far: {}.", list.join(", "))
  };
  format!("Earlier turns were replaced by the summary above. {files} Read a file again before you edit or replace it.")
}

/// The task and the items of the latest segment of a stored conversation.
pub(super) fn latest_segment(stored: Vec<Item>) -> Result<(Item, Vec<Item>), LoopError> {
  let task = match stored.first() {
    Some(
      task @ Item::User {
        origin: UserOrigin::Task,
        ..
      },
    ) => task.clone(),
    _ => return Err(LoopError::NoTask),
  };
  let Some(start) = stored.iter().rposition(|item| matches!(item, Item::Compaction { .. })) else {
    return Ok((task, stored));
  };
  let mut items = vec![task.clone()];
  items.extend(stored.into_iter().skip(start));
  Ok((task, items))
}

/// The items a resumed segment gets before its next request: a result for every call left unanswered,
/// then the continuation.
pub(super) fn answer_unanswered(items: &[Item], continuation: Continuation) -> Vec<Item> {
  let answered: HashSet<&ToolCallId> = items
    .iter()
    .filter_map(|item| match item {
      Item::ToolResult { id, .. } => Some(id),
      _ => None,
    })
    .collect();
  let pending: Vec<(&ToolCallId, &str)> = items
    .iter()
    .filter_map(|item| match item {
      Item::ToolCall { id, name, .. } if !answered.contains(id) => Some((id, name.as_str())),
      _ => None,
    })
    .collect();
  let question = pending
    .iter()
    .rev()
    .find(|(_, name)| *name == ToolName::AskQuestion.as_str())
    .map(|(id, _)| *id);
  let (mut answer, prompt) = match continuation {
    Continuation::Answer(text) => (Some(text), None),
    Continuation::Prompt(text) => (None, Some(text)),
  };
  let mut appended: Vec<Item> = pending
    .iter()
    .map(|(id, _)| match answer.take_if(|_| Some(*id) == question) {
      Some(text) => Item::tool_result((*id).clone(), text, false),
      None => Item::tool_result((*id).clone(), NOT_RUN, true),
    })
    .collect();
  if let Some(text) = answer {
    appended.push(Item::user(text, UserOrigin::Answer));
  }
  if let Some(text) = prompt {
    appended.push(Item::user(text, UserOrigin::Resume));
  }
  appended
}
