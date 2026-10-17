//! Active runs in Used in pipelines (v7YbXO in uIMpt).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{agents::usage::ActiveUse, display};

use super::{editor::Props, words};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, agents::active_task_text},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
    tooltip::Truncated,
  },
};

/// l5Pjj row and GgnX4 retention note in v7YbXO.
const ROW_HEIGHT: f32 = 48.0;
const NOTE_WIDTH: f32 = 170.0;

pub fn render(ctx: &mut Ctx, props: &Props) -> Element {
  let active: Vec<_> = props
    .listed
    .usage
    .active
    .iter()
    .filter(|r| props.entry.as_ref().is_some_and(|p| p.id == r.profile))
    .collect();
  let section = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("agents.activeTitle")).variant(TypographyStyle::Title))
        .child(
          Text::new(&active.len().to_string())
            .id("agent-active-count")
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextMuted),
        ),
    );
  if active.is_empty() {
    return section.child(words::note(&ctx.t("agents.noActive"))).into();
  }
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Lg)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border));
  for (index, run) in active.iter().enumerate() {
    list = list.child(run_row(ctx, props, index, run));
  }
  section.child(list).into()
}

fn run_row(ctx: &mut Ctx, props: &Props, index: usize, run: &ActiveUse) -> Element {
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    props.project.open.project.id(),
    ProjectPage::Task(run.task.clone(), TaskView::Summary),
  );
  let title = ctx.t_args(
    "agents.activeTask",
    [("key", run.task.to_string()), ("title", run.title.to_string())],
  );
  let version = run.version.get().to_string();
  let profile = match &run.profile_name {
    Some(name) => ctx.t_args("agents.activeProfile", [("name", name.as_ref()), ("version", &version)]),
    None => ctx.t_args("agents.activeVersion", [("version", &version)]),
  };
  opening(navigation.to(route))
    .id(format!("agent-active-{index}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .rounded(RadiusSize::Lg)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Truncated::new(format!("agent-active-title-{index}"), title, active_task_text()).render(ctx))
        .child(
          Truncated::new(
            format!("agent-active-detail-{index}"),
            metadata(ctx, run),
            ExtraTypography::Note.role(),
          )
          .render(ctx),
        ),
    )
    .child(
      Text::new(&profile)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(
      Row::new().width(Dimension::Px(NOTE_WIDTH)).justify(Justify::End).child(
        Text::new(&ctx.t_args("agents.keepsVersion", [("version", version)]))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      ),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Md, PaletteColor::TextMuted))
    .into()
}

fn metadata(ctx: &Ctx, run: &ActiveUse) -> std::sync::Arc<str> {
  let args = [
    ("pipeline", run.pipeline.to_string()),
    ("version", run.pipeline_version.get().to_string()),
    ("position", format!("{:02}", run.position)),
    ("step", run.step.to_string()),
  ];
  match run.started_at {
    Some(at) => ctx.t_args(
      "agents.activeWhereStarted",
      args.into_iter().chain([("time", display::clock(at))]),
    ),
    None => ctx.t_args("agents.activeWhere", args),
  }
}
