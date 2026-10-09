/// A tab of the Overview's task list (`Tabs` `f3VTo` in `q9mf0k`); Running is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OverviewTab {
  #[default]
  Running,
  NeedsYou,
  Queued,
  Failed,
}

impl OverviewTab {
  pub const ALL: [Self; 4] = [Self::Running, Self::NeedsYou, Self::Queued, Self::Failed];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Running => "running",
      Self::NeedsYou => "needs_you",
      Self::Queued => "queued",
      Self::Failed => "failed",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}
