//! The Changes tab of the task page: what the latest run changed, as the pull request screen shows
//! a pull request's changes (`Rrmhf`: the file list `h8PZJa` beside the selected file's diff
//! `rz27k`). The files are the run's diff from its base to its latest checkpoint, then what its
//! worktree holds beyond that, listed apart as not checkpointed. The task page reads both
//! (`load`), so the count on the tab's label is of the files listed here. The file shown is part
//! of the path, so an Activity entry can open it and going back returns to it.

mod listed;
mod load;
mod toolbar;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  diff::{Changes, MAX_DIFF_FILES, MAX_DIFF_LINES},
  id::TaskId,
};
use parking_lot::Mutex;

pub(super) use self::load::{RunChanges, checkpointed, not_checkpointed, worktree_refresh};
use self::{listed::Listed, toolbar::toolbar};
use super::{super::ProjectNav, tab_state::state};
use crate::{
  messages::Message,
  navigation::route::ChangedFile,
  tasks::{
    changes::{Checkpointed, NotCheckpointed, ReadChanges},
    detail::TaskDetail,
  },
  ui::{
    button::ActionButton,
    diff::{DiffView, DiffViewProps, FileList, FileListProps},
    icon::Icon,
    message_text,
    notice::Notice,
  },
};

/// The run's checkpointed changes as the task page read them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum CheckpointedLoad {
  #[default]
  Loading,
  Ready(Checkpointed),
  Failed(Message),
}

#[derive(Clone)]
pub(super) struct ChangesProps {
  pub(super) repo_root: PathBuf,
  pub(super) detail: Arc<TaskDetail>,
  pub(super) changes: RunChanges,
  /// The file the path names.
  pub(super) file: Option<ChangedFile>,
  /// Reads the checkpointed changes again, after they could not be read.
  pub(super) retry: Signal<u64>,
}

impl PartialEq for ChangesProps {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.detail == other.detail
      && self.changes == other.changes
      && self.file == other.file
  }
}

impl lurq::app::component::DevtoolsInspectable for ChangesProps {}

pub(super) struct ChangesTab {
  nav: ProjectNav,
  /// The list of the diffs on screen, kept while they stay, so the list sees the same groups.
  listed: Arc<Mutex<Option<Listed>>>,
}

impl Component for ChangesTab {
  type Props = ChangesProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
      listed: Arc::new(Mutex::new(None)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChangesProps>().clone();
    let Some(run) = props.detail.run.as_ref() else {
      let text = ctx.t("runs.changes.noRunDetail");
      return state(
        "changes-empty",
        Icon::FileDiff,
        &ctx.t("runs.changes.noRunTitle"),
        &text,
        Vec::new(),
      );
    };
    let worktree = &props.changes.worktree;
    let checkpointed = match &props.changes.checkpointed {
      CheckpointedLoad::Loading => return loading(ctx),
      CheckpointedLoad::Failed(error) => return failed(ctx, error, &props.retry),
      CheckpointedLoad::Ready(Checkpointed::TooLarge { size, .. }) => {
        return too_large(ctx, *size, run.run.branch().as_str());
      }
      CheckpointedLoad::Ready(checkpointed) => checkpointed,
    };
    let committed = match checkpointed {
      Checkpointed::Read { changes, .. } => Some(changes.clone()),
      Checkpointed::NoCheckpoint | Checkpointed::TooLarge { .. } => None,
    };
    let (uncommitted, notice) = match worktree {
      Some(Ok(NotCheckpointed::Read(changes))) => (Some(changes.clone()), None),
      Some(Ok(NotCheckpointed::TooLarge { size, .. })) => (None, Some(worktree_too_large(ctx, *size))),
      Some(Err(error)) => (None, Some(worktree_failed(ctx, error))),
      Some(Ok(NotCheckpointed::Removed)) => (None, Some(worktree_removed(ctx))),
      Some(Ok(NotCheckpointed::Clean)) | None => (None, None),
    };
    let listed = self.listed(ctx, committed.as_deref(), uncommitted.as_deref());
    if listed.is_empty() {
      let empty = match (checkpointed, worktree) {
        (_, None) => loading(ctx),
        (Checkpointed::NoCheckpoint, _) => no_checkpoint(ctx, run.run.number().get()),
        _ => no_changes(ctx),
      };
      return Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .with_children(notice)
        .child(empty)
        .into();
    }
    let position = listed.position(props.file.as_ref());
    let source = listed.sources[position].clone();
    let shown = if source.worktree { uncommitted } else { committed }.expect("a listed file comes from a read diff");
    let workspace = Row::new()
      .id("changes-workspace")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(self.file_list(ctx, &listed, position, props.detail.task.id()))
      .child(
        Column::new()
          .id("changes-diff")
          .flex(1.0)
          .height(Dimension::Pct(100.0))
          .child(toolbar(ctx, run, checkpointed, &shown, source.worktree))
          .child(ctx.mount_keyed::<DiffView>(
            "changes-diff-view",
            DiffViewProps {
              diff: shown.diff.clone(),
              key: shown.key.clone(),
              index: source.index,
              omitted: "runs.changes.omitted",
            },
          )),
      );
    Column::new()
      .id("task-changes")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .with_children(notice)
      .child(workspace)
      .into()
  }
}

