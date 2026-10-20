use super::{
  RouteError,
  query::{Params, with_query},
};

const FILE: &str = "file";
const FROM: &str = "from";

/// A tab of a task's page, in the page's order (ORC-51): Details first, the one a task opens on, then
/// Conversation, Changes, Local integration while it is shown, and Summary last. The design (`Page tabs`
/// `L9OIIZ` in `lUXff`, `vShIt`) still draws Summary first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskTab {
  #[default]
  Details,
  Conversation,
  Changes,
  LocalIntegration,
  Summary,
}

impl TaskTab {
  pub const ALL: [Self; 5] = [
    Self::Details,
    Self::Conversation,
    Self::Changes,
    Self::LocalIntegration,
    Self::Summary,
  ];

  /// The tab's name: the last segment of its path, and its name in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Summary => "summary",
      Self::Details => "details",
      Self::Conversation => "conversation",
      Self::Changes => "changes",
      Self::LocalIntegration => "local-integration",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// i18n key of the tab's name.
  pub const fn title_key(self) -> &'static str {
    match self {
      Self::Summary => "task.tabs.summary",
      Self::Details => "task.tabs.details",
      Self::Conversation => "task.tabs.conversation",
      Self::Changes => "task.tabs.changes",
      Self::LocalIntegration => "local_integration.title",
    }
  }
}

/// What a task's page shows: a tab, and on Changes the file chosen in its list, if one is. The default is the
/// tab a task opens on, Details.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskView {
  #[default]
  Details,
  Conversation,
  Changes(Option<ChangedFile>),
  LocalIntegration,
  Summary,
}

impl TaskView {
  /// `tab`, on no particular file.
  pub const fn of(tab: TaskTab) -> Self {
    match tab {
      TaskTab::Summary => Self::Summary,
      TaskTab::Details => Self::Details,
      TaskTab::Conversation => Self::Conversation,
      TaskTab::Changes => Self::Changes(None),
      TaskTab::LocalIntegration => Self::LocalIntegration,
    }
  }

  pub const fn tab(&self) -> TaskTab {
    match self {
      Self::Summary => TaskTab::Summary,
      Self::Details => TaskTab::Details,
      Self::Conversation => TaskTab::Conversation,
      Self::Changes(_) => TaskTab::Changes,
      Self::LocalIntegration => TaskTab::LocalIntegration,
    }
  }

  pub const fn file(&self) -> Option<&ChangedFile> {
    match self {
      Self::Changes(file) => file.as_ref(),
      Self::Summary | Self::Details | Self::Conversation | Self::LocalIntegration => None,
    }
  }

  pub(super) fn path(&self, task: &str) -> String {
    let path = format!("{task}/{}", self.tab().name());
    match self.file() {
      Some(file) => {
        let from = (file.source != ChangeSource::default()).then(|| file.source.name());
        with_query(path, &[(FILE, Some(&file.path)), (FROM, from)])
      }
      None => path,
    }
  }

  /// The view the segment after a task's path names, its tab, with its file from the query on Changes. A path
  /// without the tab, as links are written by hand (`/tasks/ORC-24`), is the tab a task opens on, Details; the
  /// app writes the tab.
  pub(super) fn parse(segments: &[&str], params: &mut Params) -> Result<Self, RouteError> {
    let name = match segments {
      [] => return Ok(Self::default()),
      [name] => name,
      _ => return Err(RouteError::NoScreen),
    };
    let tab = TaskTab::from_name(name).ok_or(RouteError::NoScreen)?;
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
