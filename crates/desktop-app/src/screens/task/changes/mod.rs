//! The Changes tab of a task's page (`Task · Changes · dark` `DYun5`): what the latest run changed. The
//! toolbar names the run's base and its latest checkpoint; the file list (`qT0kd`) holds the files of the
//! run's diff from its base to that checkpoint, then what its worktree holds beyond it, listed apart as
//! not checkpointed; beside it, the selected file's unified diff with line numbers and syntax colors
//! (`RsPIH`). The file shown is part of the path (`?file=`, `&from=worktree`), and choosing another is a
//! new entry of the tab's history.
//!
//! The task page's frame owns the reads ([`ChangesReads::new`] in its `create`, provided to its parts
//! with `ctx.provide`) and reads with them on every render ([`ChangesReads::read`] with [`Follow::of`]);
//! its page tabs show [`count`] through [`tab_label`] (`Changes 6`, `Changes 4+`) and its Changes slot
//! mounts [`tab`], both what was read last, so the count is of the files listed here and nothing is read
//! twice.

pub(crate) mod diff;
pub(crate) mod files;
pub(crate) mod listed;
mod load;
mod open_file;
mod states;
mod toolbar;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::KeyboardEvent,
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

pub use self::load::{ChangesReads, FileCount, Follow, tab_label};
use self::{
  diff::{CutNote, DiffView, DiffViewProps},
  files::{FILE_LIST_WIDTH, FileItem, FileList, FileListProps, FileSummary, WorktreeNote},
  listed::{FileListing, ListKeys, Listed},
  load::CheckpointedRead,
  toolbar::toolbar,
};
use super::TaskContext;
use crate::{
  navigation::{Navigation, press::OpenIn},
  routes::{ChangeSource, ChangedFile, ProjectPage, Route, TaskTab, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

/// How many files the Changes tab lists, for its label; `None` while that is not known: what the page's
/// [`ChangesReads`] read last.
pub fn count(ctx: &mut Ctx) -> Option<FileCount> {
  ChangesReads::of(ctx).latest().and_then(|changes| changes.file_count())
}

/// The Changes tab of `task`'s page, on the file the path names: what the page's [`ChangesReads`] read
/// last.
pub fn tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let file = route_view(ctx).and_then(|view| view.file().cloned());
  let props = ChangesProps {
    project: task.project.open.project.id(),
    detail: task.detail.clone(),
    file,
    agent_at_work: Follow::of(&task.detail, task.online, TaskTab::Changes).agent_at_work,
    reads: ChangesReads::of(ctx),
  };
  ctx.mount_keyed::<ChangesTab>("task-changes-tab", props)
}

/// The task page's view the window's path names.
fn route_view(ctx: &mut Ctx) -> Option<TaskView> {
  match Route::parse(&ctx.route_path()) {
    Ok(Route::Project(_, page)) => page.task_view().map(|(_, view)| view.clone()),
    _ => None,
  }
}

/// What the Changes tab shows.
#[derive(Clone)]
struct ChangesProps {
  project: ProjectId,
  detail: Arc<TaskDetail>,
  /// The file the path names.
  file: Option<ChangedFile>,
  /// An agent works on the run: the worktree is read again while it does.
  agent_at_work: bool,
  reads: ChangesReads,
}

impl PartialEq for ChangesProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.detail == other.detail
      && self.file == other.file
      && self.agent_at_work == other.agent_at_work
  }
}

impl lurq::app::component::DevtoolsInspectable for ChangesProps {}

/// The file list's items as last made, and what they were made of.
type ItemsKey = (ListKeys, usize, Option<WorktreeNote>);
type KeptItems = Option<(ItemsKey, Arc<Vec<FileItem>>)>;

