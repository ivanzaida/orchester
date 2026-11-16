//! Which *Remove worktree* confirmation of the task page is open.

use orchester_core::id::RunId;

/// Whose *Remove worktree* confirmation is open, and where: under the header when the header's button
/// opened it for the latest run, in the run's row of *Runs of this task* when the row's did. One at a
/// time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Removal {
  pub run: Option<RunId>,
  pub place: RemovalPlace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RemovalPlace {
  #[default]
  Header,
  Row,
}

impl lurq::app::component::DevtoolsInspectable for Removal {}

impl Removal {
  pub const fn closed() -> Self {
    Self {
      run: None,
      place: RemovalPlace::Header,
    }
  }

  /// Open for `run` at `place`.
  pub const fn of(run: RunId, place: RemovalPlace) -> Self {
    Self { run: Some(run), place }
  }

  /// Open for `run` at `place` now.
  pub fn shows(self, run: RunId, place: RemovalPlace) -> bool {
    self.run == Some(run) && self.place == place
  }
}
