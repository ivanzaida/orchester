//! Variable catalog (05.03 `EfTiT`, dialog `CMmsI`): the variables by group, searchable by name
//! and description, with the chosen one's type, value in the previewed context and availability
//! (`details`). Insert adds its expression to the prompt; a value missing in the context is shown
//! as unavailable (05.A `m2WjL`), never as empty text.

mod details;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Modal, ModalTarget, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::{Variable, VariableGroup};

use super::{
  editor::InstructionEditor,
  preview::{LoadedCatalog, task_label},
};
use crate::{
  instructions::preview::Resolved,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    field::styled_input,
    icon::{Icon, IconSize, icon},
    nav_item::NavItem,
    pressable::{Ring, keyboard_stop},
  },
};

/// Dialog size, header and footer heights, column widths, search, row and page bar heights, rows per page
/// (`CMmsI`, `jQaAf`, `oFlLX`, `IGagT`, `HMei5`, `O0AWC`, `WH2TF`, `TD4PC`, `br44j`).
const DIALOG_WIDTH: f32 = 1120.0;
const DIALOG_HEIGHT: f32 = 800.0;
const HEADER_HEIGHT: f32 = 128.0;
const FOOTER_HEIGHT: f32 = 56.0;
const CATEGORIES_WIDTH: f32 = 184.0;
const LIST_WIDTH: f32 = 456.0;
const SEARCH_HEIGHT: f32 = 38.0;
const ROW_HEIGHT: f32 = 62.0;
const PAGES_HEIGHT: f32 = 30.0;
const PER_PAGE: usize = 7;

/// Which variables the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupFilter {
  #[default]
  All,
  Group(VariableGroup),
}

impl lurq::app::component::DevtoolsInspectable for GroupFilter {}

/// What the catalog shows: the search, the group, the page and the chosen variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogView {
  pub group: GroupFilter,
  pub page: usize,
  pub selected: Variable,
}

impl lurq::app::component::DevtoolsInspectable for CatalogView {}

impl Default for CatalogView {
  fn default() -> Self {
    Self {
      group: GroupFilter::All,
      page: 0,
      selected: Variable::TaskTitle,
    }
  }
}

