//! Tool library (06.01 `ZFwWl`): the project's tools with their type, secrets and status, who owns
//! them, the pipelines that call them, and the kinds of tool that can be created. Only command
//! tools can be created in this version; filtering and importing do not exist yet, and the other
//! kinds need project secrets or approvals, so they are unavailable.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{super::ProjectNav, row::tool_row};
use crate::{
  navigation::{
    import::ImportTab,
    route::{Page, PipelineView, ToolSelection},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  tools::data::ProjectTools,
  ui::{
    button::ActionButton,
    check_row::check_row,
    field::{ValueTone, value_box_sized},
    icon::Icon,
    notice::ownership_note,
    panel::RecoveryPanel,
    pressable::surface,
  },
  workspace::OpenProject,
};

/// Topbar, type filter and side panel sizes (`GIQmD`, `wVmwl`, `VCqC0`), and the height of the
/// side panel's choices (`oeTlM`).
const TOPBAR_HEIGHT: f32 = 64.0;
const FILTER_WIDTH: f32 = 150.0;
const FILTER_HEIGHT: f32 = 34.0;
const SIDE_PANEL_WIDTH: f32 = 268.0;
const CHOICE_HEIGHT: f32 = 40.0;

pub(super) fn library(
  ctx: &Ctx,
  open: &OpenProject,
  tools: &ProjectTools,
  (nav, scroll): (&ProjectNav, &ScrollState),
) -> Element {
  let project = open.project.name().as_str();
  let content = if tools.tools.is_empty() {
    empty(ctx, nav)
  } else {
    inventory(ctx, project, tools, (nav, scroll))
  };
  let body = Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Section)
    .spacing(SpacingSize::Section)
    .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(content))
    .child(create_panel(ctx, nav));
  Column::new()
    .id("tools-library")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(topbar(ctx, project, nav))
    .child(heading(ctx, project))
    .child(body)
    .into()
}

/// Breadcrumb, Import from project and Add tool (`GIQmD`). Add tool writes a command tool, the
/// only kind that can be created.
fn topbar(ctx: &Ctx, project: &str, nav: &ProjectNav) -> Element {
  let importing = nav.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t_args("tools.breadcrumb", [("project", project)])).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(ctx.t("tools.import"))
        .icon(Icon::Download)
        .id("tools-import")
        .on_click(move || importing.show_import(ImportTab::Tools))
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("tools.add"))
        .primary()
        .icon(Icon::Plus)
        .id("tools-add")
        .link(nav.link(Page::Tools(ToolSelection::New)))
        .render(ctx),
    )
    .into()
}

/// Title and description (`qqmHu`).
fn heading(ctx: &Ctx, project: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("tools.title")).variant(ExtraTypography::ScreenTitle.role()))
    .child(
      Text::new(&ctx.t_args("tools.description", [("project", project)]))
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    )
    .into()
}

/// Filter and counts, the rows, who owns the tools and where pipelines call them (`lWH6n`).
fn inventory(ctx: &Ctx, project: &str, tools: &ProjectTools, (nav, scroll): (&ProjectNav, &ScrollState)) -> Element {
  let key = if tools.tools.len() == 1 {
    "tools.counts.one"
  } else {
    "tools.counts.other"
  };
  let filters = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FILTER_HEIGHT))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(Column::new().width(Dimension::Px(FILTER_WIDTH)).child(value_box_sized(
      ctx,
      "tools-filter",
      &ctx.t("tools.allTypes"),
      ValueTone::Unavailable,
      None,
      Dimension::Px(FILTER_HEIGHT),
    )))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t_args(key, [("count", tools.tools.len().to_string().as_str())]))
        .variant(TypographyStyle::Caption)
        .id("tools-counts"),
    );
  let rows = tools
    .tools
    .iter()
    .map(|entry| tool_row(ctx, entry, nav.link(Page::Tools(ToolSelection::Tool(entry.id)))));
  let list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .id("tools-list")
        .width(Dimension::Pct(100.0))
        .with_children(rows),
    )
    .child(ownership_note(&ctx.t_args("tools.ownership", [("project", project)])))
    .child(usage(ctx, tools, nav));
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(SpacingSize::Md)
    .child(filters)
    .child(
      ScrollVertical::new(list)
        .with_scroll_state(scroll.clone())
        .width(Dimension::Pct(100.0))
        .flex(1.0),
    )
    .into()
}

