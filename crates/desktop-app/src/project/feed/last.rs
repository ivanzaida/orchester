//! What Review's screens read last, kept in the project's feed so they show it again at once when they open again.

/// The merges Review › Merges read last, with how many pages of merged runs they hold.
#[derive(Clone, Debug, PartialEq)]
pub struct LastMerges {
  pub merges: orchester_workspace::tasks::merge_attempts::merges::Merges,
  pub pages: usize,
}

impl lurq::app::component::DevtoolsInspectable for LastMerges {}

/// The counts the Review hub's tabs read last, each `None` until it is first counted and `Some(None)` while it shows
/// none: its read failed, or there is nothing it counts (pull requests of a project not linked to a host). The Merge
/// queue counts the tasks the feed keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReviewCounts {
  /// The open pull requests the stored list holds.
  pub pull_requests: Option<Option<usize>>,
  /// The runs that need the person in Merges.
  pub needing_you: Option<Option<usize>>,
}

impl lurq::app::component::DevtoolsInspectable for ReviewCounts {}
