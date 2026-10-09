use super::{
  RouteError,
  query::{Params, with_query},
};

const FILE: &str = "file";
const FROM: &str = "from";

/// A tab of a task's page (`Page tabs` `L9OIIZ` in `lUXff`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskTab {
  #[default]
  Conversation,
  Summary,
  Changes,
}

impl TaskTab {
  pub const ALL: [Self; 3] = [Self::Conversation, Self::Summary, Self::Changes];

  /// The name the app's MCP tools use for the tab.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Conversation => "conversation",
      Self::Summary => "summary",
      Self::Changes => "changes",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// i18n key of the tab's name.
  pub const fn title_key(self) -> &'static str {
    match self {
      Self::Conversation => "task.tabs.conversation",
      Self::Summary => "task.tabs.summary",
      Self::Changes => "task.tabs.changes",
    }
  }

  /// The segment after the task's path; the Conversation is the task's own path.
  pub(super) const fn segment(self) -> Option<&'static str> {
    match self {
      Self::Conversation => None,
      Self::Summary => Some("summary"),
      Self::Changes => Some("changes"),
    }
  }
}

/// What a task's page shows: a tab, and on Changes the file chosen in its list, if one is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskView {
  #[default]
  Conversation,
  Summary,
  Changes(Option<ChangedFile>),
}

impl TaskView {
  /// `tab`, on no particular file.
  pub const fn of(tab: TaskTab) -> Self {
    match tab {
      TaskTab::Conversation => Self::Conversation,
      TaskTab::Summary => Self::Summary,
      TaskTab::Changes => Self::Changes(None),
    }
  }

  pub const fn tab(&self) -> TaskTab {
    match self {
      Self::Conversation => TaskTab::Conversation,
      Self::Summary => TaskTab::Summary,
      Self::Changes(_) => TaskTab::Changes,
    }
  }

  pub const fn file(&self) -> Option<&ChangedFile> {
    match self {
      Self::Changes(file) => file.as_ref(),
      Self::Conversation | Self::Summary => None,
    }
  }

  pub(super) fn path(&self, task: String) -> String {
    let path = match self.tab().segment() {
      Some(segment) => format!("{task}/{segment}"),
      None => task,
    };
    match self.file() {
      Some(file) => {
        let from = (file.source != ChangeSource::default()).then(|| file.source.name());
        with_query(path, &[(FILE, Some(&file.path)), (FROM, from)])
      }
      None => path,
    }
  }

  /// The view the segments after a task's path name, with its file from the query on Changes.
  pub(super) fn parse(segments: &[&str], params: &mut Params) -> Result<Self, RouteError> {
    let tab = match segments {
      [] => TaskTab::Conversation,
      [segment] => TaskTab::ALL
        .into_iter()
        .find(|tab| tab.segment() == Some(*segment))
        .ok_or(RouteError::NoScreen)?,
      _ => return Err(RouteError::NoScreen),
    };
    if tab != TaskTab::Changes {
      return Ok(Self::of(tab));
    }
    let Some(path) = params.take(FILE) else {
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
