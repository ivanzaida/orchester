//! The Changes tab of the task page: what the latest run changed, as the pull request screen shows
//! a pull request's changes (`Rrmhf`: the file list `h8PZJa` beside the selected file's diff
//! `rz27k`). The files are the run's diff from its base to its latest checkpoint, then what its
//! worktree holds beyond that, listed apart as not checkpointed. A new checkpoint reads the diff
//! again; while an agent works, the worktree is read again every few seconds. The file shown is
//! part of the path, so an Activity entry can open it and going back returns to it.

mod listed;
mod toolbar;

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  diff::{Changes, MAX_DIFF_FILES, MAX_DIFF_LINES},
  id::{RunId, TaskId},
};
use parking_lot::Mutex;

use self::{listed::Listed, toolbar::toolbar};
use super::{super::ProjectNav, tab_state::state};
use crate::{
  background,
  messages::Message,
  navigation::route::ChangedFile,
  programs::Programs,
  tasks::{
    changes::{Checkpointed, NotCheckpointed, ReadChanges, not_checkpointed_in},
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

/// How often the worktree is read again while an agent works in it.
const WORKTREE_REFRESH: Duration = Duration::from_secs(3);

/// The run's checkpointed changes as the task page read them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CheckpointedLoad {
  Loading,
  Ready(Checkpointed),
  Failed(Message),
}

#[derive(Clone)]
pub(super) struct ChangesProps {
  pub(super) repo_root: PathBuf,
  pub(super) detail: Arc<TaskDetail>,
  pub(super) checkpointed: CheckpointedLoad,
  /// The file the path names.
  pub(super) file: Option<ChangedFile>,
  /// An agent is at work on the run: its worktree changes.
  pub(super) live: bool,
  /// Reads the checkpointed changes again, after they could not be read.
  pub(super) retry: Signal<u64>,
}

impl PartialEq for ChangesProps {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.detail == other.detail
      && self.checkpointed == other.checkpointed
      && self.file == other.file
      && self.live == other.live
  }
}

impl lurq::app::component::DevtoolsInspectable for ChangesProps {}

pub(super) struct ChangesTab {
  nav: ProjectNav,
  /// Counts the worktree reads while an agent works.
  tick: Signal<u64>,
  live: Arc<AtomicBool>,
  /// The list of the diffs on screen, kept while they stay, so the list sees the same groups.
  listed: Arc<Mutex<Option<Listed>>>,
}

impl Component for ChangesTab {
  type Props = ChangesProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let live = Arc::new(AtomicBool::new(false));
    let (next, working) = (tick.clone(), live.clone());
    ctx
      .create_interval(WORKTREE_REFRESH, move || {
        if working.load(Ordering::Relaxed) {
          next.update(|tick| *tick += 1);
        }
      })
      .start();
    Self {
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
      tick,
      live,
      listed: Arc::new(Mutex::new(None)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChangesProps>().clone();
    self.live.store(props.live, Ordering::Relaxed);
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
    let worktree = self.worktree(ctx, &props, run.id());
    let checkpointed = match &props.checkpointed {
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
    let (uncommitted, notice) = match &worktree {
      Some(Ok(NotCheckpointed::Read(changes))) => (Some(changes.clone()), None),
      Some(Ok(NotCheckpointed::TooLarge { size, .. })) => (None, Some(worktree_too_large(ctx, *size))),
      Some(Err(error)) => (None, Some(worktree_failed(ctx, error))),
      Some(Ok(NotCheckpointed::Clean)) | None => (None, None),
    };
    let listed = self.listed(ctx, committed.as_deref(), uncommitted.as_deref());
    if listed.is_empty() {
      let empty = match (checkpointed, &worktree) {
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
  /// What the run's worktree holds beyond its commit, read again after a checkpoint and, while an
  /// agent works, every [`WORKTREE_REFRESH`]. `None` until first read.
  fn worktree(&self, ctx: &mut Ctx, props: &ChangesProps, run: RunId) -> Option<Result<NotCheckpointed, Message>> {
    let programs = ctx
      .use_context::<Programs>()
      .expect("the root provides Programs before mounting screens");
    let head = props.detail.run.as_ref().and_then(|run| {
      run
        .run
        .last_checkpoint()
        .map(|checkpoint| checkpoint.revision().clone())
    });
    let deps = (props.repo_root.clone(), run, head, self.tick.get());
    let load = ctx.future(deps, move |(repo_root, run, ..)| {
      let programs = programs.clone();
      async move {
        background::run(move || not_checkpointed_in(&repo_root, &programs, run))
          .await?
          .map_err(|error| {
            tracing::warn!(%error, "cannot read the run's worktree");
            error.message()
          })
      }
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Some(Err(error)),
      (Some(changes), None) => Some(Ok(changes)),
      (None, None) => None,
    }
  }

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

/// A notice above the files, inset like the pull request screen's.
fn inset(notice: Notice) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_vertical(SpacingSize::Md)
    .child(notice)
    .into()
}
