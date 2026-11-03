//! An Orchestrator conversation as Markdown: its title, the date, the conversation with how many messages it holds
//! and when, then the person's messages and the Orchestrator's answers in the order they were kept, each file
//! attached to a message by its name. With tool calls, each action the Orchestrator took through Orchester's tools
//! too: the tool, the task it named and how it went.

use orchester_core::{
  memory::SecretScreen,
  orchestrator::{ActionOutcome, ChatAction, EntryKind},
  time::Timestamp,
};

use super::{Include, Words, document::Document, stamp};
use crate::{messages::Message, orchestrator::chat::ChatView};

/// What the dialog says the export covers: how many messages, from when to when.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatSpan {
  pub messages: usize,
  /// The first and the last message's times; `None` while it holds none.
  pub span: Option<(Timestamp, Timestamp)>,
}

/// The messages of `view` and their times.
pub fn span(view: &ChatView) -> ChatSpan {
  let times: Vec<Timestamp> = view
    .entries
    .iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Person { .. } | EntryKind::Reply { .. }))
    .map(|entry| entry.at)
    .collect();
  ChatSpan {
    messages: times.len(),
    span: times.first().copied().zip(times.last().copied()),
  }
}

/// Conversation `view` as Markdown with what `include` says, exported at `now`, its words in `words` and the values
/// of the project's secrets masked by `screen`.
pub fn markdown(
  view: &ChatView,
  include: Include,
  (words, screen): (Words<'_>, &SecretScreen),
  now: Timestamp,
) -> String {
  let title = view.chat.title();
  let mut file = Document::new(title, words);
  file.fact("export.markdown.date", &stamp(now));
  let started = stamp(view.chat.started());
  let conversation = file.say(
    &Message::new("export.markdown.conversationOf")
      .with("title", title.to_owned())
      .with("started", started),
  );
  file.fact("export.markdown.conversation", &conversation);
  file.rule();
  let you = file.say(&Message::new("export.markdown.you"));
  let orchestrator = file.say(&Message::new("export.markdown.orchestrator"));
  for entry in &view.entries {
    match &entry.kind {
      EntryKind::Person { message, text, .. } => {
        let names: Vec<&str> = view
          .attachments_of(*message)
          .iter()
          .map(|attachment| attachment.name().as_str())
          .collect();
        file.end_tools();
        file.message(&you, entry.at, text.as_str(), &names);
      }
      EntryKind::Reply { text, .. } => {
        file.end_tools();
        file.message(&orchestrator, entry.at, text, &[]);
      }
      EntryKind::Action(action) if include == Include::WithTools => {
        let (what, result) = action_words(&file, action);
        file.tool(&what, &action.tool, &result);
      }
      _ => {}
    }
  }
  screen.mask(&file.finish())
}

/// What an action says of itself: a tool call, on the task it names, and how it went.
fn action_words(file: &Document<'_>, action: &ChatAction) -> (String, String) {
  let outcome = match &action.outcome {
    ActionOutcome::Done => Message::new("export.markdown.outcome.done"),
    ActionOutcome::Refused { reason, .. } => {
      Message::new("export.markdown.outcome.refused").with("reason", reason.clone())
    }
    ActionOutcome::Proposed(_) | ActionOutcome::Waits => Message::new("export.markdown.outcome.proposed"),
    ActionOutcome::Applied => Message::new("export.markdown.outcome.applied"),
    ActionOutcome::Dismissed => Message::new("export.markdown.outcome.dismissed"),
  };
  let what = match &action.task {
    Some(task) => Message::new("export.markdown.toolCallOn").with("task", task.clone()),
    None => Message::new("export.markdown.toolCall"),
  };
  (file.say(&what), file.say(&outcome))
}
