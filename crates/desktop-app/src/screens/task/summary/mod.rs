//! The Summary tab (`Body` `ZhoTU` in `lUXff`): in its column, where the task stands (`notice`), the
//! decision the run waits for (P2's panels mount at [`decisions`]), the run's steps (`steps`), the brief
//! and the acceptance criteria (`brief`) and *Runs of this task* (`runs`); beside it *This run*
//! (`this_run`). A task whose latest run cannot be read shows why, that the run's actions are
//! unavailable, and its runs. The organization history comes later (agreed deviation D4).

mod brief;
mod live;
mod notice;
pub(super) mod runs;
mod steps;
mod this_run;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  run::{PauseReason, RunLimit, RunStatus},
  time::Timestamp,
};
use orchester_workspace::tasks::{approvals::ShownApproval, detail::TaskDetail, integration_jobs::ProjectJobState};

use self::runs::Runs;
use super::{PageState, read::TaskLeftovers};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  services::Services,
  theme::{TaskSpacing, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

#[derive(Clone, PartialEq)]
pub struct SummaryProps {
  pub project: ShownProject,
  pub detail: Arc<TaskDetail>,
  pub online: bool,
  pub left: TaskLeftovers,
  pub jobs: ProjectJobState,
}

impl lurq::app::component::DevtoolsInspectable for SummaryProps {}

pub struct TaskSummary;

impl Component for TaskSummary {
  type Props = SummaryProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    let detail = &props.detail;
    let removal = PageState::of(ctx).removal.get();
    let runs = runs::section(
      ctx,
      &Runs {
        project: &props.project,
        detail,
        left: &props.left,
        jobs: &props.jobs,
        removal,
      },
    );
    if let Some(unreadable) = &detail.latest_unreadable {
      let notice = Notice::new("task-notice", NoticeTone::Danger, message_text(ctx, &unreadable.title()))
        .icon(Icon::FileWarning)
        .body(message_text(ctx, &unreadable.detail));
      let actions = reason_line("task-unreadable-actions", &message_text(ctx, &unreadable.actions()));
      let parts = vec![notice.into(), actions.into()];
      return layout(column(parts.into_iter().chain(runs).collect()), None);
    }
    let parts = self.parts(ctx, &props, runs);
    let root = props.project.open.repo_root.clone();
    let panel = match &detail.run {
      Some(run) => this_run::this_run(ctx, run, &root, detail.lost_its_runner(props.online)),
      None => {
        let pipelines = match props.project.feed.tasks.get() {
          crate::project::TasksState::Ready(tasks) => tasks.pipelines.to_vec(),
          _ => Vec::new(),
        };
        this_run::this_task(ctx, detail, &pipelines)
      }
    };
    layout(column(parts), Some(panel))
  }
}

impl TaskSummary {
  /// The column's sections, in the design's order.
  fn parts(&self, ctx: &mut Ctx, props: &SummaryProps, runs: Option<Element>) -> Vec<Element> {
    let detail = &props.detail;
    let lost = detail.lost_its_runner(props.online);
    let runner = props
      .project
      .feed
      .status
      .get()
      .map(|status| status.state_for(&props.project.open.repo_root));
    let approval = waiting_approval(&props.project, detail);
    let said = notice::said(ctx, detail, runner.as_ref().filter(|_| lost), approval.as_ref());
    let conversation = said.conversation;
    let mut notice = said.notice();
    if conversation {
      notice = notice.action(open_conversation(ctx, props));
    }
    let now = Services::of(ctx).clock.now();
    let waiting: Vec<(Arc<str>, Timestamp)> = approval
      .iter()
      .filter_map(|shown| Some((shown.step_key.clone()?, shown.approval.requested_at)))
      .collect();
    // What the task asked for is what its run was asked, even after the task was edited for a later run.
    let details = detail.run_task.as_ref().unwrap_or(&detail.task).details();
    let steps = detail
      .run
      .as_ref()
      .map(|run| steps::section(ctx, run, lost, &waiting, now));
    std::iter::once(notice.into())
      .chain(decisions(ctx, detail))
      .chain(steps)
      .chain(brief::sections(ctx, details))
      .chain(runs)
      .collect()
  }
}

/// Where the decisions the run waits for mount (P2: plan review, raise the limit, the decision after
/// retries, Ready for review with its merge). Until they are built, a run that waits for one says the
/// decision is not built in this app yet.
fn decisions(ctx: &Ctx, detail: &TaskDetail) -> Option<Element> {
  let run = detail.run.as_ref()?;
  let waits = match run.run.status() {
    RunStatus::Running => !run.run.awaiting_plan().is_empty(),
    RunStatus::Paused(PauseReason::LimitReached(limit)) => *limit != RunLimit::DailyBudget,
    RunStatus::WaitingForPerson(_) => true,
    RunStatus::Succeeded => detail.merge.is_none(),
    _ => false,
  };
  waits.then(|| reason_line("task-decision-unavailable", &ctx.t("summary.decisionNotBuilt")).into())
}

/// A tool request of the task's run that waits for the person, the oldest first.
fn waiting_approval(project: &ShownProject, detail: &TaskDetail) -> Option<ShownApproval> {
  let run = detail.run.as_ref()?.id();
  match project.feed.tasks.get() {
    crate::project::TasksState::Ready(tasks) => tasks
      .pending_tool_approvals
      .iter()
      .find(|shown| shown.approval.run == run)
      .cloned(),
    _ => None,
  }
}

fn open_conversation(ctx: &mut Ctx, props: &SummaryProps) -> ActionButton {
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    props.project.open.project.id(),
    ProjectPage::Task(props.detail.task.key().clone(), TaskView::of(TaskTab::Conversation)),
  );
  ActionButton::new("task-open-conversation", ButtonKind::Secondary, ctx.t("summary.openConversation"))
    .icon(Icon::MessageSquare)
    .on_click(move || navigation.go(route.clone()))
}

/// A section's title with its count (`Head · Steps` `P4Q0cn`).
pub fn section_head(ctx: &Ctx, id: &str, title: &str, count: Option<usize>) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(title)).variant(TaskText::SectionTitle.role()))
    .with_children(count.map(|count| {
      Text::new(&count.to_string())
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
    }))
    .into()
}

/// The Summary's column of `parts`, scrolling when it is taller than the window (`Summary column`
/// `N3nD9i`).
fn column(parts: Vec<Element>) -> Element {
  let content = Column::new()
    .id("task-summary-column")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(TaskSpacing::Panel.role())
        .left(SpacingSize::Section)
        .right(SpacingSize::Section)
        .bottom(TaskSpacing::Foot.role()),
    )
    .spacing(TaskSpacing::Block.role())
    .with_children(parts);
  ScrollVertical::new(content)
    .id("task-summary-scroll")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .into()
}

fn layout(column: Element, panel: Option<Element>) -> Element {
  Row::new()
    .id("task-summary")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(column))
    .with_children(panel)
    .into()
}

