//! The Changes tab of a task's page (`Task · Changes · dark` `DYun5`): what the latest run changed. The
//! toolbar names the run's base and its latest checkpoint; the file list (`qT0kd`) holds the files of the
//! run's diff from its base to that checkpoint, then what its worktree holds beyond it, listed apart as
//! not checkpointed; beside it, the selected file's unified diff with line numbers and syntax colors
//! (`RsPIH`). The file shown is part of the path (`?file=`, `&from=worktree`), and choosing another takes
//! the current entry's place in the tab's history.
//!
//! The task page frame takes it with three lines: [`ChangesReads::new`] in its `create`;
//! [`ChangesReads::read`] with [`Follow::of`] on every render, whose [`RunChanges::file_count`] it shows on
//! the tab's label through [`tab_label`], so the count is of the files listed here; and [`ChangesTab`]
//! with [`ChangesProps`] in its Changes slot. Until it does, `host` stands in for it.

mod diff;
mod files;
mod host;
mod listed;
mod load;
mod states;
mod toolbar;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{Column, Row},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::ProjectId, tokens::grouped as group_digits};
use orchester_workspace::{
  display::short_revision,
  messages::Message,
  tasks::{
    changes::{Checkpointed, NotCheckpointed, ReadChanges},
    detail::{RunView, TaskDetail},
  },
};
use parking_lot::Mutex;

use self::{
  diff::{DiffView, DiffViewProps},
  files::{FILE_LIST_WIDTH, FileItem, FileList, FileListProps, FileSummary, WorktreeNote},
  listed::{FileListing, ListKeys, Listed},
  toolbar::toolbar,
};
pub use self::{
  host::screen,
  load::{ChangesReads, CheckpointedRead, Follow, RunChanges, tab_label},
};
use crate::{
  navigation::Navigation,
  routes::{ChangeSource, ChangedFile, ProjectPage, Route, TaskView},
};

/// What the Changes tab shows.
#[derive(Clone)]
pub struct ChangesProps {
  pub project: ProjectId,
  pub detail: Arc<TaskDetail>,
  /// The latest run's changes as read; `None` without a run.
  pub changes: Option<RunChanges>,
  /// The file the path names.
  pub file: Option<ChangedFile>,
  /// An agent works on the run: the worktree is read again while it does.
  pub agent_at_work: bool,
  pub reads: ChangesReads,
}

impl PartialEq for ChangesProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.detail == other.detail
      && self.changes == other.changes
      && self.file == other.file
      && self.agent_at_work == other.agent_at_work
  }
}

impl lurq::app::component::DevtoolsInspectable for ChangesProps {}

/// The file list's items as last made, and what they were made of.
type ItemsKey = (ListKeys, usize, Option<WorktreeNote>);
type KeptItems = Option<(ItemsKey, Arc<Vec<FileItem>>)>;

pub struct ChangesTab {
  navigation: Navigation,
  /// The list of the diffs on screen and its items, kept while they stay, so the list sees the same
  /// items.
  listing: Arc<Mutex<Option<FileListing>>>,
  items: Arc<Mutex<KeptItems>>,
}

