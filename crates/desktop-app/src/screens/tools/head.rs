//! Editor head, lifecycle words, actions and tabs (`zsBHu`, editor states `QyM60`).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{editor::EditorProps, words};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, ToolTab},
  theme::{ConnectionsSpacing, ExtraSpacing, ExtraTypography},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    tabs::{PageTab, page_tabs},
  },
};

pub(super) fn head(
  ctx: &mut Ctx,
  p: &EditorProps,
  title: &str,
  state: &(std::sync::Arc<str>, Icon, PaletteColor),
  actions: Vec<Element>,
  reason: Option<&str>,
) -> Column {
  let tools = Navigation::of(ctx).to(Route::Project(p.project.open.project.id(), ProjectPage::Tools));
  let crumbs = breadcrumb(
    ctx,
    "tool-crumbs",
    CrumbSeparator::Chevron,
    vec![Crumb::to(ctx.t("tools.title"), tools), Crumb::here(title)],
  );
  let kind = p
    .entry
    .as_ref()
    .map_or(ctx.t("tools.command"), |e| ctx.t(words::kind(&e.tool).0));
  let glyph = p.entry.as_ref().map_or(Icon::Terminal, |e| words::kind(&e.tool).1);
  let name = Row::new()
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(Text::new(title).id("tool-title").variant(TypographyStyle::Heading))
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, crate::theme::IconSize::Base, PaletteColor::TextSecondary))
        .child(Text::new(&kind).variant(TypographyStyle::Description)),
    );
  let status = Row::new()
    .id("tool-state")
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(state.1, crate::theme::IconSize::Base, state.2.clone()))
    .child(Text::new(&state.0).variant(ExtraTypography::Property.role()));
  let description = p
    .entry
    .as_ref()
    .map_or_else(|| ctx.t("tools.newDetail"), |e| e.tool.description().into());
  let right = Column::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End)
    .child(
      Row::new()
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(status)
        .with_children(actions),
    )
    .with_children(reason.map(|reason| {
      Text::new(reason)
        .id("tool-action-reason")
        .variant(TypographyStyle::Caption)
    }));
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(name)
        .child(Text::new(&description).variant(TypographyStyle::Description)),
    )
    .child(right);
  Column::new()
    .id("tool-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .left(ConnectionsSpacing::Page.role())
        .right(ConnectionsSpacing::Page.role()),
    )
    .spacing(SpacingSize::Md)
    .child(crumbs)
    .child(row)
    .child(tabs(ctx, p))
}
fn tabs(ctx: &mut Ctx, p: &EditorProps) -> Element {
  let current = match p.page {
    ProjectPage::Tool(_, tab) => tab,
    _ => ToolTab::Configuration,
  };
  let project = p.project.open.project.id();
  let nav = Navigation::of(ctx);
  let tabs = ToolTab::ALL
    .into_iter()
    .map(|tab| {
      let label = ctx.t(&format!("tools.tabs.{}", tab.name()));
      let key = format!("tool-tab-{}", tab.name());
      let Some(id) = p.entry.as_ref().map(|e| e.id) else {
        if tab != ToolTab::Configuration {
          return PageTab::unavailable(key, label, ctx.t("tools.saveFirst"));
        }
        return PageTab::to(key, label, true, nav.to(Route::Project(project, ProjectPage::NewTool)));
      };
      let item = PageTab::to(
        key,
        label,
        tab == current,
        nav.to(Route::Project(project, ProjectPage::Tool(id, tab))),
      );
      match tab {
        ToolTab::InputsOutputs | ToolTab::UsageLog => item.explanation(ctx.t("tools.locked")),
        _ => item,
      }
    })
    .collect::<Vec<_>>();
  page_tabs(ctx, "tool-tabs", tabs).into()
}
