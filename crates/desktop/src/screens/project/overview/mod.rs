//! Project overview (02.08 `Twtqc`): the runner notice while it could not restart (02.H
//! `EFY7I`), the heading with New task, the project pulse (`C95Z93`), the newest tasks by tab
//! (`BPSF0`) and, beside them, what needs the user and the recent run activity (`EKUWS`). While
//! the project has no tasks it says what the project is. Every figure is counted from the
//! project's tasks and runs; the pipeline filter is not available yet.

mod attention;
mod pulse;
mod task_list;

use std::time::SystemTime;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::{ProjectNav, TasksState, approval_queue::ApprovalQueue, runner::connection_notice};
use crate::{
  display,
  navigation::route::Page,
  runner::{Runners, state::RunnerState},
  tasks::{
    overview::{OverviewTab, Pulse},
    usage::ProjectUsage,
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
    panel::RecoveryPanel,
    property::property,
  },
  workspace::OpenProject,
};

/// Project bar height (`CsB3h`) and New task button height (`nMeVq`).
const PROJECT_BAR_HEIGHT: f32 = 64.0;
const NEW_TASK_HEIGHT: f32 = 38.0;
/// Longest repository path shown in full in the overview panel.
const PATH_CHARS: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub struct OverviewProps {
  pub open: OpenProject,
  /// The tab of the task list.
  pub tab: OverviewTab,
  pub tasks: TasksState,
  /// Today's tokens and the daily budget, once read.
  pub usage: Option<ProjectUsage>,
  /// The project's runner, and since when it has had trouble.
  pub runner: RunnerState,
  pub runner_since: Option<SystemTime>,
  /// Agents at work, as the runner last reported; `None` until it did.
  pub agents_working: Option<usize>,
}

impl lurq::app::component::DevtoolsInspectable for OverviewProps {}

pub struct OverviewScreen {
  nav: ProjectNav,
  runners: Runners,
}

impl Component for OverviewScreen {
  type Props = OverviewProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
      runners: ctx
        .use_context::<Runners>()
        .expect("the root provides Runners before mounting screens"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OverviewProps>().clone();
    let notice = connection_notice(ctx, &props.runner, props.runner_since, &self.runners);
    // The overview body spaces its sections by 28 (`FMRs3`), by 24 below a connection notice
    // (`G8JTc`, `c7qiCh`, `s3EdDI`).
    let spacing = if notice.is_some() {
      SpacingSize::Lg
    } else {
      ExtraSpacing::Rail.role()
    };
    let queue = ctx.use_context::<ApprovalQueue>();
    let mut body = Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding(SpacingSize::Section)
      .spacing(spacing)
      .with_children(notice)
      .child(heading(ctx, &props.open, &self.nav));
    body = match &props.tasks {
      TasksState::Loading => body,
      TasksState::Failed(error) => body.child(RecoveryPanel::new(
        Icon::AlertCircle,
        &ctx.t("tasks.loadFailed"),
        &message_text(ctx, error),
      )),
      TasksState::Ready(tasks) if tasks.tasks.is_empty() => body.child(empty_overview(ctx, &props.open)),
      TasksState::Ready(tasks) => {
        let figures = Pulse::of(&tasks.phases, Timestamp::now().local_day_start());
        let online = props.runner == RunnerState::Online;
        body
          .child(pulse::pulse(
            ctx,
            &figures,
            (online, props.agents_working, tasks.lost_to_runner),
            props.usage,
          ))
          .child(
            Row::new()
              .id("overview-tasks-and-attention")
              .width(Dimension::Pct(100.0))
              .flex(1.0)
              .spacing(SpacingSize::Section)
              .child(task_list::task_list(ctx, tasks, props.tab, &self.nav))
              .child(attention::attention(
                ctx,
                &props.open,
                tasks,
                (&self.nav, queue.as_ref()),
              )),
          )
      }
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(project_bar(ctx, &props.open))
      .child(body)
  }
}

fn project_bar(ctx: &Ctx, open: &OpenProject) -> Element {
  Row::new()
    .id("overview-project-bar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PROJECT_BAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(open.project.name().as_str())
        .id("overview-breadcrumb-project")
        .variant(TypographyStyle::Description)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0),
    )
    .child(icon(Icon::ChevronRight, IconSize::Small, PaletteColor::TextMuted).id("overview-breadcrumb-separator"))
    .child(
      Text::new(&ctx.t("nav.overview"))
        .id("overview-breadcrumb-current")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}

fn heading(ctx: &Ctx, open: &OpenProject, nav: &ProjectNav) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .justify(Justify::SpaceBetween)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Sm)
        .child(
          Text::new(open.project.name().as_str())
            .id("overview-project-name")
            .variant(TypographyStyle::Heading)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .child(
          Text::new(&ctx.t("overview.description"))
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(
      ActionButton::new(ctx.t("overview.newTask"))
        .primary()
        .icon(Icon::Plus)
        .id("new-task")
        .height(Dimension::Px(NEW_TASK_HEIGHT))
        .roomy()
        .link(nav.link(Page::NewTask))
        .render(ctx),
    )
    .into()
}

/// No tasks yet: what the project is.
fn empty_overview(ctx: &Ctx, open: &OpenProject) -> Element {
  RecoveryPanel::new(
    Icon::FolderGit,
    &ctx.t("overview.emptyTitle"),
    &ctx.t("overview.emptyDetail"),
  )
  .detail(property(&ctx.t("overview.projectKey"), open.project.key().as_str()))
  .detail(property(
    &ctx.t("overview.repository"),
    &display::shorten_path(&open.repo_root, PATH_CHARS),
  ))
  .into()
}