impl Component for ChangesTab {
  type Props = ChangesProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      navigation: Navigation::of(ctx),
      listing: Arc::new(Mutex::new(None)),
      items: Arc::new(Mutex::new(None)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChangesProps>().clone();
    let column = Column::new().id("task-changes").width(Dimension::Pct(100.0)).flex(1.0);
    let (Some(run), Some(changes)) = (props.detail.run.as_ref(), props.changes.as_ref()) else {
      return column.child(states::no_run(ctx));
    };
    let checkpointed = match &changes.checkpointed {
      CheckpointedRead::Loading => return column.child(states::reading(ctx)),
      CheckpointedRead::Failed(error) => {
        let reads = props.reads.clone();
        return column.child(states::failed(ctx, error, move || reads.retry()));
      }
      CheckpointedRead::Ready(checkpointed) => checkpointed,
    };
    let column = column.child(toolbar(ctx, run, checkpointed));
    let committed = match checkpointed {
      Checkpointed::TooLarge { size, .. } => {
        return column.child(states::too_large(ctx, *size, run.run.branch().as_str()));
      }
      Checkpointed::Read { changes, .. } => Some(changes.clone()),
      Checkpointed::NoCheckpoint => None,
    };
    let (uncommitted, notice) = worktree(ctx, &props, changes.worktree.as_ref());
    let column = column.with_children(notice);
    let listing = self.listing(committed.as_deref(), uncommitted.as_deref());
    if listing.is_empty() {
      // A card says the run changed nothing only when its worktree was read clean; otherwise the notice
      // over the files says what is not known.
      let empty = match (checkpointed, &changes.worktree) {
        (_, None) => Some(states::reading(ctx)),
        (Checkpointed::NoCheckpoint, Some(Ok(NotCheckpointed::Clean))) => {
          Some(states::no_checkpoint(ctx, run.run.number().get()))
        }
        (_, Some(Ok(NotCheckpointed::Clean))) => Some(states::no_changes(ctx)),
        _ => None,
      };
      return column.with_children(empty);
    }
    let shown = Shown {
      checkpointed,
      committed: committed.as_deref(),
      uncommitted: uncommitted.as_deref(),
    };
    column.child(self.body(ctx, &props, run, &listing, &shown))
  }
}

/// The diffs the files come from.
struct Shown<'a> {
  checkpointed: &'a Checkpointed,
  committed: Option<&'a ReadChanges>,
  uncommitted: Option<&'a ReadChanges>,
}

impl Shown<'_> {
  fn of(&self, source: ChangeSource) -> &ReadChanges {
    match source {
      ChangeSource::Checkpoint => self.committed,
      ChangeSource::Worktree => self.uncommitted,
    }
    .expect("a listed file comes from a diff that was read")
  }
}

impl ChangesTab {
  /// The list of `committed` and `uncommitted` changes: the same list while the diffs stay.
  fn listing(&self, committed: Option<&ReadChanges>, uncommitted: Option<&ReadChanges>) -> FileListing {
    let keys = (
      committed.map(|changes| changes.key.clone()),
      uncommitted.map(|changes| changes.key.clone()),
    );
    let mut listing = self.listing.lock();
    if let Some(kept) = listing.as_ref()
      && kept.keys == keys
    {
      return kept.clone();
    }
    let made = FileListing::of(committed, uncommitted);
    *listing = Some(made.clone());
    made
  }

  /// The file list beside the selected file's diff.
  fn body(
    &self,
    ctx: &mut Ctx,
    props: &ChangesProps,
    run: &RunView,
    listing: &FileListing,
    shown: &Shown<'_>,
  ) -> Element {
    let position = listing.position(props.file.as_ref());
    let selected = listing.files[position].clone();
    let note = shown.uncommitted.map(|changes| WorktreeNote {
      revision: short_revision(changes.from.as_str()).into(),
      live: props.agent_at_work.then(|| {
        let agent = run.current_step().and_then(|step| step.agent.clone());
        (agent, props.reads.last_read())
      }),
    });
    let items = self.items(listing, shown, (position, note));
    let select = self.select(props);
    let list = Column::new()
      .width(Dimension::Px(FILE_LIST_WIDTH))
      .height(Dimension::Pct(100.0))
      .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(ctx.mount_keyed::<FileList>("changes-file-list", FileListProps { items, select }));
    let diff = shown.of(selected.source);
    let source = match selected.source {
      ChangeSource::Checkpoint => ctx.t_args(
        "changes.diff.checkpointedAt",
        [("revision", short_revision(head_of(shown.checkpointed)))],
      ),
      ChangeSource::Worktree => ctx.t_args(
        "changes.diff.notCheckpointed",
        [("revision", short_revision(diff.from.as_str()))],
      ),
    };
    let view = ctx.mount_keyed::<DiffView>(
      "changes-diff-view",
      DiffViewProps {
        diff: diff.diff.clone(),
        key: diff.key.clone(),
        index: selected.index,
        file: format!("{}:{}", selected.source.name(), selected.path).into(),
        source,
      },
    );
    Row::new()
      .id("changes-body")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(list)
      .child(view)
      .into()
  }

