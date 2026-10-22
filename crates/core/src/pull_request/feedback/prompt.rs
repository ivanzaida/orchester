//! What the agent reads of a feedback post: a message of the task's conversation that names the pull request,
//! what the post asks (an explanation without code changes, or a fix on the task's branch), the file, side and
//! lines with the code they held, and the person's words. Agents read it as written, like the pull request body
//! `super::super::task_pull_request` writes; it is not shown as Orchester's own text.

use super::{DiffSide, FeedbackAnchor, FeedbackKind, FeedbackPost, FeedbackThread, LineSpan};

/// Characters of a revision the message names, as `git log --oneline` shows them.
const SHORT: usize = 7;

/// The message post `post` of `thread` is sent as. `moved`: where the thread's lines are at the post's revision,
/// when that is known; otherwise the message names them at the thread's own revision.
pub fn request_text(thread: &FeedbackThread, post: &FeedbackPost, moved: Option<LineSpan>) -> String {
  let asks = match post.kind() {
    FeedbackKind::Question => format!(
      "Question about pull request #{} (Orchester review feedback, not a GitHub comment). Answer it without \
       changing code.",
      thread.number()
    ),
    FeedbackKind::Fix => format!(
      "Fix request for pull request #{} (Orchester review feedback, not a GitHub comment). Change the code on \
       this task's branch, then say what you changed.",
      thread.number()
    ),
  };
  let about = match thread.anchor() {
    FeedbackAnchor::PullRequest => format!(
      "About: the whole pull request at {}.",
      &post.revision().as_str()[..SHORT]
    ),
    FeedbackAnchor::Lines(lines) => {
      let side = match lines.side() {
        DiffSide::Old => "old",
        DiffSide::New => "new",
      };
      let (span, revision) = match moved {
        Some(span) => (span, post.revision()),
        None => (lines.span(), thread.revision()),
      };
      let range = if span.len() == 1 {
        format!("line {}", span.start())
      } else {
        format!("lines {}-{}", span.start(), span.end())
      };
      format!(
        "About: {}, {side} {range} at {}:\n```\n{}\n```",
        lines.path(),
        &revision.as_str()[..SHORT],
        lines.excerpt().join("\n")
      )
    }
  };
  format!("{asks}\n{about}\n\n{}", post.body().as_str())
}
