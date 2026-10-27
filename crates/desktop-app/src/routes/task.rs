use super::{
  RouteError,
  query::{Params, with_query},
};

const FILE: &str = "file";
const FROM: &str = "from";
/// The view a tab opens in its place, in the query of the tab's path: the page keeps its frame, as it does when its
/// tabs switch.
const VIEW: &str = "view";
/// The task's memory, under its Summary (`/tasks/KON-12/summary?view=memory`): the Summary's properties link to it.
const MEMORY: &str = "memory";
/// Local integration, under Changes (`/tasks/KON-12/changes?view=local-integration`): *Try locally* opens it.
const LOCAL_INTEGRATION: &str = "local-integration";

/// A tab of a task's page, in the page's order (ORC-100, `Tabs` `QFr7g` in `Task header` `j8DakG`): Summary, the
/// one a task opens on, then Conversation and Changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskTab {
  #[default]
  Summary,
  Conversation,
  Changes,
}

impl TaskTab {
  pub const ALL: [Self; 3] = [Self::Summary, Self::Conversation, Self::Changes];

  /// The tab's name: its segment in a path, and its name in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Summary => "summary",
      Self::Conversation => "conversation",
      Self::Changes => "changes",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// i18n key of the tab's name.
  pub const fn title_key(self) -> &'static str {
    match self {
      Self::Summary => "task.tabs.summary",
      Self::Conversation => "task.tabs.conversation",
      Self::Changes => "task.tabs.changes",
    }
  }
}

/// What a task's page shows: a tab, on Changes the file chosen in its list, if one is, and the two views a tab
/// opens in its place: the task's memory from the Summary, Local integration from Changes. The default is the tab
/// a task opens on, Summary.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskView {
  #[default]
  Summary,
  /// The task's memory, which the Summary's properties link to; under the Summary's tab.
  Memory,
  Conversation,
  Changes(Option<ChangedFile>),
  /// Local integration, which *Try locally* on Changes opens; under the Changes tab.
  LocalIntegration,
}

impl TaskView {
  /// The views a tab opens in its place, by their names in a path and in the app's MCP tools.
  pub const SUB_VIEWS: [&'static str; 2] = [MEMORY, LOCAL_INTEGRATION];

  /// `tab`, on no particular file.
  pub const fn of(tab: TaskTab) -> Self {
    match tab {
      TaskTab::Summary => Self::Summary,
      TaskTab::Conversation => Self::Conversation,
      TaskTab::Changes => Self::Changes(None),
    }
  }

  /// The view `name` (one of [`Self::SUB_VIEWS`]) of `tab`; `None` when the tab has no such view.
  pub fn sub_view(tab: TaskTab, name: &str) -> Option<Self> {
    match (tab, name) {
      (TaskTab::Summary, MEMORY) => Some(Self::Memory),
      (TaskTab::Changes, LOCAL_INTEGRATION) => Some(Self::LocalIntegration),
      _ => None,
    }
  }

  /// The tab the view is under, which the page's tabs show as chosen.
  pub const fn tab(&self) -> TaskTab {
    match self {
      Self::Summary | Self::Memory => TaskTab::Summary,
      Self::Conversation => TaskTab::Conversation,
      Self::Changes(_) | Self::LocalIntegration => TaskTab::Changes,
    }
  }

  /// The name of the view a tab opens in its place, while it shows one.
  pub const fn sub_name(&self) -> Option<&'static str> {
    match self {
      Self::Memory => Some(MEMORY),
      Self::LocalIntegration => Some(LOCAL_INTEGRATION),
      Self::Summary | Self::Conversation | Self::Changes(_) => None,
    }
  }

  /// The same view on no particular file: what the page's frame shows.
  #[must_use]
  pub const fn without_file(&self) -> Self {
    match self {
      Self::Summary => Self::Summary,
      Self::Memory => Self::Memory,
      Self::Conversation => Self::Conversation,
      Self::Changes(_) => Self::Changes(None),
      Self::LocalIntegration => Self::LocalIntegration,
    }
  }

  pub const fn file(&self) -> Option<&ChangedFile> {
    match self {
      Self::Changes(file) => file.as_ref(),
      Self::Summary | Self::Memory | Self::Conversation | Self::LocalIntegration => None,
    }
  }

  pub(super) fn path(&self, task: &str) -> String {
    let path = format!("{task}/{}", self.tab().name());
    if let Some(sub) = self.sub_name() {
      return with_query(path, &[(VIEW, Some(sub))]);
    }
    match self.file() {
      Some(file) => {
        let from = (file.source != ChangeSource::default()).then(|| file.source.name());
        with_query(path, &[(FILE, Some(&file.path)), (FROM, from)])
      }
      None => path,
    }
  }

  /// The view the segment after a task's path names, its tab, with the view the tab opens in its place, or its file
  /// on Changes, from the query. A path without the tab, as links are written by hand (`/tasks/ORC-24`), is the tab a
  /// task opens on, Summary; the app writes the tab. The tabs a task had before ORC-100 still read, as links in
  /// messages and notifications name them: Details and Artifacts are on the Summary now, Memory and Local
  /// integration views of their tabs.
  pub(super) fn parse(segments: &[&str], params: &mut Params) -> Result<Self, RouteError> {
    let name = match segments {
      [] => return Ok(Self::default()),
      [name] => *name,
      _ => return Err(RouteError::NoScreen),
    };
    let Some(tab) = TaskTab::from_name(name) else {
      return match name {
        "details" | "artifacts" => Ok(Self::Summary),
        MEMORY => Ok(Self::Memory),
        LOCAL_INTEGRATION => Ok(Self::LocalIntegration),
        _ => Err(RouteError::NoScreen),
      };
    };
    if let Some(view) = params.take(VIEW)? {
      return Self::sub_view(tab, &view).ok_or(RouteError::InvalidParam {
        param: VIEW.to_owned(),
        value: view,
      });
    }
    if tab != TaskTab::Changes {
      return Ok(Self::of(tab));
    }
    let Some(path) = params.take(FILE)? else {
      return Ok(Self::Changes(None));
    };
    let from = params.take_or_default(FROM, ChangeSource::from_name)?;
    ChangedFile::new(path, from).map(|file| Self::Changes(Some(file)))
  }
}

/// Where a changed file's change is read from: the run's latest checkpoint (`CHECKPOINTED`) or its
/// worktree after it (`NOT CHECKPOINTED`, in `DYun5`). A file can be in both lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChangeSource {
  #[default]
  Checkpoint,
  Worktree,
}

impl ChangeSource {
  pub const ALL: [Self; 2] = [Self::Checkpoint, Self::Worktree];

  /// The name in a path's query and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Checkpoint => "checkpoint",
      Self::Worktree => "worktree",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|source| source.name() == name)
  }
}

/// A file the Changes tab shows: its repository path as the run's changes list it (never empty) and where
/// its change is read from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChangedFile {
  path: String,
  source: ChangeSource,
}

impl ChangedFile {
  /// The file at `path`; an empty path names no file.
  pub fn new(path: impl Into<String>, source: ChangeSource) -> Result<Self, RouteError> {
    let path = path.into();
    if path.is_empty() {
      return Err(RouteError::InvalidParam {
        param: FILE.to_owned(),
        value: path,
      });
    }
    Ok(Self { path, source })
  }

  pub fn path(&self) -> &str {
    &self.path
  }

  pub const fn source(&self) -> ChangeSource {
    self.source
  }
}
