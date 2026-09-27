//! The Conversation tab (02.12 `jgqPe`): the task's active work (`FjbKY`) beside the task
//! conversation panel (`kjsmd`, see [`panel`]). The active work repeats where the run stands — its
//! notice, its progress per step and its working context — so a message is written with the work in
//! view.

mod composer;
mod entry;
mod panel;

use std::{path::Path, sync::Arc};

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};

pub use self::panel::{ConversationPanel, ConversationProps};
use super::{
  notice::{Lost, notice},
  progress::progress_list,
  summary::{budget_value, permissions_row},
};
use crate::{
  display,
  navigation::route::{Page, TaskPageTab},
  screens::project::ProjectNav,
  tasks::{
    conversation::Conversation,
    detail::{RunView, TaskDetail},
  },
  theme::{ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    pressable::{pressable, surface},
    property::{ValueStyle, property, styled_property},
  },
};

/// What the tab needs besides the task.
pub(super) struct TabContext<'a> {
  pub(super) repo_root: &'a Path,
  pub(super) lost: Option<Lost>,
  /// Files changed up to the latest checkpoint, once read.
  pub(super) files: Option<usize>,
  pub(super) nav: &'a ProjectNav,
  /// Set to have the composer take the focus (Ask agent, Reply).
  pub(super) focus: &'a Signal<bool>,
  /// Bumped once a message is stored, so the task is read again.
  pub(super) reload: &'a Signal<u64>,
}

pub(super) fn conversation_tab(ctx: &mut Ctx, detail: &Arc<TaskDetail>, tab: &TabContext<'_>) -> Element {
  let conversation = Conversation::of(detail, tab.lost.is_some());
  let props = ConversationProps {
    repo_root: tab.repo_root.to_path_buf(),
    task: detail.task.id(),
    run: detail.run.as_ref().map(RunView::id),
    conversation: Arc::new(conversation),
    focus: tab.focus.clone(),
    reload: tab.reload.clone(),
  };
  Row::new()
    .id("task-conversation-tab")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(active_work(ctx, detail, tab))
    .child(ctx.mount_keyed::<ConversationPanel>("task-conversation", props))
    .into()
}

/// Active work (`FjbKY`): the notice, the progress rows (`qB7mj`) and the working context, in a
/// viewport with the scroll gutter; the communication policy (`e2D1lv`) stays at the bottom.
fn active_work(ctx: &mut Ctx, detail: &TaskDetail, tab: &TabContext<'_>) -> Element {
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Section)
    .padding_left(SpacingSize::Section)
    .padding_right(ExtraSpacing::BesideGutter.role())
    .padding_bottom(SpacingSize::Section)
    .spacing(ExtraSpacing::Form.role())
    .child(notice(ctx, detail, tab.lost))
    .with_children(
      detail
        .run
        .as_ref()
        .map(|run| progress_list(ctx, run, tab.lost.is_some())),
    )
    .with_children(detail.run.as_ref().map(|run| working_context(ctx, run, tab)));
  let policy = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Form.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Section)
    .child(
      Text::new(&ctx.t("conversation.policy"))
        .id("task-conversation-policy")
        .variant(ExtraTypography::Detail.role())
        .width(Dimension::Pct(100.0)),
    );
  Column::new()
    .id("task-active-work")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .child(
      ScrollVertical::new(content)
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .child(policy)
    .into()
}

/// Working context (`PdgpB` and the `tfGRt` rows after it): branch, last checkpoint, changed files
/// with the way to the Changes tab, budget and the current agent's permissions.
fn working_context(ctx: &mut Ctx, run: &RunView, tab: &TabContext<'_>) -> Element {
  let checkpoint = run.run.last_checkpoint().map_or_else(
    || ctx.t("conversation.work.noCheckpoint"),
    |checkpoint| {
      ctx.t_args(
        "runs.summary.checkpointValue",
        [
          ("revision", display::short_revision(checkpoint.revision().as_str())),
          ("time", display::clock(checkpoint.created_at()).as_str()),
        ],
      )
    },
  );
  Column::new()
    .id("task-working-context")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Form.role())
    .child(Text::new(&ctx.t("conversation.work.title")).variant(ExtraTypography::SectionHeading.role()))
    .child(styled_property(
      &ctx.t("runs.summary.branch"),
      run.run.branch().as_str(),
      ValueStyle::Mono,
    ))
    .child(property(&ctx.t("runs.summary.checkpoint"), &checkpoint))
    .child(changed_files(ctx, run, tab))
    .child(property(&ctx.t("runs.summary.budget"), &budget_value(ctx, run)))
    .with_children(permissions_row(ctx, run, tab.repo_root))
    .into()
}

/// "Changed files · 3 files · view changes ↗", which opens the Changes tab.
fn changed_files(ctx: &Ctx, run: &RunView, tab: &TabContext<'_>) -> Element {
  let value = match tab.files {
    Some(1) => ctx.t("conversation.work.files.one"),
    Some(count) => ctx.t_args("conversation.work.files.other", [("count", count.to_string().as_str())]),
    None => ctx.t("conversation.work.filesUnknown"),
  };
  let (nav, task) = (tab.nav.clone(), run.run.task());
  pressable(surface().id("task-conversation-changes"), move || {
    nav.show(Page::Task(task, TaskPageTab::Changes));
  })
  .width(Dimension::Pct(100.0))
  .child(
    Row::new()
      .width(Dimension::Pct(100.0))
      .child(property(&ctx.t("conversation.work.changedFiles"), &value)),
  )
  .into()
}
