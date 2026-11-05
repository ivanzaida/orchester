//! What the implementing agent reads of a review: a question with the lines it is about and the thread so far, for a
//! read-only turn, and the one note *Send to Implement* returns the run's work with, quoting each comment's file, lines
//! and words. Agents read them as written; they are not shown as Orchester's own text.

use super::{ReviewMessage, ReviewThread, Said};
use crate::run::ReviewNote;

/// Characters of a revision the texts name, as `git log --oneline` shows them.
const SHORT: usize = 7;

/// The prompt of the read-only turn that answers `question`, the person's latest message of `thread`.
pub fn question_text(thread: &ReviewThread, question: &ReviewMessage) -> String {
  let mut text = format!(
    "A person reviewing your work on this run asks a question. Answer it without changing anything: this turn is \
     read-only, so write no file, make no commit and run nothing that changes the worktree.\n\n{}",
    about(thread)
  );
  let earlier: Vec<String> = thread
    .messages()
    .iter()
    .take_while(|message| message.id() != question.id())
    .filter(|message| !message.body().trim().is_empty())
    .map(|message| {
      let who = if message.said().is_person() { "Person" } else { "You" };
      format!("{who}: {}", message.body())
    })
    .collect();
  if !earlier.is_empty() {
    text.push_str("\n\nEarlier in this thread:\n");
    text.push_str(&earlier.join("\n\n"));
  }
  text.push_str("\n\nQuestion: ");
  text.push_str(question.body());
  text
}

/// The note that returns the run's work to its implementing step with `comments`, each the person's comment and its
/// thread, in the order given. `None` for no comment.
pub fn send_note(comments: &[(&ReviewThread, &ReviewMessage)]) -> Option<ReviewNote> {
  if comments.is_empty() {
    return None;
  }
  let intro = if comments.len() == 1 {
    "A person reviewed your work and left a comment. Change the code as it asks, commit, and say what you changed."
      .to_owned()
  } else {
    format!(
      "A person reviewed your work and left {} comments. Change the code as each asks, commit, and say for each what \
       you changed.",
      comments.len()
    )
  };
  let items: Vec<String> = comments
    .iter()
    .enumerate()
    .map(|(at, (thread, comment))| {
      let mut item = format!("{}. {}\n{}", at + 1, about(thread), comment.body());
      if let Some(context) = replied_to(thread, comment) {
        item.push_str(&format!("\n(It replies to your earlier words: {context})"));
      }
      item
    })
    .collect();
  ReviewNote::new(&format!("{intro}\n\n{}", items.join("\n\n"))).ok()
}

/// The file, lines and code `thread` is about, at its head.
fn about(thread: &ReviewThread) -> String {
  let anchor = thread.anchor();
  let span = anchor.span();
  let range = if span.len() == 1 {
    format!("line {}", span.start())
  } else {
    format!("lines {}-{}", span.start(), span.end())
  };
  format!(
    "{}, {range} at {}:\n```\n{}\n```",
    anchor.path(),
    &thread.head().as_str()[..SHORT],
    anchor.excerpt().join("\n")
  )
}

/// Implement's words just before `comment` in `thread`, when the comment replies to them.
fn replied_to<'a>(thread: &'a ReviewThread, comment: &ReviewMessage) -> Option<&'a str> {
  thread
    .messages()
    .iter()
    .take_while(|message| message.id() != comment.id())
    .last()
    .filter(|message| matches!(message.said(), Said::Agent(_)) && !message.body().trim().is_empty())
    .map(ReviewMessage::body)
}
