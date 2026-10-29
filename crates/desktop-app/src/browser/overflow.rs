//! The overflow button and its menu (`01d · Tab overflow` `fWhIr`): before *+*, a chevron, how many tabs are not
//! wholly in view (*⌄ 13*) and the strongest dot among them; it opens a menu of every tab of the project: a filter
//! field, one row per tab with its dot, its full label cut only at the menu's width and its run on the right, × under
//! the pointer, the person's tabs after a divider. Choosing a row shows its tab, which the strip scrolls into view.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Placement, Popup, Row, ScrollVertical, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::BrowserCommand;

use super::{
  model::{Indicator, Owner, StripTab},
  state::Showing,
  tab::{TAB_HEIGHT, inline_dot, label, tab_icon},
};
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::FieldInput,
    icon::{Icon, icon},
    menu::{menu_panel, separator},
    pressable::{in_title_bar, pressable},
  },
};

/// The menu (`Overflow menu` in `fWhIr`) and the most of it that shows before its rows scroll.
const MENU_WIDTH: f32 = 440.0;
const MENU_MAX_HEIGHT: f32 = 560.0;

#[derive(Clone, PartialEq)]
pub struct OverflowProps {
  /// How many tabs are not wholly in view.
  pub hidden: usize,
  pub strongest: Option<Indicator>,
  pub showing: Showing,
}

impl DevtoolsInspectable for OverflowProps {}

pub struct Overflow {
  open: Signal<bool>,
  anchor: ElementRef,
  filter: Signal<String>,
}

impl Component for Overflow {
  type Props = OverflowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
      filter: ctx.signal(String::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let OverflowProps {
      hidden,
      strongest,
      showing,
    } = ctx.props::<OverflowProps>().clone();
    let (open, filter) = (self.open.clone(), self.filter.clone());
    let count_name = if hidden == 1 {
      ctx.t_args("browser.overflow.button.one", [("count", hidden.to_string())])
    } else {
      ctx.t_args("browser.overflow.button.other", [("count", hidden.to_string())])
    };
    let button = in_title_bar(pressable(move || {
      filter.set(String::new());
      open.update(|open| *open = !*open);
    }))
    .id("browser-overflow")
    .ref_element(self.anchor.clone())
    .describe("a11y_name", count_name)
    .describe("count", hidden.to_string())
    .describe("indicator", strongest.map_or("none", Indicator::name))
    .height(Dimension::Px(TAB_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&hidden.to_string())
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .with_children(strongest.map(|indicator| inline_dot(indicator, "browser-overflow-dot".to_owned())));
    let button = if self.open.get() {
      button.background(ExtraColor::Hover.role())
    } else {
      button
    };
    let menu = self.open.get().then(|| self.menu(ctx, &showing));
    let popup = Popup::new(self.anchor.clone(), menu.unwrap_or_else(Element::new))
      .open(self.open.clone())
      .placement(Placement::BottomEnd)
      .offset(0.0, ExtraSpacing::MenuOffset.value());
    Row::new().flex_shrink(0.0).child(button).child(popup)
  }
}

impl Overflow {
  fn menu(&self, ctx: &mut Ctx, showing: &Showing) -> Element {
    let filter = self.filter.get().to_lowercase();
    let rows = |tabs: &[StripTab], offset: usize, ctx: &mut Ctx| -> Vec<Element> {
      let mut rows = Vec::new();
      for (index, tab) in tabs.iter().enumerate() {
        if filter.is_empty() || label(ctx, tab).to_lowercase().contains(&filter) {
          rows.push(self.row(ctx, showing, tab, offset + index));
        }
      }
      rows
    };
    let agents = rows(&showing.strip.agents, 0, ctx);
    let person = rows(&showing.strip.person, showing.strip.agents.len(), ctx);
    let divider = (!agents.is_empty() && !person.is_empty()).then(|| separator(ctx));
    let none = (agents.is_empty() && person.is_empty()).then(|| {
      Row::new()
        .height(Dimension::Px(ControlHeight::Md.value()))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("browser.overflow.none")).variant(TypographyStyle::Caption))
    });
    let field = FieldInput::new(
      "browser-overflow-filter",
      ctx.t("browser.overflow.filter"),
      self.filter.clone(),
    )
    .placeholder(ctx.t("browser.overflow.filter"))
    .search()
    .render(ctx);
    let rows = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .with_children(agents)
      .with_children(divider)
      .with_children(person)
      .with_children(none);
    menu_panel("browser-overflow-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .child(field)
      .child(
        ScrollVertical::new(rows)
          .width(Dimension::Pct(100.0))
          .max_height(Dimension::Px(MENU_MAX_HEIGHT)),
      )
      .into()
  }

  /// A tab's row: its icon and dot, its label, its run, × under the pointer.
  fn row(&self, ctx: &mut Ctx, showing: &Showing, tab: &StripTab, index: usize) -> Element {
    let id = format!("browser-overflow-{index}");
    let run = match &tab.owner {
      Owner::Run { known: Some(known), .. } => Some(
        ctx
          .t_args("browser.overflow.run", [("number", known.number.get().to_string())])
          .to_string(),
      ),
      _ => None,
    };
    let shown = showing.shown() == Some(&tab.id);
    let (open, hooks, picked) = (self.open.clone(), showing.person.clone(), tab.id.clone());
    let (commands, closed) = (showing.commands.clone(), tab.id.clone());
    let close = Button::empty()
      .id(format!("{id}-close"))
      .describe("a11y_name", ctx.t("browser.tabs.close"))
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        commands.send(BrowserCommand::CloseTab { tab: closed.clone() });
      })
      .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted));
    let label = label(ctx, tab);
    let row = pressable(move || {
      open.set(false);
      hooks.pick(&picked);
    })
    .id(id.clone())
    .describe("a11y_name", label.clone())
    .describe("chosen", if shown { "true" } else { "false" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(tab_icon(tab, PaletteColor::TextMuted, &id))
    .child(
      Text::new(&label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(run.map(|run| Text::new(&run).variant(ExtraTypography::Note.role()).nowrap()))
    .child(close);
    if shown {
      row.background(ExtraColor::Hover.role()).into()
    } else {
      row.into()
    }
  }
}
