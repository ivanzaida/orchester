//! The header of every Modules page (`Module header` in `Modules · redesign` `oEELJ`), the task page's own geometry
//! (`Task header` `j8DakG`): 20 over it and 28 at its sides, its rows 10 apart: the back row (`← Modules / name`;
//! the list's is empty and keeps its height), the title row with the page's actions, the status line, and the tab row
//! 4 under it, so opening a module from the list, or a task from its Summary, moves nothing.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, module::ModuleState};

use super::words;
use crate::{
  navigation::Navigation,
  routes::{ModuleListTab, ProjectPage, Route},
  theme::{ExtraRadius, ExtraSpacing, IconSize, TaskSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    pressable::opening,
    tooltip::Truncated,
  },
};

/// The back row's height, a 12 line at 1.5 as the task page's crumbs lay out: the list's empty row keeps it.
const BACK_ROW_HEIGHT: f32 = 18.0;
/// The state chip's dot (`Dot` `wYW13`).
const DOT: f32 = 6.0;

/// The module's state on its status line (`State` `i4BwJE`): a 6 dot and the word in 11 in the state's tone on its
/// muted fill, a pill no taller than the line's words, 8 at its sides; Open in `success`, Closed in `text_secondary`.
pub fn state_chip(ctx: &Ctx, id: &str, state: ModuleState) -> Element {
  let (tone, fill) = match state {
    ModuleState::Open => (PaletteColor::Success, PaletteColor::SuccessMuted),
    ModuleState::Closed => (PaletteColor::TextSecondary, PaletteColor::AccentMuted),
  };
  Row::new()
    .id(id.to_owned())
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .background(fill)
    .rounded(ExtraRadius::Full.role())
    .child(
      Row::new()
        .size(Dimension::Px(DOT), Dimension::Px(DOT))
        .rounded(ExtraRadius::Full.role())
        .background(tone.clone()),
    )
    .child(
      Text::new(&words::state(ctx, state))
        .variant(TypographyStyle::Caption)
        .color(tone)
        .nowrap(),
    )
    .into()
}

/// What a page's header holds, top to bottom.
pub struct Header {
  /// The header's element id.
  pub id: &'static str,
  /// The way back; `None` on the list, whose row stays empty.
  pub back: Option<Element>,
  /// The title, then the page's actions at the row's end.
  pub title: Element,
  pub actions: Vec<Element>,
  pub status: Element,
  pub tabs: Element,
}

pub fn header(parts: Header) -> Element {
  let back = parts.back.unwrap_or_else(|| {
    Row::new()
      .id("modules-back-row")
      .height(Dimension::Px(BACK_ROW_HEIGHT))
      .into()
  });
  let title_row = Row::new()
    .id(format!("{}-title-row", parts.id))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(parts.title))
    .with_children(parts.actions);
  Column::new()
    .id(parts.id)
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(TaskSpacing::Header.role())
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .child(back)
    .child(title_row)
    .child(parts.status)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(SpacingSize::Xs))
        .child(parts.tabs),
    )
    .into()
}

/// The page's title in 20/600, cut to its room with the whole of it in a tooltip.
pub fn title(ctx: &mut Ctx, id: &str, text: &str) -> Element {
  Truncated::new(id.to_owned(), text.to_owned(), TypographyStyle::Heading).render(ctx)
}

/// `← Modules / name`: the arrow and *Modules* open the list.
pub fn back_row(ctx: &mut Ctx, project: ProjectId, here: &str) -> Element {
  let navigation = Navigation::of(ctx);
  let list = Route::Project(project, ProjectPage::Modules(ModuleListTab::default()));
  let crumbs = vec![
    Crumb::to(ctx.t("modules.page.back"), navigation.to(list.clone())),
    Crumb::here(here.to_owned()),
  ];
  let arrow = opening(navigation.to(list))
    .id("module-back")
    .describe("a11y_name", ctx.t("modules.page.back"))
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted));
  Row::new()
    .id("module-back-row")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(arrow)
    .child(breadcrumb(ctx, "module-crumbs", CrumbSeparator::Slash, crumbs))
    .into()
}

/// A status line: `parts` 8 apart on one line.
pub fn status_line(id: &str, parts: Vec<Element>) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(parts)
    .into()
}
