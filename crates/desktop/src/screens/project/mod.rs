//! Project shell: the sidebar (`zo9Py`) beside the project overview (`Twtqc`). Tasks, runs and
//! agents do not exist yet, so the overview shows what the project is and says so, and the
//! state of the project's runner.

mod runner;
mod sidebar;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use self::sidebar::sidebar;
use crate::{
  display,
  runner::{RunnerStatus, Runners, state::RunnerState},
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    panel::RecoveryPanel,
  },
  workspace::OpenProject,
};

/// Project bar height (`CsB3h`) and New task button height (`nMeVq`).
const PROJECT_BAR_HEIGHT: f32 = 64.0;
const NEW_TASK_HEIGHT: f32 = 38.0;
/// Longest repository path shown in full in the overview panel.
const PATH_CHARS: usize = 48;

pub struct ProjectShell;

impl Component for ProjectShell {
  type Props = OpenProject;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting screens");
    let status = ctx.stream(
      runners.clone(),
      |runners, emitter: StreamEmitter<RunnerStatus, ()>| async move {
        let mut updates = runners.subscribe();
        loop {
          let status = updates.borrow_and_update().clone();
          if !emitter.emit(status) || updates.changed().await.is_err() {
            return;
          }
        }
      },
    );
    let runner = status
      .state()
      .get()
      .data
      .unwrap_or_else(|| runners.status())
      .state_for(&open.repo_root);
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(sidebar(ctx, &open, &runner))
      .child(
        Column::new()
          .flex(1.0)
          .height(Dimension::Pct(100.0))
          .child(project_bar(ctx, &open))
          .child(overview(ctx, &open, &runner, &runners)),
      )
  }
}

fn project_bar(ctx: &Ctx, open: &OpenProject) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PROJECT_BAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(open.project.name().as_str()).variant(TypographyStyle::Description))
    .child(icon(Icon::ChevronRight, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("nav.overview"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}

fn overview(ctx: &Ctx, open: &OpenProject, runner: &RunnerState, runners: &Runners) -> Element {
  let heading = Row::new()
    .width(Dimension::Pct(100.0))
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(Text::new(open.project.name().as_str()).variant(TypographyStyle::Heading))
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
        .render(ctx),
    );
  let content = runner::offline_panel(ctx, runner, runners).unwrap_or_else(|| empty_overview(ctx, open));
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Rail.role())
    .child(heading)
    .with_children(runner::reconnect_notice(ctx, runner))
    .child(content)
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

/// Key/value property (`tfGRt`).
fn property(label: &str, value: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .justify(Justify::SpaceBetween)
    .child(Text::new(label).variant(TypographyStyle::Caption).nowrap())
    .child(
      Text::new(value)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .selectable(true),
    )
    .into()
}
