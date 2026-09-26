//! A task and its latest run (02.10 `RD4pc`): the topbar with the way back to the task list and
//! the run controls, the heading, the tabs, and the Summary (`uVIxb`, `F5Lsi`) or Activity
//! (02.I `SfXU1`) tab. The run is read from the project database and read again whenever the
//! runner reports a change. Conversation, Changes and Artifacts, and Pause and Cancel run, come
//! in later milestones and are shown unavailable.

mod activity;
mod heading;
mod summary;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::TaskId;

use self::activity::{ActivityProps, ActivityTab};
use super::{ProjectNav, Section, page};
use crate::{
  background, messages,
  tasks::detail::TaskDetail,
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    pressable::{pressable, surface},
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Section tab row (`X7jYe`).
const TAB_ROW_HEIGHT: f32 = 42.0;

#[derive(Clone, Debug, PartialEq)]
pub struct TaskProps {
  pub project: OpenProject,
  pub task: TaskId,
  /// Changes when tasks were saved or the runner changed a run: the task is read again.
  pub revision: (u64, u64),
}

impl lurq::app::component::DevtoolsInspectable for TaskProps {}

/// The task as read, shared with the tabs.
#[derive(Clone, Debug, PartialEq)]
struct LoadedTask(Arc<TaskDetail>);

impl lurq::app::component::DevtoolsInspectable for LoadedTask {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskTab {
  Summary,
  Activity,
}

impl lurq::app::component::DevtoolsInspectable for TaskTab {}

pub struct TaskScreen {
  tab: Signal<TaskTab>,
  nav: ProjectNav,
}

impl Component for TaskScreen {
  type Props = TaskProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      tab: ctx.signal(TaskTab::Summary),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskProps>().clone();
    let deps = (props.project.repo_root.clone(), props.task, props.revision);
    let load = ctx.future(deps, |(repo_root, task, _)| async move {
      background::run(move || TaskDetail::load_in(&repo_root, task))
        .await?
        .map(|detail| LoadedTask(Arc::new(detail)))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the task");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    let column = Column::new()
      .id("task-page")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    let Some(LoadedTask(detail)) = state.data else {
      let panel = match &state.error {
        Some(error) => RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("runs.page.loadFailed"),
          &message_text(ctx, error),
        ),
        None => RecoveryPanel::new(
          Icon::Loader,
          &ctx.t("runs.page.loading"),
          &ctx.t("runs.page.loadingDetail"),
        ),
      };
      return column
        .child(self.topbar(ctx, &props, None))
        .child(centered(panel.into()));
    };
    let tab = self.tab.get();
    let body = match tab {
      TaskTab::Summary => summary::summary(ctx, &detail),
      TaskTab::Activity => ctx.mount_keyed::<ActivityTab>(
        "task-activity",
        ActivityProps {
          repo_root: props.project.repo_root.clone(),
          detail: detail.clone(),
          revision: props.revision,
        },
      ),
    };
    column
      .child(self.topbar(ctx, &props, Some(&detail)))
      .child(heading::heading(ctx, &detail))
      .child(self.tabs(ctx))
      .child(body)
  }
}

impl TaskScreen {
  /// The breadcrumb back to the task list, then Pause run and Cancel run, which come with a
  /// later milestone (`ctwcG`).
  fn topbar(&self, ctx: &Ctx, props: &TaskProps, detail: Option<&TaskDetail>) -> Element {
    let nav = self.nav.clone();
    let key = detail.map_or_else(String::new, |detail| detail.task.key().to_string());
    let breadcrumb = pressable(surface().id("task-breadcrumb"), move || nav.show(Section::Tasks)).child(
      Text::new(&ctx.t_args(
        "runs.page.breadcrumb",
        [
          ("project", props.project.project.name().as_str()),
          ("task", key.as_str()),
        ],
      ))
      .variant(TypographyStyle::Description)
      .nowrap(),
    );
    let pause = ActionButton::new(ctx.t("runs.page.pause"))
      .icon(Icon::Pause)
      .id("task-pause-run")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT));
    let cancel = ActionButton::new(ctx.t("runs.page.cancel"))
      .icon(Icon::Square)
      .id("task-cancel-run")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT));
    page::topbar_with(breadcrumb.into(), [pause.render(ctx), cancel.render(ctx)])
  }

  /// Summary and Activity; the others need later milestones (`X7jYe`).
  fn tabs(&self, ctx: &Ctx) -> Element {
    let selected = self.tab.get();
    let height = Dimension::Pct(100.0);
    let tab = |id, key, value: Option<TaskTab>| {
      let item = Tab::new(id, ctx.t(key), height);
      let Some(value) = value else {
        return item;
      };
      let signal = self.tab.clone();
      item.selected(value == selected).on_click(move || signal.set(value))
    };
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      ExtraSpacing::Step.role(),
      Some(SpacingSize::Section),
      [
        tab("task-tab-summary", "runs.page.tabs.summary", Some(TaskTab::Summary)),
        tab("task-tab-conversation", "runs.page.tabs.conversation", None),
        tab("task-tab-changes", "runs.page.tabs.changes", None),
        tab("task-tab-activity", "runs.page.tabs.activity", Some(TaskTab::Activity)),
        tab("task-tab-artifacts", "runs.page.tabs.artifacts", None),
      ],
    )
  }
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}
