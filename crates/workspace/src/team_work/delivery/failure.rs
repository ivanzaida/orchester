//! Why a team delivery's merge failed on this computer, kept with the delivery (`DeliveryMerge::Failed`) by its kind:
//! what the person can act on (the base moved and the merge conflicts, files of the checkout stand in the way) names
//! its files, anything else is git's own words. The team server is told only that it failed.

use orchester_core::integration::MergeError;
use serde::{Deserialize, Serialize};

use crate::messages::Message;

/// Why the merge of a module's branch into its base branch failed here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum MergeFailure {
  /// The base branch moved, and the merge conflicts in these files.
  Conflict { files: Vec<String> },
  /// These files of the person's checkout, uncommitted or untracked, would be overwritten.
  InTheWay { files: Vec<String> },
  /// Git did not merge it for another reason.
  Other { detail: String },
}

impl MergeFailure {
  pub(super) fn of(error: &MergeError) -> Self {
    match error {
      MergeError::Conflict { files } => Self::Conflict { files: files.clone() },
      MergeError::UncommittedInTheWay { files } => Self::InTheWay { files: files.clone() },
      other => Self::Other {
        detail: other.to_string(),
      },
    }
  }

  /// As a delivery keeps it.
  pub(super) fn kept(&self) -> String {
    serde_json::to_string(self).expect("a failure of plain texts serializes")
  }

  /// The failure a delivery kept as `text`; one kept before failures had a kind is the words it kept.
  pub fn read(text: &str) -> Self {
    serde_json::from_str(text).unwrap_or_else(|_| Self::Other {
      detail: text.to_owned(),
    })
  }

  /// What the person reads of it, in the words a local module's delivery uses.
  pub fn message(&self) -> Message {
    match self {
      Self::Conflict { files } => Message::new("modules.delivery.conflicts").with("files", files.join(", ")),
      Self::InTheWay { files } => Message::new("modules.delivery.inTheWay").with("files", files.join(", ")),
      Self::Other { detail } => Message::new("modules.delivery.failed").with("detail", detail.clone()),
    }
  }
}