/// Pipelines whose published version calls tools (`my6Kx`), each opening in its editor.
fn usage(ctx: &Ctx, tools: &ProjectTools, nav: &ProjectNav) -> Element {
  let pipelines = tools.pipelines();
  let mut column = Column::new()
    .id("tools-usage")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .child(Text::new(&ctx.t("tools.usage.title")).variant(ExtraTypography::SectionHeading.role()));
  if pipelines.is_empty() {
    return column
      .child(Text::new(&ctx.t("tools.usage.none")).variant(TypographyStyle::Caption))
      .into();
  }
  for pipeline in pipelines {
    let key = if pipeline.tools == 1 {
      "tools.usage.countOne"
    } else {
      "tools.usage.countOther"
    };
    let count = pipeline.tools.to_string();
    let version = pipeline.version.get().to_string();
    let result = ctx.t_args(key, [("count", count.as_str()), ("version", version.as_str())]);
    let row = surface()
      .width(Dimension::Pct(100.0))
      .describe("pipeline", pipeline.name.to_string())
      .child(check_row(Icon::Workflow, &pipeline.name, &result));
    column = column.child(nav.link(Page::Pipeline(pipeline.pipeline, PipelineView::Draft)).on(row));
  }
  column.into()
}

/// No tools yet: what a command tool is, and writing one.
fn empty(ctx: &Ctx, nav: &ProjectNav) -> Element {
  let action = ActionButton::new(ctx.t("tools.empty.add"))
    .primary()
    .icon(Icon::Terminal)
    .id("tools-empty-add")
    .link(nav.link(Page::Tools(ToolSelection::New)))
    .render(ctx);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Lg)
    .child(RecoveryPanel::new(Icon::Wrench, &ctx.t("tools.empty.title"), &ctx.t("tools.empty.detail")).action(action))
    .into()
}

/// The kinds of tool (`VCqC0`): Command writes a new command tool; the others are not available.
fn create_panel(ctx: &Ctx, nav: &ProjectNav) -> Element {
  let choice = |key: &str, glyph: Icon, id: &'static str| {
    ActionButton::new(ctx.t(key))
      .icon(glyph)
      .id(id)
      .start()
      .fill_width()
      .height(Dimension::Px(CHOICE_HEIGHT))
  };
  Column::new()
    .id("tools-create")
    .width(Dimension::Px(SIDE_PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_left(SpacingSize::Lg)
    .spacing(ExtraSpacing::Step.role())
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("tools.create.title")).variant(ExtraTypography::SectionHeading.role()))
    .child(Text::new(&ctx.t("tools.create.description")).variant(ExtraTypography::Explanation.role()))
    .child(choice("tools.create.http", Icon::Globe, "tools-create-http").render(ctx))
    .child(
      choice("tools.create.command", Icon::Terminal, "tools-create-command")
        .link(nav.link(Page::Tools(ToolSelection::New)))
        .render(ctx),
    )
    .child(choice("tools.create.mcp", Icon::Plug, "tools-create-mcp").render(ctx))
    .child(choice("tools.create.database", Icon::Database, "tools-create-database").render(ctx))
    .child(
      choice(
        "tools.create.pullRequest",
        Icon::GitPullRequest,
        "tools-create-pull-request",
      )
      .render(ctx),
    )
    .child(Text::new(&ctx.t("tools.create.unavailable")).variant(ExtraTypography::Detail.role()))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("tools.create.credentials")).variant(ExtraTypography::Detail.role()))
    .into()
}