  /// The file list's items: each group's heading and files, a gap between the groups. The same items
  /// while the diffs, the selected file and the worktree's note stay.
  fn items(
    &self,
    listing: &FileListing,
    shown: &Shown<'_>,
    (position, note): (usize, Option<WorktreeNote>),
  ) -> Arc<Vec<FileItem>> {
    let key = (listing.keys.clone(), position, note.clone());
    let mut kept = self.items.lock();
    if let Some((made_of, items)) = kept.as_ref()
      && *made_of == key
    {
      return items.clone();
    }
    let mut items = Vec::new();
    for source in [ChangeSource::Checkpoint, ChangeSource::Worktree] {
      let count = listing.count(source);
      if count == 0 {
        continue;
      }
      if !items.is_empty() {
        items.push(FileItem::Gap);
      }
      let note = (source == ChangeSource::Worktree).then(|| note.clone()).flatten();
      items.push(FileItem::Group { source, count, note });
      let files = shown.of(source).diff.files();
      for (at, listed) in listing
        .files
        .iter()
        .enumerate()
        .filter(|(_, listed)| listed.source == source)
      {
        items.push(FileItem::File {
          position: at,
          listed: listed.clone(),
          summary: FileSummary::of(&files[listed.index]),
          selected: at == position,
        });
      }
    }
    let items = Arc::new(items);
    *kept = Some((key, items.clone()));
    items
  }

  /// Choosing a file shows it in place of the current one in the tab's history.
  fn select(&self, props: &ChangesProps) -> files::SelectFile {
    let navigation = self.navigation.clone();
    let (project, key) = (props.project, props.detail.task.key().clone());
    Arc::new(
      move |listed: &Listed| match ChangedFile::new(listed.path.to_string(), listed.source) {
        Ok(file) => {
          let page = ProjectPage::Task(key.clone(), TaskView::Changes(Some(file)));
          navigation.replace(Route::Project(project, page));
        }
        Err(error) => tracing::error!(%error, "a listed file has no path"),
      },
    )
  }
}

/// What the worktree adds to the list, and the notice over the files when it adds nothing it read.
fn worktree(
  ctx: &Ctx,
  props: &ChangesProps,
  worktree: Option<&Result<NotCheckpointed, Message>>,
) -> (Option<Arc<ReadChanges>>, Option<Element>) {
  match worktree {
    Some(Ok(NotCheckpointed::Read(changes))) => (Some(changes.clone()), None),
    Some(Ok(NotCheckpointed::TooLarge { size, .. })) => (None, Some(states::worktree_too_large(ctx, *size))),
    Some(Ok(NotCheckpointed::Removed)) => (None, Some(states::worktree_removed(ctx))),
    Some(Err(error)) => {
      let reads = props.reads.clone();
      (None, Some(states::worktree_failed(ctx, error, move || reads.retry())))
    }
    Some(Ok(NotCheckpointed::Clean)) | None => (None, None),
  }
}

fn head_of(checkpointed: &Checkpointed) -> &str {
  match checkpointed {
    Checkpointed::Read { head, .. } | Checkpointed::TooLarge { head, .. } => head.as_str(),
    Checkpointed::NoCheckpoint => "",
  }
}

/// `value` with its digits grouped as the locale groups them ("20,000").
fn grouped(ctx: &Ctx, value: u32) -> String {
  group_digits(u64::from(value), &ctx.t("changes.groupSeparator"))
}
