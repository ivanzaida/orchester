//! A handoff record as its row shows it: the summary, then *Completed*, *Decisions*, *Open questions* and *Next
//! agent should know* as label and text lines. The record's text is core's ([`orchester_core::memory::Handoff`]):
//! the summary, then each list that has items under its English heading.

use orchester_core::memory::split_handoff;

/// One list of a handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandoffList {
  Completed,
  Decisions,
  OpenQuestions,
  NextAgentShouldKnow,
}

impl HandoffList {
  pub const ALL: [Self; 4] = [
    Self::Completed,
    Self::Decisions,
    Self::OpenQuestions,
    Self::NextAgentShouldKnow,
  ];

  /// The heading core writes the list under in the record's text.
  const fn heading(self) -> &'static str {
    match self {
      Self::Completed => "Completed",
      Self::Decisions => "Decisions",
      Self::OpenQuestions => "Open questions",
      Self::NextAgentShouldKnow => "The next agent should know",
    }
  }
}

/// A handoff's text split into its summary and its lists, in the order the text has them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffParts {
  pub summary: String,
  pub lists: Vec<(HandoffList, Vec<String>)>,
}

impl HandoffParts {
  pub fn of(text: &str) -> Self {
    let (summary, lists) = split_handoff(text);
    let mut parts = Self {
      summary: summary.to_owned(),
      lists: Vec::new(),
    };
    for paragraph in lists.split("\n\n") {
      let mut lines = paragraph.lines();
      let heading = lines.next().unwrap_or_default().strip_suffix(':');
      let Some(list) = HandoffList::ALL
        .into_iter()
        .find(|list| heading == Some(list.heading()))
      else {
        continue;
      };
      let items = lines
        .map(|line| line.strip_prefix("- ").unwrap_or(line).trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect();
      parts.lists.push((list, items));
    }
    parts
  }
}
