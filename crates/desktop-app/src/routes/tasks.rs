use orchester_core::project::TaskKey;

use super::{
  RouteError, TAB,
  query::{Params, with_query},
};

const SEARCH: &str = "q";
const HIGHLIGHT: &str = "highlight";

/// A tab of the Tasks list (`Workflow tabs` `F3K93b` in `t3oAOi`): every task, the ones that need the
/// person, or one workflow status. All is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskListTab {
  #[default]
  All,
  NeedsYou,
  Backlog,
  Todo,
  InProgress,
  InReview,
  Done,
}

impl TaskListTab {
  pub const ALL: [Self; 7] = [
    Self::All,
    Self::NeedsYou,
    Self::Backlog,
    Self::Todo,
    Self::InProgress,
    Self::InReview,
    Self::Done,
  ];

  /// The tab's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::NeedsYou => "needs_you",
      Self::Backlog => "backlog",
      Self::Todo => "todo",
      Self::InProgress => "in_progress",
      Self::InReview => "in_review",
      Self::Done => "done",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}

/// What the Tasks list shows: its tab, the text searched for, and the task a Create just added, which
/// the list highlights (`Tasks · new task highlighted` `t3oAOi`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TaskList {
  tab: TaskListTab,
  /// Never empty: an empty search is no search.
  search: Option<String>,
  highlight: Option<TaskKey>,
}

impl TaskList {
  /// The list on `tab`, without a search or a highlighted task.
  pub const fn new(tab: TaskListTab) -> Self {
    Self {
      tab,
      search: None,
      highlight: None,
    }
  }

  /// The list searched for `text` as typed; an empty text searches for nothing.
  #[must_use]
  pub fn with_search(mut self, text: impl Into<String>) -> Self {
    let text = text.into();
    self.search = (!text.is_empty()).then_some(text);
    self
  }

  #[must_use]
  pub fn with_highlight(mut self, key: TaskKey) -> Self {
    self.highlight = Some(key);
    self
  }

  pub const fn tab(&self) -> TaskListTab {
    self.tab
  }

  pub fn search(&self) -> Option<&str> {
    self.search.as_deref()
  }

  pub const fn highlight(&self) -> Option<&TaskKey> {
    self.highlight.as_ref()
  }

  pub(super) fn path(&self, list: String) -> String {
    let tab = (self.tab != TaskListTab::default()).then(|| self.tab.name());
    let highlight = self.highlight.as_ref().map(ToString::to_string);
    with_query(
      list,
      &[
        (TAB, tab),
        (SEARCH, self.search.as_deref()),
        (HIGHLIGHT, highlight.as_deref()),
      ],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let mut list = Self::new(params.take_or_default(TAB, TaskListTab::from_name)?);
    if let Some(text) = params.take(SEARCH) {
      list = list.with_search(text);
    }
    if let Some(key) = params.take(HIGHLIGHT) {
      list = list.with_highlight(key.parse::<TaskKey>().map_err(|_| RouteError::InvalidParam {
        param: HIGHLIGHT.to_owned(),
        value: key,
      })?);
    }
    Ok(list)
  }
}
