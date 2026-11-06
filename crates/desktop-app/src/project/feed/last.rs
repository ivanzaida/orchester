//! What Review's screens read last, kept in the project's feed so they show it again at once when they open again.

/// The merges Review › Merges read last, with how many pages of merged runs they hold.
#[derive(Clone, Debug, PartialEq)]
pub struct LastMerges {
  pub merges: orchester_workspace::tasks::merge_attempts::merges::Merges,
  pub pages: usize,
}

impl lurq::app::component::DevtoolsInspectable for LastMerges {}
