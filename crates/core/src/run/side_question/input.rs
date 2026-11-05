//! What the side agent reads: the task and run it is asked about, the run's transcript as lines, its changes as a
//! unified diff, the questions asked before in the pane with their answers, and the question. A long transcript keeps
//! its latest entries and a long diff its start, each with a note of what was left out, so a prompt never outgrows
//! what one request takes. Nothing here is a secret by construction: the transcript is kept redacted, and the runner
//! masks the project's secret values in all of it before the agent reads it ([`SideInput::masked`]).

use serde::{Deserialize, Serialize};

use super::SideQuestionText;

/// The most characters of the transcript the side agent reads: its latest entries.
pub const TRANSCRIPT_MAX_CHARS: usize = 60_000;
/// The most characters of one transcript entry (a long command output); the rest of it is cut.
const ENTRY_MAX_CHARS: usize = 4_000;
/// The most characters of the diff the side agent reads: its start.
pub const DIFF_MAX_CHARS: usize = 40_000;
/// The earlier questions of the pane the side agent reads, the latest ones, and the most characters of each answer.
const EARLIER_MAX: usize = 4;
const EARLIER_ANSWER_MAX_CHARS: usize = 2_000;
/// What marks text that was cut.
const CUT: &str = "\u{2026} (cut)";

/// The task and run a side question is about, and the agent at work in it, if one is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideAbout {
  pub task_key: String,
  pub title: String,
  pub run: u32,
  pub agent: Option<String>,
}

/// A question asked before in the pane, with its answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exchange {
  pub question: String,
  pub answer: String,
}

/// What one side question is answered from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideInput {
  about: SideAbout,
  question: String,
  /// The latest entries of the transcript, in order, and how many before them were left out.
  transcript: Vec<String>,
  left_out: usize,
  /// The start of the diff, and how many characters the whole of it has.
  diff: String,
  diff_chars: usize,
  earlier: Vec<Exchange>,
}

impl SideInput {
  /// `question` about `about`, answered from the transcript's `entries` and the run's `diff`, after `earlier`
  /// questions; trimmed to what one request takes.
  pub fn new(
    about: SideAbout,
    question: &SideQuestionText,
    (entries, diff): (&[String], &str),
    earlier: &[Exchange],
  ) -> Self {
    let (transcript, left_out) = latest_entries(entries);
    let diff_chars = diff.chars().count();
    let earlier = earlier[earlier.len().saturating_sub(EARLIER_MAX)..]
      .iter()
      .map(|exchange| Exchange {
        question: exchange.question.clone(),
        answer: cut(&exchange.answer, EARLIER_ANSWER_MAX_CHARS),
      })
      .collect();
    Self {
      about,
      question: question.as_str().to_owned(),
      transcript,
      left_out,
      diff: diff.chars().take(DIFF_MAX_CHARS).collect(),
      diff_chars,
      earlier,
    }
  }

  /// This input with `mask` applied to every text in it: the runner masks the project's secret values so.
  #[must_use]
  pub fn masked(self, mask: &dyn Fn(&str) -> String) -> Self {
    Self {
      about: SideAbout {
        task_key: self.about.task_key,
        title: mask(&self.about.title),
        run: self.about.run,
        agent: self.about.agent,
      },
      question: mask(&self.question),
      transcript: self.transcript.iter().map(|entry| mask(entry)).collect(),
      left_out: self.left_out,
      diff: mask(&self.diff),
      diff_chars: self.diff_chars,
      earlier: self
        .earlier
        .iter()
        .map(|exchange| Exchange {
          question: mask(&exchange.question),
          answer: mask(&exchange.answer),
        })
        .collect(),
    }
  }

  pub const fn about(&self) -> &SideAbout {
    &self.about
  }

  pub fn question(&self) -> &str {
    &self.question
  }

  pub fn transcript(&self) -> &[String] {
    &self.transcript
  }

  pub fn diff(&self) -> &str {
    &self.diff
  }

  pub fn earlier(&self) -> &[Exchange] {
    &self.earlier
  }

  /// How many of the transcript's earliest entries were left out.
  pub const fn left_out(&self) -> usize {
    self.left_out
  }

  /// Whether the diff was cut.
  pub fn diff_cut(&self) -> bool {
    self.diff_chars > self.diff.chars().count()
  }

  /// What was left out, as the side agent is told it; none when nothing was.
  pub fn notes(&self) -> Vec<String> {
    let mut notes = Vec::new();
    if self.left_out > 0 {
      notes.push(format!(
        "The transcript is long: its {} earliest entries were left out; you read its latest {}.",
        self.left_out,
        self.transcript.len()
      ));
    }
    if self.diff_cut() {
      notes.push(format!(
        "The diff is long: you read its first {} of {} characters; the files after that were left out.",
        self.diff.chars().count(),
        self.diff_chars
      ));
    }
    notes
  }
}

/// The latest of `entries` that fit in [`TRANSCRIPT_MAX_CHARS`], each cut to [`ENTRY_MAX_CHARS`], and how many
/// before them were left out.
fn latest_entries(entries: &[String]) -> (Vec<String>, usize) {
  let mut kept = Vec::new();
  let mut chars = 0;
  for entry in entries.iter().rev() {
    let entry = cut(entry, ENTRY_MAX_CHARS);
    let length = entry.chars().count();
    if chars + length > TRANSCRIPT_MAX_CHARS {
      break;
    }
    chars += length;
    kept.push(entry);
  }
  kept.reverse();
  let left_out = entries.len() - kept.len();
  (kept, left_out)
}

/// `text` cut to `max` characters, marked when it was.
fn cut(text: &str, max: usize) -> String {
  if text.chars().count() <= max {
    return text.to_owned();
  }
  let mut cut: String = text.chars().take(max.saturating_sub(CUT.chars().count())).collect();
  cut.push_str(CUT);
  cut
}
