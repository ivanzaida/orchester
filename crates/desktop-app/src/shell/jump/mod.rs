//! *Jump to* (`Jump to · open` `aDYAX`): a palette over the window, 560 wide, that finds the tab's project's tasks
//! and modules, every page and hub tab, and actions (`found`). The row under the project switcher (`Jump to`
//! `Z3lvL` in `Zn8dR`) and Ctrl+K anywhere (Cmd+K on macOS, `navigation::shortcuts`) open it with the focus in its
//! field. ↑ and ↓ move among the results, Enter opens the current one, Ctrl+Enter (Cmd+Enter) opens it in a new
//! tab, and Esc, a press outside or a choice close it. Files' *Go to file* keeps its own Ctrl+P.

mod found;
mod panel;

use lurq::{
  app::{
    component::Component,
    ctx::{CollisionStrategy, Ctx},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use parking_lot::Mutex;

pub use self::found::{Found, Group, Target, find};
use crate::{
  navigation::Navigation,
  theme::{ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    kbd::key_cap,
    nav_item::NAV_ITEM_HEIGHT,
    pressable::pressable,
  },
};

/// How far under the content panel's top the palette opens.
const PALETTE_TOP: f32 = 64.0;

/// The keys that open *Jump to*, as the platform writes them.
fn keys(ctx: &Ctx) -> std::sync::Arc<str> {
  ctx.t(if cfg!(target_os = "macos") {
    "shell.jump.keysMac"
  } else {
    "shell.jump.keys"
  })
}

/// *Jump to…* with its keys under the project switcher (`Jump` `qe5ny`): it opens the palette.
pub fn row(ctx: &mut Ctx, navigation: &Navigation) -> Element {
  let jump = navigation.jump();
  let label = ctx.t("shell.jump.row");
  let button = pressable(move || jump.set(true))
    .id("sidebar-jump")
    .describe("a11y_name", label.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(NAV_ITEM_HEIGHT))
    .padding(Padding::new().left(SpacingSize::Sm).right(SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .align_items(Alignment::Center)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::Search, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex(1.0),
    )
    .child(key_cap("sidebar-jump-keys", &keys(ctx)));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::horizontal(SpacingSize::Md)
        .top(ExtraSpacing::Snug.role())
        .bottom(SpacingSize::Xs),
    )
    .child(button)
    .into()
}

/// The palette's state while the window lives: what is typed, the current result and the field.
pub struct JumpTo {
  query: Signal<String>,
  at: Signal<usize>,
  input: ElementRef,
  /// Whether it was open at the last render, so that each opening starts afresh with the focus in the field.
  was_open: Mutex<bool>,
}

impl Component for JumpTo {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      query: ctx.signal(String::new()),
      at: ctx.signal(0),
      input: ctx.element_ref(),
      was_open: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let open = navigation.jump();
    let shown = open.get();
    let opened = {
      let mut was_open = self.was_open.lock();
      let opened = shown && !*was_open;
      *was_open = shown;
      opened
    };
    if opened {
      self.query.set(String::new());
      self.at.set(0);
    }
    let palette = if shown {
      let state = panel::State {
        query: self.query.clone(),
        at: self.at.clone(),
        input: self.input.clone(),
        open: open.clone(),
      };
      let palette = panel::palette(ctx, &navigation, &state);
      if opened {
        ctx.focus(&self.input);
      }
      palette
    } else {
      Element::new()
    };
    // Centred over the content panel, `PALETTE_TOP` under its top: below its bottom edge, moved up by its height. A
    // press on the panel is not outside the popup, so the panel closes it itself (`shell::layout`).
    let anchor = navigation.jump_anchor();
    let up = anchor.height();
    Popup::new(anchor, palette)
      .open(open)
      .placement(Placement::Bottom)
      .offset(0.0, PALETTE_TOP - up)
      .collision(CollisionStrategy::Clamp)
      .switch_on_anchor_press(false)
  }
}