struct ChangesTab {
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
    let changes = props.reads.latest();
    if let Some(unreadable) = &props.detail.latest_unreadable {
      return column.child(states::unreadable(ctx, &message_text(ctx, &unreadable.title())));
    }
    let Some(run) = props.detail.run.as_ref() else {
      return column.child(states::no_run(ctx));
    };
    let Some(changes) = changes.as_ref() else {
      return column.child(states::reading(ctx));
    };
    let checkpointed = match &changes.checkpointed {
      CheckpointedRead::Loading => return column.child(states::reading(ctx)),
      CheckpointedRead::Failed(error) => {
        let reads = props.reads.clone();
        return column.child(states::failed(ctx, error, move || reads.retry()));
      }
      CheckpointedRead::Ready(checkpointed) => checkpointed,
    };
    let column = column.child(toolbar(ctx, run, checkpointed, self.try_locally(ctx, &props)));
    let committed = match checkpointed {
      Checkpointed::TooLarge { size, .. } => {
        return column.child(states::too_large(ctx, *size, run.run.branch().as_str()));
      }
      Checkpointed::Read { changes, .. } => Some(changes.clone()),
      Checkpointed::NoCheckpoint => None,
    };
    let checkpointed_files = committed.is_some();
    let (uncommitted, notice) = worktree(ctx, &props, (changes.worktree.as_ref(), checkpointed_files));
    let column = column.with_children(notice);
    let listing = self.listing(committed.as_deref(), uncommitted.as_deref());
    if listing.is_empty() {
      // A card says the run changed nothing only when its worktree was read clean; otherwise the notice
      // over the files says what is not known.
      let empty = match (checkpointed, &changes.worktree) {
        (_, None) => Some(states::reading(ctx)),
        (Checkpointed::NoCheckpoint, Some(Ok(NotCheckpointed::Clean))) => {
          let ended = run.run.status().is_terminal();
          Some(states::no_checkpoint(ctx, run.run.number().get(), ended))
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

  /// *Try locally* (`Local integration` `G2Vq3` in `03 · Changes`): Local integration, in this tab's place.
  fn try_locally(&self, ctx: &Ctx, props: &ChangesProps) -> ActionButton {
    let task = &props.detail.task;
    let page = ProjectPage::task(
      task.key().clone(),
      TaskView::LocalIntegration,
      props.detail.not_shared(),
    );
    ActionButton::new(
      "changes-try-locally",
      ButtonKind::Ghost,
      ctx.t("changes.toolbar.tryLocally"),
    )
    .icon(Icon::MonitorPlay)
    .opens(self.navigation.to(Route::Project(props.project, page)))
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
    // No file named: the one shown last stays while it is listed, e.g. when a read adds files above it.
    let named = props.file.clone().or_else(|| props.reads.shown_file());
    let position = listing.position(named.as_ref());
    let selected = listing.files[position].clone();
    // A listed path is never empty, so it always names a file.
    if let Ok(file) = ChangedFile::new(selected.path.to_string(), selected.source) {
      props.reads.set_shown(file);
    }
    let note = shown.uncommitted.map(|changes| WorktreeNote {
      revision: short_revision(changes.from.as_str()).into(),
      live: props.agent_at_work.then(|| {
        let agent = run.current_step().and_then(|step| step.agent.clone());
        (agent, props.reads.last_read())
      }),
    });
    let items = self.items(listing, shown, (position, note));
    let select = self.select(props);
    let keys = arrow_keys(listing, position, select.clone());
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
    let head = match shown.checkpointed {
      Checkpointed::Read { head, .. } | Checkpointed::TooLarge { head, .. } => Some(head),
      Checkpointed::NoCheckpoint => None,
    };
    let file = diff
      .diff
      .files()
      .get(selected.index)
      .expect("a listed file is in the diff it was listed from");
    let opened = open_file::file_view(file, selected.source, (run.run.id(), &diff.from, head));
    let open = Route::Project(props.project, ProjectPage::Files(opened));
    let view = ctx.mount_keyed::<DiffView>(
      "changes-diff-view",
      DiffViewProps {
        diff: diff.diff.clone(),
        key: diff.key.clone(),
        index: selected.index,
        file: format!("{}:{}", selected.source.name(), selected.path).into(),
        source,
        open: Some(open),
        viewed: None,
        cut: CutNote {
          key: "changes.diff.omitted",
          action: None,
        },
        annotations: None,
      },
    );
    Row::new()
      .id("changes-body")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .on_key_down(keys)
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

  /// Choosing a file opens it where the press asks; in the active tab as a new entry of its history.
  fn select(&self, props: &ChangesProps) -> files::SelectFile {
    let navigation = self.navigation.clone();
    let (project, key, kept) = (
      props.project,
      props.detail.task.key().clone(),
      props.detail.not_shared(),
    );
    Arc::new(
      move |listed: &Listed, at| match ChangedFile::new(listed.path.to_string(), listed.source) {
        Ok(file) => {
          let page = ProjectPage::task(key.clone(), TaskView::Changes(Some(file)), kept);
          navigation.open(Route::Project(project, page), at);
        }
        Err(error) => tracing::error!(%error, "a listed file has no path"),
      },
    )
  }
}

/// ArrowDown and ArrowUp choose the next and the previous file of the list, unless a text field has the
/// keys or a modifier is held.
pub(crate) fn arrow_keys(
  listing: &FileListing,
  position: usize,
  select: files::SelectFile,
) -> impl Fn(KeyboardEvent) + use<> {
  let files = listing.files.clone();
  move |event: KeyboardEvent| {
    let plain = !(event.ctrl || event.alt || event.meta || event.shift);
    if event.default_prevented() || event.text_input_focused || !plain {
      return;
    }
    let next = match event.key.as_str() {
      "ArrowDown" => position + 1,
      "ArrowUp" if position > 0 => position - 1,
      _ => return,
    };
    if let Some(file) = files.get(next) {
      event.prevent_default();
      select(file, OpenIn::ActiveTab);
    }
  }
}

/// What the worktree adds to the list, and the notice over the files when it adds nothing it read.
fn worktree(
  ctx: &Ctx,
  props: &ChangesProps,
  (worktree, checkpointed): (Option<&Result<NotCheckpointed, Message>>, bool),
) -> (Option<Arc<ReadChanges>>, Option<Element>) {
  match worktree {
    Some(Ok(NotCheckpointed::Read(changes))) => (Some(changes.clone()), None),
    Some(Ok(NotCheckpointed::TooLarge { size, .. })) => (None, Some(states::worktree_too_large(ctx, *size))),
    Some(Ok(NotCheckpointed::Removed)) => (None, Some(states::worktree_removed(ctx))),
    Some(Err(error)) => {
      let reads = props.reads.clone();
      (
        None,
        Some(states::worktree_failed(ctx, (error, checkpointed), move || {
          reads.retry()
        })),
      )
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
