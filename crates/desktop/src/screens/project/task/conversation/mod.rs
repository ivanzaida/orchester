//! The Conversation tab (02.12 `jgqPe`, as built 02.S `JOG1i`): the task's active work (`FjbKY`)
//! beside the task conversation panel (`kjsmd`, see [`panel`]). The active work repeats where the
//! run stands — its notice, its progress per step and its working context — so a message is written
//! with the work in view. Once no agent reads messages any more, the policy about when they do is
//! left out; a merged run's notice says where its work went, and its permissions cannot change, so
//! their row has no link (02.S2 `m3gLZ`).

mod composer;
mod entry;
mod panel;

use std::{path::Path, sync::Arc};

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::layout_kind::ScrollState,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::Leftovers;

pub use self::panel::{ConversationPanel, ConversationProps};
use super::{
  notice::{Lost, notice},
  permissions::permissions_value,
  progress::progress_list,
  ready::merged_notice,
  summary::{budget_value, permissions_row},
};
use crate::{
  display,
  navigation::route::{Page, TaskPageTab},
  screens::project::ProjectNav,
  tasks::{
    conversation::{Conversation, PanelState},
    detail::{RunView, TaskDetail},
  },
  theme::{ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    icon::Icon,
    notice::Notice,
    pressable::surface,
    property::{ValueStyle, property, styled_property},
  },
};

/// Puts the focus in the conversation's composer once it shows (Ask agent, Reply); provided by the
/// task page.
#[derive(Clone)]
pub(super) struct ComposerFocus(pub(super) Signal<bool>);

/// What the tab needs besides the task.
pub(super) struct TabContext<'a> {
  pub(super) repo_root: &'a Path,
  pub(super) lost: Option<Lost>,
  /// Files changed up to the latest checkpoint, once read.
  pub(super) files: Option<usize>,
  /// What the latest run left once it ended, once read.
  pub(super) leftovers: Option<&'a Leftovers>,
  pub(super) nav: &'a ProjectNav,
  /// Set to have the composer take the focus (Ask agent, Reply).
  pub(super) focus: &'a Signal<bool>,
  /// Bumped once a message is stored, so the task is read again.
  pub(super) reload: &'a Signal<u64>,
  /// Where the active work is scrolled.
  pub(super) scroll: &'a ScrollState,
}

pub(super) fn conversation_tab(ctx: &mut Ctx, detail: &Arc<TaskDetail>, tab: &TabContext<'_>) -> Element {
  let conversation = Conversation::of(detail, tab.lost.is_some(), tab.leftovers);
  let open = conversation.state.takes_messages();
  let removed = conversation.state == PanelState::WorktreeRemoved;
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
    .child(active_work(ctx, detail, tab, open, removed))
    .child(ctx.mount_keyed::<ConversationPanel>("task-conversation", props))
    .into()
}

/// Active work (`FjbKY`): the notice, the progress rows (`qB7mj`) and the working context, in a
/// viewport with the scroll gutter; the communication policy (`e2D1lv`) stays at the bottom while
/// the conversation is `open`, that is, while an agent reads messages.
fn active_work(ctx: &mut Ctx, detail: &TaskDetail, tab: &TabContext<'_>, open: bool, removed: bool) -> Element {
  let notice = match &detail.merge {
    Some(merge) => merged_notice(ctx, merge, tab.leftovers),
    None if removed => Notice::new(
      Icon::FolderX,
      ctx.t("runs.changes.worktreeRemovedTitle"),
      ctx.t("conversation.worktreeRemovedNoticeDetail"),
    )
    .id("task-notice")
    .into(),
    None => notice(ctx, detail, tab.lost),
  };
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Section)
    .padding_left(SpacingSize::Section)
    .padding_right(ExtraSpacing::BesideGutter.role())
    .padding_bottom(SpacingSize::Section)
    .spacing(ExtraSpacing::Form.role())
    .child(notice)
    .with_children(
      detail
        .run
        .as_ref()
        .map(|run| progress_list(ctx, run, tab.lost.is_some())),
    )
    .with_children(
      detail
        .run
        .as_ref()
        .map(|run| working_context(ctx, run, tab, detail.merge.is_some() || removed)),
    );
  let policy = open.then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Form.role())
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Section)
      .child(
        Text::new(&ctx.t("conversation.policy"))
          .id("task-conversation-policy")
          .variant(ExtraTypography::Detail.role())
          .width(Dimension::Pct(100.0)),
      )
  });
  Column::new()
    .id("task-active-work")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .child(
      ScrollVertical::new(content)
        .with_scroll_state(tab.scroll.clone())
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .with_children(policy)
    .into()
}

/// Working context (`PdgpB` and the `tfGRt` rows after it): branch, last checkpoint, changed files
/// with the way to the Changes tab, budget and the current agent's permissions. Once no agent
/// can continue, its permissions are only shown (`d9SVut` in 02.S2).
fn working_context(ctx: &mut Ctx, run: &RunView, tab: &TabContext<'_>, read_only: bool) -> Element {
  let permissions = if read_only {
    run
      .current_step()
      .and_then(|step| permissions_value(ctx, step))
      .map(|value| property(&ctx.t("runs.summary.permissions"), &value))
  } else {
    permissions_row(ctx, run, tab.repo_root, true)
  };
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
    .child(property(&ctx.t("runs.summary.budget"), &budget_value(ctx, run)).id("task-conversation-budget"))
    .with_children(permissions)
    .into()
}

/// "Changed files · 3 files · view changes ↗", which opens the Changes tab.
fn changed_files(ctx: &Ctx, run: &RunView, tab: &TabContext<'_>) -> Element {
  let value = match tab.files {
    Some(1) => ctx.t("conversation.work.files.one"),
    Some(count) => ctx.t_args("conversation.work.files.other", [("count", count.to_string().as_str())]),
    None => ctx.t("conversation.work.filesUnknown"),
  };
  tab
    .nav
    .link(Page::Task(run.run.task(), TaskPageTab::Changes))
    .on(surface().id("task-conversation-changes"))
    .width(Dimension::Pct(100.0))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .child(property(&ctx.t("conversation.work.changedFiles"), &value)),
    )
    .into()
}
