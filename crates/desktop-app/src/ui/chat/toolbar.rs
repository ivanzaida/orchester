//! What a message's actions offer besides Copy ([`super::copy`]; ORC-330, ORC-706, `06 · Message actions` `XPDAX`):
//! on the person's own message *Edit* (`pencil`) before Copy, and *Rewind to here* (`history`) and *Fork from here*
//! (`git-fork`, ORC-334) after it; an answer offers the last two after its Copy. Each is an icon button as Copy is,
//! its label its tooltip and its name. One that cannot be used now has its icon at half and says why in its tooltip
//! (*Stop the answer to edit or rewind*); *Fork from here* works while the Orchestrator answers, and Copy always does.

use std::sync::Arc;

use lurq::app::component::DevtoolsInspectable;

use crate::ui::icon::Icon;

/// The class of an action while it cannot be pressed, for tests and QA.
pub const HELD_CLASS: &str = "toolbar-held";

/// What a choice does, or why it cannot be made now.
pub type Choice = Result<Arc<dyn Fn() + Send + Sync>, Arc<str>>;

/// What a message's actions hold besides Copy.
#[derive(Clone, PartialEq)]
pub struct MessageExtras {
  /// *Edit*, before Copy.
  pub edit: Option<ToolbarChoice>,
  /// *Rewind to here*, after Copy.
  pub rewind: Option<ToolbarChoice>,
  /// *Fork from here*, last.
  pub fork: Option<ToolbarChoice>,
}

impl MessageExtras {
  /// The choice `action` makes; `None` for Copy, which is the line's own, and for one the message does not offer.
  pub(super) const fn choice(&self, action: Action) -> Option<&ToolbarChoice> {
    match action {
      Action::Edit => self.edit.as_ref(),
      Action::Copy => None,
      Action::Rewind => self.rewind.as_ref(),
      Action::Fork => self.fork.as_ref(),
    }
  }
}

/// An action named `label` for its tooltip and for tools.
#[derive(Clone)]
pub struct ToolbarChoice {
  pub label: Arc<str>,
  pub choose: Choice,
}

/// Compared by what they show; what a choice does is its caller's.
impl PartialEq for ToolbarChoice {
  fn eq(&self, other: &Self) -> bool {
    self.label == other.label
      && match (&self.choose, &other.choose) {
        (Ok(_), Ok(_)) => true,
        (Err(one), Err(two)) => one == two,
        _ => false,
      }
  }
}

/// An action of a message, in the order its line shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
  Edit,
  Copy,
  Rewind,
  Fork,
}

impl DevtoolsInspectable for Action {}

impl Action {
  pub(super) const ALL: [Self; 4] = [Self::Edit, Self::Copy, Self::Rewind, Self::Fork];

  /// Its button's element id after the message's: `<id>-<key>`.
  pub(super) const fn key(self) -> &'static str {
    match self {
      Self::Edit => "edit",
      Self::Copy => "copy",
      Self::Rewind => "rewind",
      Self::Fork => "fork",
    }
  }

  /// Its glyph; Copy's says what came of its last press instead ([`super::copy`]).
  pub(super) const fn icon(self) -> Icon {
    match self {
      Self::Edit => Icon::Pencil,
      Self::Copy => Icon::Copy,
      Self::Rewind => Icon::History,
      Self::Fork => Icon::GitFork,
    }
  }

  /// Where it stands in [`Self::ALL`].
  pub(super) const fn index(self) -> usize {
    self as usize
  }
}
