use lurq::layout::layout_kind::ScrollState;

/// Where the task page's scrolled panes are: kept by the page, so a tab that goes inactive and
/// back keeps them.
pub(crate) struct TaskScrolls {
  /// The Summary's notice and progress (`ZVdRa`).
  pub(super) progress: ScrollState,
  /// This run's configuration (`F5Lsi`).
  pub(super) this_run: ScrollState,
  /// The Summary of a run that cannot be read (02.R).
  pub(super) unreadable: ScrollState,
}
