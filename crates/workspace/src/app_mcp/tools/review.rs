//! `get_run`'s `review`: the person's review of the run (`orchester_core::review`): its threads, oldest first, each
//! with its kind, state and messages, the files marked *Viewed* at the run's head, the comments waiting unsent, and
//! whether Implement works on comments sent to it. A thread gives the file and lines it is about, never the code they
//! hold; Orchester's reason a question was not asked or a comment not sent is told in the app's words.

use std::path::Path;

use orchester_core::{
  id::RunId,
  review::{QuestionState, ReviewThread, Said, ThreadState},
};
use orchester_exec::programs::Programs;
use serde_json::{Value, json};

use super::{Translate, wire};
use crate::{
  app_mcp::error::ToolError,
  messages::Message,
  tasks::review_threads::{
    failure_message,
    read::{Review, review_of},
  },
};

/// A run's review as `get_run` gives it, but for the reasons, which are told in the app's words ([`Reviewed::told`]).
pub(super) struct Reviewed {
  read: Value,
  /// Each thread's reason, in thread order.
  reasons: Vec<Option<Message>>,
}

impl Reviewed {
  /// `run`'s review in the project at `root`, read with `programs`.
  pub(super) fn of_run(root: &Path, programs: &Programs, run: RunId) -> Result<Self, ToolError> {
    let review =
      review_of(root, programs, run).map_err(|refusal| ToolError::refused(refusal.code(), vec![refusal.message()]))?;
    Ok(Self::of(&review))
  }

  fn of(review: &Review) -> Self {
    let (threads, reasons): (Vec<Value>, Vec<Option<Message>>) = review
      .threads
      .iter()
      .map(|view| {
        (
          thread_json(&view.thread, &view.state),
          reason(&view.thread, &view.state),
        )
      })
      .unzip();
    let read = json!({
      "head": review.head.as_ref().map(ToString::to_string),
      "threads": threads,
      "viewed": review.viewed.iter().map(|viewed| viewed.path.clone()).collect::<Vec<_>>(),
      "unsent_comments": review.unsent(),
      "implement_at_work": review.implement_at_work,
    });
    Self { read, reasons }
  }

  /// The review, each reason in the app's words (`translate`).
  pub(super) fn told(mut self, translate: &Translate) -> Value {
    if let Some(threads) = self.read["threads"].as_array_mut() {
      for (thread, reason) in threads.iter_mut().zip(self.reasons) {
        thread["reason"] = json!(reason.map(|reason| translate.text(&reason)));
      }
    }
    self.read
  }
}

/// `thread` in state `state`, but its reason.
fn thread_json(thread: &ReviewThread, state: &ThreadState) -> Value {
  let anchor = thread.anchor();
  let commit = match state {
    ThreadState::Addressed { commit } => commit.as_ref().map(ToString::to_string),
    ThreadState::Outdated { commit } => Some(commit.to_string()),
    _ => None,
  };
  let messages: Vec<Value> = thread
    .messages()
    .iter()
    .map(|message| {
      let (by, writing, activity) = match message.said() {
        Said::Agent(words) => ("implement", words.writing, words.activity.clone()),
        _ => ("person", false, None),
      };
      json!({
        "id": message.id().to_string(),
        "by": by,
        "body": message.body(),
        "at": wire::time(message.at()),
        "writing": writing,
        "activity": activity,
      })
    })
    .collect();
  json!({
    "id": thread.id().to_string(),
    "kind": thread.kind().as_str(),
    "state": state.name(),
    "folded": state.folds(),
    "path": anchor.path(),
    "side": anchor.side().as_str(),
    "lines": { "start": anchor.span().start(), "end": anchor.span().end() },
    "head": thread.head().to_string(),
    "commit": commit,
    "created_at": wire::time(thread.created_at()),
    "resolved_at": thread.resolved_at().map(wire::time),
    "messages": messages,
  })
}

/// Orchester's reason the thread's question was not asked or its comment not sent, when it has one.
fn reason(thread: &ReviewThread, state: &ThreadState) -> Option<Message> {
  match state {
    ThreadState::NotAsked(failure) | ThreadState::CouldNotSend(failure) => Some(failure_message(failure)),
    // A folded thread keeps the reason its latest question was not asked.
    _ => match thread.latest_ask().said() {
      Said::Question(QuestionState::NotAsked { failure }) => Some(failure_message(failure)),
      _ => None,
    },
  }
}