impl ChangesTab {
  /// The list of `committed` and `uncommitted` changes: the same groups while the diffs stay.
  fn listed(&self, ctx: &Ctx, committed: Option<&ReadChanges>, uncommitted: Option<&ReadChanges>) -> Listed {
    let keys = (
      committed.map(|changes| changes.key.clone()),
      uncommitted.map(|changes| changes.key.clone()),
    );
    let mut listed = self.listed.lock();
    if let Some(kept) = listed.as_ref()
      && kept.keys == keys
    {
      return kept.clone();
    }
    let made = Listed::of(ctx, committed, uncommitted);
    *listed = Some(made.clone());
    made
  }

  /// The file list; choosing a file shows it in place of the current one in history.
  fn file_list(&self, ctx: &mut Ctx, listed: &Listed, position: usize, task: TaskId) -> Element {
    let (nav, sources) = (self.nav.clone(), listed.sources.clone());
    ctx.mount_keyed::<FileList>(
      "changes-files",
      FileListProps {
        groups: listed.groups.clone(),
        selected: position,
        select: Arc::new(move |index| {
          if let Some(source) = sources.get(index) {
            let file = ChangedFile {
              path: source.path.to_string(),
              worktree: source.worktree,
            };
            nav.replace_changes(task, &file);
          }
        }),
      },
    )
  }
}

fn loading(ctx: &Ctx) -> Element {
  state(
    "changes-loading",
    Icon::Loader,
    &ctx.t("runs.changes.loadingTitle"),
    &ctx.t("runs.changes.loadingDetail"),
    Vec::new(),
  )
}

fn failed(ctx: &Ctx, error: &Message, retry: &Signal<u64>) -> Element {
  let retry = retry.clone();
  let again = ActionButton::new(ctx.t("runs.changes.tryAgain"))
    .icon(Icon::RefreshCw)
    .id("changes-try-again")
    .on_click(move || retry.update(|attempt| *attempt += 1))
    .render(ctx);
  state(
    "changes-failed",
    Icon::AlertCircle,
    &ctx.t("runs.changes.failedTitle"),
    &message_text(ctx, error),
    vec![again],
  )
}

/// Before the run's first checkpoint nothing of its work is committed.
fn no_checkpoint(ctx: &Ctx, run: u32) -> Element {
  let run = run.to_string();
  state(
    "changes-empty",
    Icon::History,
    &ctx.t("runs.changes.noCheckpointTitle"),
    &ctx.t_args("runs.changes.noCheckpointDetail", [("run", run.as_str())]),
    Vec::new(),
  )
}

fn no_changes(ctx: &Ctx) -> Element {
  state(
    "changes-empty",
    Icon::CircleCheck,
    &ctx.t("runs.changes.noChangesTitle"),
    &ctx.t("runs.changes.noChangesDetail"),
    Vec::new(),
  )
}

/// The size of `size` and the limits, as the locale groups numbers ("20,000").
fn size_args(ctx: &Ctx, size: Changes) -> Vec<(&'static str, String)> {
  let separator = ctx.t("number.groupSeparator");
  let number = |value: u32| orchester_core::tokens::grouped(u64::from(value), &separator);
  vec![
    ("files", number(size.files)),
    ("lines", number(size.additions.saturating_add(size.deletions))),
    ("maxFiles", number(MAX_DIFF_FILES)),
    ("maxLines", number(MAX_DIFF_LINES)),
  ]
}

/// The run's checkpointed changes are larger than Orchester reads; its branch has them.
fn too_large(ctx: &Ctx, size: Changes, branch: &str) -> Element {
  let mut args = size_args(ctx, size);
  args.push(("branch", branch.to_owned()));
  let text = ctx.t_args(
    "runs.changes.tooLargeDetail",
    args.iter().map(|(name, value)| (*name, value.as_str())),
  );
  state(
    "changes-too-large",
    Icon::FileText,
    &ctx.t("runs.changes.tooLargeTitle"),
    &text,
    Vec::new(),
  )
}

fn worktree_too_large(ctx: &Ctx, size: Changes) -> Element {
  let args = size_args(ctx, size);
  let text = ctx.t_args(
    "runs.changes.worktreeTooLargeDetail",
    args.iter().map(|(name, value)| (*name, value.as_str())),
  );
  inset(Notice::new(Icon::FileText, ctx.t("runs.changes.worktreeTooLargeTitle"), text).id("changes-notice"))
}

fn worktree_failed(ctx: &Ctx, error: &Message) -> Element {
  inset(
    Notice::new(
      Icon::AlertCircle,
      ctx.t("runs.changes.worktreeFailedTitle"),
      message_text(ctx, error),
    )
    .id("changes-notice"),
  )
}

/// The run ended and its worktree was removed: what it committed is all there is to show.
fn worktree_removed(ctx: &Ctx) -> Element {
  inset(
    Notice::new(
      Icon::FolderX,
      ctx.t("runs.changes.worktreeRemovedTitle"),
      ctx.t("runs.changes.worktreeRemovedDetail"),
    )
    .id("changes-notice"),
  )
}

/// A notice above the files, inset like the pull request screen's.
fn inset(notice: Notice) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_vertical(SpacingSize::Md)
    .child(notice)
    .into()
}