impl InstructionEditor {
  pub(super) fn variables_modal(&self, ctx: &Ctx, catalog: &LoadedCatalog) -> Element {
    let closed = || Modal::new(Column::new()).open_when(false).into();
    if !self.variables_open.get() {
      return closed();
    }
    let resolved = match catalog {
      Ok(Some(catalog)) => catalog
        .resolve(&self.preview.get())
        .ok()
        .map(|resolved| (catalog, resolved)),
      _ => None,
    };
    let view = self.catalog_view.get();
    let search = self.catalog_search.get();
    let matching = matching(ctx, &view, &search);
    let body = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(self.categories(ctx, &view))
      .child(self.variable_list(ctx, &view, &matching))
      .child(self.variable_details(ctx, view.selected, resolved.as_ref().map(|(_, resolved)| resolved)));
    let available = resolved
      .as_ref()
      .is_some_and(|(_, resolved)| resolved.value(view.selected).is_some());
    let dialog = Column::new()
      .id("variable-catalog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .height(Dimension::Px(DIALOG_HEIGHT))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .child(self.catalog_header(ctx, resolved.as_ref()))
      .child(body)
      .child(footer(ctx, available));
    Modal::new(backdrop(dialog))
      .target(ModalTarget::Root)
      .open(self.variables_open.clone())
      .into()
  }

  /// Title, the previewed task and the context selects (`jQaAf`).
  fn catalog_header(
    &self,
    ctx: &Ctx,
    resolved: Option<&(&Arc<crate::instructions::preview::PreviewCatalog>, Resolved<'_>)>,
  ) -> Element {
    let project = self.context.project.project.name().as_str();
    let subtitle = match resolved {
      Some((_, resolved)) => ctx.t_args(
        "variables.subtitle",
        [("project", project), ("task", &*task_label(ctx, &resolved.task.task))],
      ),
      None => project.into(),
    };
    let mut title = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("variables.title")).variant(ExtraTypography::DialogTitle.role()))
      .child(Text::new(&subtitle).variant(TypographyStyle::Caption));
    // The previewed context in one row (`C0Y6hE`): task, run, agent and step, event.
    if let Some((catalog, resolved)) = resolved {
      title = title.child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(self.task_select(ctx, catalog, resolved))
          .child(self.run_select(ctx, resolved))
          .child(self.step_select(ctx, resolved))
          .child(self.event_select(ctx)),
      );
    }
    let open = self.variables_open.clone();
    let close = keyboard_stop(Button::empty().button(), Ring::Outer)
      .id("variable-catalog-close")
      .cursor(CursorIcon::Pointer)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_click(move |_| open.set(false))
      .child(icon(Icon::X, IconSize::Notice, PaletteColor::TextSecondary));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEADER_HEIGHT))
      .padding_horizontal(SpacingSize::Lg)
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(title)
      .child(close)
      .into()
  }

  /// All variables and each group, with counts (`IGagT`).
  fn categories(&self, ctx: &Ctx, view: &CatalogView) -> Element {
    let item = |id: &'static str, glyph: Icon, label: Arc<str>, filter: GroupFilter| {
      let count = Variable::ALL
        .iter()
        .filter(|variable| filter == GroupFilter::All || filter == GroupFilter::Group(variable.group()))
        .count();
      let signal = self.catalog_view.clone();
      NavItem::new(id, glyph, label)
        .category()
        .count(Some(count))
        .selected(view.group == filter)
        .on_click(move || {
          signal.update(|view| {
            view.group = filter;
            view.page = 0;
          });
        })
    };
    let groups = VariableGroup::ALL.map(|group| {
      let (id, glyph) = details::group_item(group);
      item(id, glyph, details::group_label(ctx, group), GroupFilter::Group(group))
    });
    Column::new()
      .width(Dimension::Px(CATEGORIES_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Step.role())
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Xs)
      .background(PaletteColor::SurfacePanel)
      .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(item(
        "variables-all",
        Icon::Braces,
        ctx.t("variables.all"),
        GroupFilter::All,
      ))
      .with_children(groups)
      .child(Spacer::new().flex(1.0))
      .child(Text::new(&ctx.t("variables.completeness")).variant(ExtraTypography::Hint.role()))
      .into()
  }

  /// Search, the matching variables a page at a time, and the pages (`HMei5`).
  fn variable_list(&self, ctx: &Ctx, view: &CatalogView, matching: &[Variable]) -> Element {
    let search = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SEARCH_HEIGHT))
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(ExtraSpacing::Field.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfacePanel)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Search, IconSize::Dismiss, PaletteColor::TextSecondary))
      .child(
        styled_input(ctx, self.catalog_search.clone(), TypographyStyle::FieldLabel)
          .id("variable-search")
          .single_line()
          .flex(1.0)
          .placeholder(&ctx.t("variables.search")),
      );
    let label = match view.group {
      GroupFilter::All => ctx.t("variables.all"),
      GroupFilter::Group(group) => details::group_label(ctx, group),
    };
    let total = ctx.t_args(
      "variables.total",
      [("group", &*label), ("count", matching.len().to_string().as_str())],
    );
    let meta = Row::new()
      .width(Dimension::Pct(100.0))
      .justify(Justify::SpaceBetween)
      .child(Text::new(&total).variant(TypographyStyle::Caption))
      .child(
        Text::new(&ctx.t("variables.order"))
          .variant(ExtraTypography::Footnote.role())
          .color(PaletteColor::TextMuted),
      );
    let page = view.page.min(matching.len().saturating_sub(1) / PER_PAGE);
    let rows = matching
      .iter()
      .skip(page * PER_PAGE)
      .take(PER_PAGE)
      .map(|variable| self.variable_row(ctx, *variable, *variable == view.selected));
    Column::new()
      .width(Dimension::Px(LIST_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Step.role())
      .padding_horizontal(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(search)
      .child(meta)
      .child(Column::new().width(Dimension::Pct(100.0)).with_children(rows))
      .child(self.pages(ctx, page, matching.len()))
      .into()
  }

  /// A variable (`ZJ6KN`): its name over its description. Pointer-only, as list rows are.
  fn variable_row(&self, ctx: &Ctx, variable: Variable, selected: bool) -> Element {
    let signal = self.catalog_view.clone();
    let text = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(variable.name())
          .variant(ExtraTypography::MonoField.role())
          .nowrap(),
      )
      .child(
        Text::new(&details::description(ctx, variable))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      );
    let row = Button::empty()
      .id(Arc::<str>::from(format!("variable-{}", variable.name())))
      .focusable(false)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding_horizontal(SpacingSize::Md)
      .align_items(Alignment::Center)
      .cursor(CursorIcon::Pointer)
      .child(text);
    if selected {
      return row
        .rounded(RadiusSize::Sm)
        .background(PaletteColor::AccentMuted)
        .describe("state", "selected")
        .into();
    }
    row
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
      .on_click(move |_| signal.update(|view| view.selected = variable))
      .into()
  }

  /// "1–7 of 25 variables" and the page numbers (`TD4PC`).
  fn pages(&self, ctx: &Ctx, page: usize, total: usize) -> Element {
    let pages = total.div_ceil(PER_PAGE).max(1);
    let from = if total == 0 { 0 } else { page * PER_PAGE + 1 };
    let to = ((page + 1) * PER_PAGE).min(total);
    let range = ctx.t_args(
      "variables.range",
      [
        ("from", from.to_string().as_str()),
        ("to", to.to_string().as_str()),
        ("total", total.to_string().as_str()),
      ],
    );
    let step = |id: &'static str, glyph: Icon, target: Option<usize>| {
      let signal = self.catalog_view.clone();
      let button = Button::empty()
        .id(id)
        .focusable(false)
        .padding_horizontal(SpacingSize::Xs)
        .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary));
      match target {
        Some(target) => button
          .cursor(CursorIcon::Pointer)
          .on_click(move |_| signal.update(|view| view.page = target)),
        None => button.cursor(CursorIcon::NotAllowed),
      }
    };
    let numbers = (0..pages).map(|index| {
      let signal = self.catalog_view.clone();
      let text = Text::new(&(index + 1).to_string())
        .variant(ExtraTypography::MonoValue.role())
        .color(if index == page {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        });
      Button::empty()
        .id(Arc::<str>::from(format!("variables-page-{}", index + 1)))
        .focusable(false)
        .padding_horizontal(SpacingSize::Xs)
        .cursor(CursorIcon::Pointer)
        .on_click(move |_| signal.update(|view| view.page = index))
        .child(text)
    });
    let controls = Row::new()
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(step("variables-page-previous", Icon::ChevronLeft, page.checked_sub(1)))
      .with_children(numbers)
      .child(step(
        "variables-page-next",
        Icon::ChevronRight,
        (page + 1 < pages).then_some(page + 1),
      ));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(PAGES_HEIGHT))
      .align_items(Alignment::Center)
      .justify(Justify::SpaceBetween)
      .child(Text::new(&range).variant(ExtraTypography::Footnote.role()))
      .child(controls)
      .into()
  }
}

/// Variables of the view's group whose name or description holds the search, in catalog order.
fn matching(ctx: &Ctx, view: &CatalogView, search: &str) -> Vec<Variable> {
  let search = search.trim().to_lowercase();
  Variable::ALL
    .into_iter()
    .filter(|variable| view.group == GroupFilter::All || view.group == GroupFilter::Group(variable.group()))
    .filter(|variable| {
      search.is_empty()
        || variable.name().contains(&search)
        || details::description(ctx, *variable).to_lowercase().contains(&search)
    })
    .collect()
}

fn footer(ctx: &Ctx, available: bool) -> Element {
  let hint = if available {
    "variables.hint"
  } else {
    "variables.unavailableHint"
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding_horizontal(SpacingSize::Lg)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(Icon::Info, IconSize::Dismiss, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t(hint)).variant(TypographyStyle::Caption))
    .into()
}

/// Inserts `variable`'s expression at the end of `text`, apart from the word before it.
pub(super) fn with_variable(text: &str, variable: Variable) -> String {
  let expression = format!("{{{{{}}}}}", variable.name());
  if text.is_empty() || text.ends_with(char::is_whitespace) {
    format!("{text}{expression}")
  } else {
    format!("{text} {expression}")
  }
}
