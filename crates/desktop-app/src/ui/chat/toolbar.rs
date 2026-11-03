//! What a message's toolbar offers besides Copy ([`super::copy`]; ORC-330, `Message actions` `Fh2UW` in 09 `GYAPg`,
//! its states in 09d `Pe6Di`): *Edit* with its label before Copy on the person's own message (a 12 `pencil` and
//! 11.5 `text_primary`, 5 apart, 8 at its sides), and `…` after it, which opens the message's menu (`Message menu`
//! `N8Eh1I`, 200 wide) under its end, `pressed` while it is open. Each is 22 high in the toolbar, the `hover` fill
//! under the pointer. *Edit* while it cannot be used now is at half and says why in its tooltip (*Stop the answer to
//! edit or rewind*); so does a row of the menu, while `…` still opens it for the rows that can (*Fork from here*,
//! ORC-334, works while the Orchestrator answers). Copy still works.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu_at_end, menu_panel},
    pressable::{Press, inert},
    tooltip::Tooltip,
  },
};

/// The message menu's width (`N8Eh1I`).
const MENU_WIDTH: f32 = 200.0;
/// The class of *Edit* or `…` while it cannot be pressed, for tests and QA.
pub const HELD_CLASS: &str = "toolbar-held";

/// What a choice does, or why it cannot be made now.
pub type Choice = Result<Arc<dyn Fn() + Send + Sync>, Arc<str>>;

/// What a message's toolbar holds besides Copy.
#[derive(Clone)]
pub struct MessageExtras {
  /// *Edit*, before Copy.
  pub edit: Option<ToolbarChoice>,
  /// `…` and its menu, after Copy.
  pub more: Option<MoreActions>,
}

/// A labelled choice of the toolbar.
#[derive(Clone)]
pub struct ToolbarChoice {
  pub label: Arc<str>,
  pub choose: Choice,
}

/// The `…` button, named `label` for tools and its tooltip, and the rows of its menu.
#[derive(Clone)]
pub struct MoreActions {
  pub label: Arc<str>,
  pub rows: Vec<MoreRow>,
}

/// A row of the message's menu: its element id is the toolbar's id and `-<key>`.
#[derive(Clone)]
pub struct MoreRow {
  pub key: &'static str,
  pub label: Arc<str>,
  pub icon: Icon,
  pub choose: Choice,
}

/// Compared by what they show; what a choice does is its caller's.
impl PartialEq for MessageExtras {
  fn eq(&self, other: &Self) -> bool {
    let choice = |one: &Choice, two: &Choice| match (one, two) {
      (Ok(_), Ok(_)) => true,
      (Err(one), Err(two)) => one == two,
      _ => false,
    };
    let edits = match (&self.edit, &other.edit) {
      (Some(one), Some(two)) => one.label == two.label && choice(&one.choose, &two.choose),
      (None, None) => true,
      _ => false,
    };
    let mores = match (&self.more, &other.more) {
      (Some(one), Some(two)) => {
        one.label == two.label
          && one.rows.len() == two.rows.len()
          && one.rows.iter().zip(&two.rows).all(|(one, two)| {
            one.key == two.key && one.label == two.label && one.icon == two.icon && choice(&one.choose, &two.choose)
          })
      }
      (None, None) => true,
      _ => false,
    };
    edits && mores
  }
}

/// *Edit* of the toolbar `id` (`<id>-edit`).
pub(super) fn edit_button(ctx: &mut Ctx, id: &str, edit: &ToolbarChoice) -> Element {
  let id = format!("{id}-edit");
  let button = Button::empty()
    .id(id.clone())
    .height(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .describe("a11y_name", edit.label.clone())
    .child(icon(Icon::Pencil, IconSize::Sm, PaletteColor::TextPrimary))
    .child(
      Text::new(&edit.label)
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    );
  match &edit.choose {
    Ok(choose) => Press::Run(choose.clone())
      .on(hovered(button))
      .describe("state", "enabled")
      .into(),
    Err(why) => held(ctx, &id, button, why),
  }
}

/// The `…` of the toolbar `id` (`<id>-more`) and the menu it opens while `open` holds, under its end at `anchor`.
pub(super) fn more_button(
  ctx: &mut Ctx,
  id: &str,
  more: &MoreActions,
  (open, anchor): (&Signal<bool>, &ElementRef),
) -> Element {
  let button_id = format!("{id}-more");
  let shown = open.get();
  let button = Button::empty()
    .id(button_id.clone())
    .width(Dimension::Px(ControlHeight::Helper.value()))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .ref_element(anchor.clone())
    .describe("a11y_name", more.label.clone())
    .describe("open", if shown { "true" } else { "false" })
    .child(icon(Icon::Ellipsis, IconSize::Md, PaletteColor::TextPrimary));
  let button = if shown {
    button.background(ExtraColor::Pressed.role())
  } else {
    hovered(button)
  };
  let toggle = open.clone();
  let button = Press::run(move || toggle.update(|open| *open = !*open))
    .on(button)
    .describe("state", "enabled");
  let rows: Vec<Element> = more
    .rows
    .iter()
    .map(|row| {
      let row_id = format!("{id}-{}", row.key);
      let item = match &row.choose {
        Ok(choose) => {
          let choose = choose.clone();
          MenuItem::new(row_id, row.label.clone(), move || choose())
        }
        Err(why) => MenuItem::unavailable(row_id, row.label.clone(), why.clone()),
      };
      item.icon(row.icon).render(ctx, open)
    })
    .collect();
  let panel = menu_panel(&format!("{id}-menu"))
    .width(Dimension::Px(MENU_WIDTH))
    .with_children(rows);
  let tip = Tooltip::new(button_id, more.label.clone()).silent(shown);
  Row::new()
    .height(Dimension::Pct(100.0))
    .child(tip.wrap(ctx, button))
    .child(menu_at_end(anchor.clone(), open.clone(), panel))
    .into()
}

/// `button` under the pointer and pressed.
fn hovered(button: Button) -> Button {
  button
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .active_style(Style::new().background(ExtraColor::Pressed.role()))
}

/// `button`, which cannot be pressed now, at half, saying `why` in its tooltip.
fn held(ctx: &mut Ctx, id: &str, button: Button, why: &Arc<str>) -> Element {
  let faded = inert(button)
    .opacity(DISABLED_OPACITY)
    .class(HELD_CLASS)
    .describe("state", "unavailable")
    .describe("reason", why.clone());
  Tooltip::new(format!("{id}-held"), why.clone()).wrap(ctx, faded)
}
