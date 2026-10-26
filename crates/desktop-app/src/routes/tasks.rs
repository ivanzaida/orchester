use orchester_core::project::TaskKey;

use super::{
  RouteError, TAB,
  query::{Params, with_query},
};

const SEARCH: &str = "q";
const HIGHLIGHT: &str = "highlight";
/// Shows the tasks kept *Not shared* when the project joined, in place of the list's (`A7PKfL`).
const SHOW: &str = "show";
const NOT_SHARED: &str = "not-shared";

/// A view of the Tasks list (`Views` in `Tasks · redesign` `Lw1Gb`, ORC-99): Active (Todo, In progress, In review and
/// what waits for the person), Backlog, Done (Done and Canceled) or All. Active is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskListTab {
  #[default]
  Active,
  Backlog,
  Done,
  All,
}

impl TaskListTab {
  pub const ALL: [Self; 4] = [Self::Active, Self::Backlog, Self::Done, Self::All];

  /// The view's name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Active => "active",
      Self::Backlog => "backlog",
      Self::Done => "done",
      Self::All => "all",
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
  /// The *Not shared* filter is on.
  not_shared: bool,
}

impl TaskList {
  /// The list on `tab`, without a search or a highlighted task.
  pub const fn new(tab: TaskListTab) -> Self {
    Self {
      tab,
      search: None,
      highlight: None,
      not_shared: false,
    }
  }

  /// The list with the *Not shared* filter on or off, its tab and search kept.
  #[must_use]
  pub const fn with_not_shared(mut self, on: bool) -> Self {
    self.not_shared = on;
    self
  }

  /// Whether the *Not shared* filter is on.
  pub const fn not_shared(&self) -> bool {
    self.not_shared
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
        (SHOW, self.not_shared.then_some(NOT_SHARED)),
      ],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let mut list = Self::new(params.take_or_default(TAB, TaskListTab::from_name)?);
    if let Some(text) = params.take(SEARCH)? {
      list = list.with_search(text);
    }
    if let Some(key) = params.take(HIGHLIGHT)? {
      list = list.with_highlight(key.parse::<TaskKey>().map_err(|_| RouteError::InvalidParam {
        param: HIGHLIGHT.to_owned(),
        value: key,
      })?);
    }
    match params.take(SHOW)?.as_deref() {
      None => {}
      Some(NOT_SHARED) => list.not_shared = true,
      Some(value) => {
        return Err(RouteError::InvalidParam {
          param: SHOW.to_owned(),
          value: value.to_owned(),
        });
      }
    }
    Ok(list)
  }
}
