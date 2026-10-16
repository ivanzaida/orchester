//! A row of the tab list (`Tab list row` `uCIaF`, `Tab list row/current` `zDB5B`): 28 high, radius 4, 8
//! before and 4 after its content, 8 apart: the tab's 14 icon, its page in 12 `text_primary`, its
//! project in 12 `text_muted` filling the row, then an `eye-off` 12 while the tab is not in the strip,
//! the mark of a project that needs the person, a 14 check on the current tab, and a 20 slot that holds
//! the close button while the pointer is over the row. The current row is filled with `accent_muted`
//! and says its page in 500; a hovered one takes `hover`; the keyboard's row takes `hover` inside a 1
//! `border_focus` stroke (`a1i6Ws`). A hovered close button takes `pressed`, its × `text_primary`, and
//! says *Close tab*.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{MouseButton, MouseEvent},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  chrome::tab_label::{attention, tab_icon},
  navigation::{Navigation, tabs::Tab},
  project::Projects,
  theme::{ControlHeight, ExtraColor, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// The slot that keeps the close button's place (`Close` `CHx8i`).
const CLOSE_SLOT: f32 = 20.0;

/// What switches to the row's tab and closes the list.
pub type Choose = Arc<dyn Fn() + Send + Sync>;

/// A row: its tab, what it says and how it stands.
#[derive(Clone)]
pub struct RowProps {
  pub tab: Tab,
  pub page: Arc<str>,
  pub project: Option<Arc<str>>,
  /// The tab is not in the strip.
  pub hidden: bool,
  pub current: bool,
  /// The keyboard's row.
  pub ringed: bool,
  pub closable: bool,
  /// Kept in view while the list scrolls.
  pub kept: Option<ElementRef>,
  pub choose: Choose,
}

/// Compared by what it shows; the closure and the ref are the list's for its life.
impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.tab == other.tab
      && self.page == other.page
      && self.project == other.project
      && self.hidden == other.hidden
      && self.current == other.current
      && self.ringed == other.ringed
      && self.closable == other.closable
      && self.kept.is_some() == other.kept.is_some()
  }
}

impl DevtoolsInspectable for RowProps {}

pub struct ListRow {
  hovered: Signal<bool>,
  close_hovered: Signal<bool>,
}

impl Component for ListRow {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
      close_hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let id = format!("tab-list-{}", props.tab.id());
    let projects = Projects::of(ctx);
    let navigation = Navigation::of(ctx);
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let (closer, tab) = (navigation.clone(), props.tab.id());
    let closable = props.closable;
    let choose = props.choose.clone();
    let row = pressable(move || choose())
      .id(id.clone())
      .describe("a11y_name", props.page.clone())
      .describe("state", if props.current { "current" } else { "other" })
      .describe("hidden", if props.hidden { "true" } else { "false" })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .flex_shrink(0.0)
      .padding(Padding::new().left(SpacingSize::Sm).right(SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .rounded(RadiusSize::Sm)
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .on_mouse_up(move |event: MouseEvent| {
        if event.button == MouseButton::Middle && closable {
          event.prevent_default();
          closer.close(tab);
        }
      })
      .child(icon(
        glyph(&props.tab),
        IconSize::Md,
        if props.current {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        },
      ))
      .with_children(words(&props))
      .with_children(
        props
          .hidden
          .then(|| icon(Icon::EyeOff, IconSize::Sm, PaletteColor::TextMuted).id(format!("{id}-hidden"))),
      )
      .with_children(attention(ctx, &projects, &props.tab, format!("{id}-attention")))
      .with_children(
        props
          .current
          .then(|| icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary).id(format!("{id}-current"))),
      )
      .child(self.close_slot(ctx, &props, navigation));
    let row = match &props.kept {
      Some(kept) => row.ref_element(kept.clone()),
      None => row,
    };
    let row = if props.current {
      row.class("current").background(PaletteColor::AccentMuted)
    } else if props.ringed {
      row.background(ExtraColor::Hover.role())
    } else {
      row.hovered_style(Style::new().background(ExtraColor::Hover.role()))
    };
    if props.ringed {
      row
        .class("ringed")
        .border_inside(BorderSize::Sm, PaletteColor::BorderFocus)
    } else {
      row
    }
  }
}

impl ListRow {
  /// The slot at the row's end: the close button while the pointer is over a row whose tab can close.
  fn close_slot(&self, ctx: &mut Ctx, props: &RowProps, navigation: Navigation) -> Element {
    let slot = Row::new()
      .width(Dimension::Px(CLOSE_SLOT))
      .height(Dimension::Px(CLOSE_SLOT))
      .flex_shrink(0.0);
    if !(props.closable && self.hovered.get()) {
      return slot.into();
    }
    let id = format!("tab-list-{}-close", props.tab.id());
    let tab = props.tab.id();
    let (enter, leave) = (self.close_hovered.clone(), self.close_hovered.clone());
    let hovered = self.close_hovered.get();
    let label = ctx.t("shell.tabs.close");
    let button = pressable(move || navigation.close(tab))
      .id(id.clone())
      .describe("a11y_name", label.clone())
      .width(Dimension::Px(CLOSE_SLOT))
      .height(Dimension::Px(CLOSE_SLOT))
      .flex_shrink(0.0)
      .justify(Justify::Center)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(icon(
        Icon::X,
        IconSize::Sm,
        if hovered {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextMuted
        },
      ));
    let button = if hovered {
      button.background(ExtraColor::Pressed.role())
    } else {
      button
    };
    Tooltip::new(id, label).wrap(ctx, button)
  }
}

/// The row's page and, after it, its project filling the row; both end in an ellipsis when cut.
fn words(props: &RowProps) -> [Element; 2] {
  let page = Text::new(&props.page)
    .variant(if props.current {
      TypographyStyle::Label
    } else {
      TypographyStyle::Description
    })
    .color(PaletteColor::TextPrimary)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0));
  let project = Text::new(props.project.as_deref().unwrap_or_default())
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextMuted)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  [page.into(), project.into()]
}

/// The tab's glyph: the folders for All projects, else its page's ([`tab_icon`]).
fn glyph(tab: &Tab) -> Icon {
  if tab.route().is_home() {
    Icon::Folders
  } else {
    tab_icon(tab.route())
  }
}
