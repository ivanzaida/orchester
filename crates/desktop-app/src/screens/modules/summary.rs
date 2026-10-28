//! A module's Summary (`02 · Module · Summary` `KpLYv`), laid out as the task's (`Body` `e0k4N`): one page that
//! scrolls whole, 24 under the tabs and 28 at its sides. In its column, 28 apart: the description as the task's brief
//! (13 `text_secondary` at 1.6 under *Description*), the module's tasks by workflow status ([`super::tasks`]), and the
//! burndown line on `surface_raised` (`Burndown summary` `H1KiaK`: a 14 `chart-line`, *4 remaining* in 13/500, the
//! forecast in 12 `text_muted`, *Burndown →* in 12 `text_secondary`), which opens the Burndown tab. Beside it, 40
//! away, the properties ([`super::properties`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_workspace::modules::data::ModulePage;

use super::{
  page::body_padding,
  properties::{ModuleProperties, PropertiesProps},
  save::Written,
  tasks::{self, Toggled},
  words,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ModuleTab, ProjectPage, Route},
  screens::task::summary::section_head,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, TaskSpacing, TaskText},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
  },
};

#[derive(Clone)]
pub struct SummaryProps {
  pub project: ShownProject,
  pub page: Arc<ModulePage>,
  /// The page's, for as long as it shows.
  pub written: Written,
}

impl PartialEq for SummaryProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && Arc::ptr_eq(&self.page, &other.page)
  }
}

impl DevtoolsInspectable for SummaryProps {}

pub struct ModuleSummary {
  /// The task groups the person opened or folded.
  toggled: Signal<Toggled>,
}

impl Component for ModuleSummary {
  type Props = SummaryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      toggled: ctx.signal(Toggled::default()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    let page = props.page.clone();
    let project = props.project.open.project.id();
    let column = Column::new()
      .id("module-summary-column")
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(TaskSpacing::Sections.role())
      .child(description(ctx, &page))
      .child(tasks::section(ctx, project, &page.tasks, &self.toggled))
      .child(burndown_line(ctx, (&props.project, &page)));
    let properties = ctx.mount_keyed::<ModuleProperties>(
      "module-properties-component",
      PropertiesProps {
        project: props.project.clone(),
        module: Arc::new(page.module.clone()),
        written: props.written.clone(),
      },
    );
    let body = Row::new()
      .id("module-summary-body")
      .width(Dimension::Pct(100.0))
      .padding(body_padding())
      .spacing(TaskSpacing::Columns.role())
      .align_items(Alignment::Start)
      .child(column)
      .child(properties);
    Column::new()
      .id("module-summary")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(ScrollVertical::new(body).id("module-scroll").flex(1.0))
  }
}

/// *Description* and what the module delivers, as the task's brief reads.
fn description(ctx: &Ctx, page: &ModulePage) -> Element {
  let words = page.module.details().description();
  let text = if words.is_empty() {
    Text::new(&ctx.t("modules.summary.noDescription")).color(PaletteColor::TextMuted)
  } else {
    Text::new(words)
  };
  Column::new()
    .id("module-description")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(section_head(
      ctx,
      "module-description-head",
      "modules.summary.description",
      None,
    ))
    .child(
      text
        .id("module-description-text")
        .variant(TaskText::Brief.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// `4 remaining · forecast · Burndown →`, opening the Burndown tab.
fn burndown_line(ctx: &mut Ctx, (project, page): (&ShownProject, &ModulePage)) -> Element {
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    project.open.project.id(),
    ProjectPage::Module(page.module.id(), ModuleTab::Burndown),
  );
  let remaining = ctx.t_args(
    "modules.summary.remaining",
    [("count", page.progress.remaining().to_string())],
  );
  Press::Open(navigation.to(route))
    .button()
    .id("module-burndown-line")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::ChartLine, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&remaining)
        .id("module-burndown-remaining")
        .variant(ExtraTypography::Strong.role())
        .nowrap(),
    )
    .child(
      Text::new(&words::forecast_line(ctx, &page.forecast))
        .id("module-burndown-forecast-line")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("modules.summary.toBurndown"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .into()
}
