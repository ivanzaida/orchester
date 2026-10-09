//! What an agent CLI's call of one of Orchester's file tools asked, read from the call's input, so that its
//! transcript row reads as the native agent's row for the same tool (04.H10): what the call is about and, for
//! `edit_file` and `create_file`, the change ([`CallChange`]). The answer a CLI gets says less than the native
//! executors report (no lines read, no matches counted), so the row shows what the input asked; the call's end says
//! whether it happened.
//!
//! An edit's and a write's texts are kept whole ([`Requested`]) until the event is redacted, and only then made into
//! the change's lines, cut to size ([`CallInput::settle`]): a secret that spans lines, or that a cut would split, is
//! still found whole. The whole texts are never stored; an event stored without being settled shows no change.

use std::{fmt, sync::Arc};

use serde::{Deserialize, Serialize};

use super::call_change::CallChange;
use crate::native::tools::ToolInput;

/// What a CLI's call of a file tool asked.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallInput {
  target: String,
  /// Where a search looks: its path, else its glob; `None` for the whole worktree and for other tools.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  scope: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  change: Option<Arc<CallChange>>,
  /// The whole texts of an edit or a write, until [`Self::settle`] makes them the change.
  #[serde(skip)]
  requested: Option<Requested>,
}

/// The whole texts an edit or a write asked for.
#[derive(Clone, PartialEq, Eq)]
pub enum Requested {
  /// `edit_file`: its replacements, in order.
  Edits(Vec<EditText>),
  /// `create_file` (or the native agent's `write_file`): the file's whole new content.
  Content(String),
}

/// One replacement of an `edit_file` call.
#[derive(Clone, PartialEq, Eq)]
pub struct EditText {
  pub(super) old: String,
  pub(super) new: String,
  pub(super) replace_all: bool,
}

impl EditText {
  /// Replacing `old` with `new`, at every occurrence when `replace_all`.
  pub fn new(old: &str, new: &str, replace_all: bool) -> Self {
    Self {
      old: old.to_owned(),
      new: new.to_owned(),
      replace_all,
    }
  }
}

impl CallInput {
  /// What a call of a file tool with `input` asked; `None` for any other tool.
  pub fn of(input: &ToolInput) -> Option<Self> {
    let (target, requested) = match input {
      ToolInput::ReadFile(read) => (read.path().to_owned(), None),
      ToolInput::ListFiles(list) => {
        let target = [list.path(), list.pattern()]
          .into_iter()
          .flatten()
          .collect::<Vec<_>>()
          .join(" ");
        (if target.is_empty() { ".".to_owned() } else { target }, None)
      }
      ToolInput::Search(search) => {
        let scope = search.path().or(search.glob()).map(str::to_owned);
        return Some(Self {
          target: search.pattern().to_owned(),
          scope,
          change: None,
          requested: None,
        });
      }
      ToolInput::EditFile(edit) => {
        let edits = edit
          .edits()
          .iter()
          .map(|one| EditText::new(one.old(), one.new_text(), one.replace_all()))
          .collect();
        (edit.path().to_owned(), Some(Requested::Edits(edits)))
      }
      ToolInput::WriteFile(write) => (
        write.path().to_owned(),
        Some(Requested::Content(write.content().to_owned())),
      ),
      ToolInput::RunCommand(_) | ToolInput::FinishStep(_) | ToolInput::AskQuestion(_) => return None,
    };
    Some(Self {
      target,
      scope: None,
      change: None,
      requested,
    })
  }

  /// What a call about `target`, looking in `scope`, asked, with the whole texts of the change it `requested`.
  pub fn requested(target: &str, scope: Option<&str>, requested: Option<Requested>) -> Self {
    Self {
      target: target.to_owned(),
      scope: scope.map(str::to_owned),
      change: None,
      requested,
    }
  }

  /// What the call is about: the file it reads, edits or writes, the folder and pattern it lists, or the pattern
  /// it searches for.
  pub fn target(&self) -> &str {
    &self.target
  }

  /// Where a search looks, when it names a path or a glob.
  pub fn scope(&self) -> Option<&str> {
    self.scope.as_deref()
  }

  /// The change an `edit_file` or `create_file` call asked for, once settled.
  pub fn change(&self) -> Option<&Arc<CallChange>> {
    self.change.as_ref()
  }

  /// Makes the whole texts the change's lines, cut to size: after the event was redacted, before it is stored.
  pub(super) fn settle(&mut self) {
    if let Some(requested) = self.requested.take() {
      self.change = Some(Arc::new(CallChange::of(&requested)));
    }
  }

  /// Redacts the target and the whole texts, or the lines of a change already settled.
  pub(super) fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    self.target = redact(&self.target);
    if let Some(scope) = &mut self.scope {
      *scope = redact(scope);
    }
    match &mut self.requested {
      Some(Requested::Edits(edits)) => {
        for edit in edits {
          edit.old = redact(&edit.old);
          edit.new = redact(&edit.new);
        }
      }
      Some(Requested::Content(content)) => *content = redact(content),
      None => {}
    }
    if let Some(change) = &mut self.change {
      Arc::make_mut(change).redact(redact);
    }
  }
}

/// The target and the change; the whole texts only by their size.
impl fmt::Debug for CallInput {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CallInput")
      .field("target", &self.target)
      .field("scope", &self.scope)
      .field("change", &self.change)
      .field("requested", &self.requested)
      .finish()
  }
}

impl fmt::Debug for Requested {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Edits(edits) => write!(f, "Edits({} edits)", edits.len()),
      Self::Content(content) => write!(f, "Content({} bytes)", content.len()),
    }
  }
}

impl fmt::Debug for EditText {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "EditText({} → {} bytes)", self.old.len(), self.new.len())
  }
}
