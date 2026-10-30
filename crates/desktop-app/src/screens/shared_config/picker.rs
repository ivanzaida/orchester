//! The picker of what a reference is bound to on this computer (`04c` `rpQ7s`, `07b` `zE2cB`, `06b` `p779C`): a
//! select (*Choose a connection*, 200 wide) or *Change* opens a menu, 300 wide at the control's end, of the person's
//! own connections (or MCP servers, secrets), the current one checked; after a line, the ones of another role say so
//! after their name and cannot be chosen. A menu may end, after a line, in *Add an account…*, which opens Settings ›
//! Accounts.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::{ControlHeight, ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    menu::{MenuItem, menu_at_end, menu_panel, separator},
    pressable::{Focus, pressable},
  },
};

/// The select's width (`Select` `n7AtXh` in `04b`).
pub(crate) const SELECT_WIDTH: f32 = 200.0;
/// The menu's width (`Menu` in `04c`, `07b`).
const MENU_WIDTH: f32 = 300.0;

/// What opens the picker.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Opener {
  /// A select saying what to choose, or the chosen one's name, `width` wide.
  Select { words: Arc<str>, width: f32 },
  /// *Change*, after the row's state.
  Change,
}

/// One thing the menu offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Item {
  pub label: Arc<str>,
  pub glyph: Icon,
  /// What follows the label on its line, e.g. an account's plan.
  pub detail: Option<Arc<str>>,
  pub chosen: bool,
  /// Why it cannot be chosen, after its name: *another role*.
  pub unavailable: Option<Arc<str>>,
}

#[derive(Clone)]
pub(crate) struct Props {
  pub id: Arc<str>,
  pub opener: Opener,
  pub items: Vec<Item>,
  /// The menu ends in *Add an account…*.
  pub add_account: bool,
  /// A binding is being made: the picker neither opens nor chooses until it is done.
  pub busy: bool,
  /// Runs with the index of the chosen item, in the handler of the input that chose it.
  pub choose: Arc<dyn Fn(usize) + Send + Sync>,
}

impl PartialEq for Props {
  /// The same `choose` too: a parent that renders again makes a new one, which may hold other ids for the same items.
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.opener == other.opener
      && self.items == other.items
      && self.add_account == other.add_account
      && self.busy == other.busy
      && Arc::ptr_eq(&self.choose, &other.choose)
  }
}

impl DevtoolsInspectable for Props {}

pub(crate) struct Picker {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for Picker {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let trigger = self.trigger(ctx, &props);
    let panel = menu_panel(&format!("{}-menu", props.id))
      .width(Dimension::Px(MENU_WIDTH))
      .with_children(self.rows(ctx, &props));
    Row::new()
      .flex_shrink(0.0)
      .child(trigger)
      .child(menu_at_end(self.anchor.clone(), self.open.clone(), panel))
  }
}

impl Picker {
  fn trigger(&self, ctx: &Ctx, props: &Props) -> Element {
    let (open, busy) = (self.open.clone(), props.busy);
    let toggle = move || {
      if !busy {
        open.update(|open| *open = !*open);
      }
    };
    match &props.opener {
      Opener::Change => {
        let change = ActionButton::new(
          format!("{}-change", props.id),
          ButtonKind::Secondary,
          ctx.t("sharing.bindings.change"),
        )
        .in_form()
        .element_ref(self.anchor.clone())
        .on_click(toggle);
        if busy { change.disabled() } else { change }.into()
      }
      Opener::Select { words, width } => {
        let open = self.open.get();
        Focus::Keyboard
          .apply(pressable(toggle))
          .id(props.id.to_string())
          .ref_element(self.anchor.clone())
          .describe("a11y_name", words.clone())
          .describe(
            "state",
            match (busy, open) {
              (true, _) => "disabled",
              (false, true) => "open",
              (false, false) => "closed",
            },
          )
          .width(Dimension::Px(*width))
          .height(Dimension::Px(ControlHeight::Md.value()))
          .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
          .spacing(SpacingSize::Sm)
          .rounded(RadiusSize::Md)
          .background(PaletteColor::SurfaceInput)
          .border_inside(
            BorderSize::Sm,
            if open {
              PaletteColor::BorderFocus
            } else {
              PaletteColor::Border
            },
          )
          .hovered_style(Style::new().border_inside(BorderSize::Sm, PaletteColor::BorderStrong))
          .child(
            Text::new(words)
              .variant(TypographyStyle::Description)
              .color(if busy {
                PaletteColor::TextMuted
              } else {
                PaletteColor::TextPrimary
              })
              .flex(1.0)
              .min_width(Dimension::Px(0.0))
              .nowrap()
              .text_overflow(TextOverflow::Elipsis),
          )
          .child(icon(
            if open { Icon::ChevronUp } else { Icon::ChevronDown },
            IconSize::Md,
            PaletteColor::TextMuted,
          ))
          .into()
      }
    }
  }

  fn rows(&self, ctx: &mut Ctx, props: &Props) -> Vec<Element> {
    let mut rows: Vec<Element> = Vec::new();
    for (index, item) in props.items.iter().enumerate() {
      // A line between the choosable ones and the first that can't be chosen (`04c`).
      let after_choosable = index > 0 && props.items[index - 1].unavailable.is_none();
      if item.unavailable.is_some() && after_choosable {
        rows.push(separator(ctx));
      }
      let id = format!("{}-option-{index}", props.id);
      let row = match &item.unavailable {
        Some(reason) => MenuItem::unavailable(id, item.label.clone(), reason.clone()).without_lock(),
        None => {
          let (choose, busy) = (props.choose.clone(), props.busy);
          let row = MenuItem::new(id, item.label.clone(), move || {
            if !busy {
              choose(index);
            }
          });
          match &item.detail {
            Some(detail) => row.detail(detail.clone()),
            None => row,
          }
        }
      };
      rows.push(
        row
          .icon(item.glyph)
          .inline_detail()
          .chosen(item.chosen)
          .in_form()
          .render(ctx, &self.open),
      );
    }
    if props.add_account {
      rows.push(separator(ctx));
      let open = Navigation::of(ctx).to(Route::Accounts(AccountsView {
        account: None,
        panel: AccountsPanel::Add,
      }));
      rows.push(
        MenuItem::opens(
          format!("{}-add-account", props.id),
          ctx.t("sharing.bindings.addAccount"),
          open,
        )
        .icon(Icon::Plus)
        .render(ctx, &self.open),
      );
    }
    rows
  }
}

/// The picker `props` as an element.
pub(crate) fn picker(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("{}-picker", props.id);
  ctx.mount_keyed::<Picker>(&key, props)
}

/// *Add an account…* as a button, where a reference has no connection of its role to choose (`06c`, `07c`).
pub(crate) fn add_account(ctx: &mut Ctx, id: &str) -> Element {
  let open = Navigation::of(ctx).to(Route::Accounts(AccountsView {
    account: None,
    panel: AccountsPanel::Add,
  }));
  ActionButton::new(
    id.to_owned(),
    ButtonKind::Secondary,
    ctx.t("sharing.bindings.addAccount"),
  )
  .icon(Icon::Plus)
  .in_form()
  .opens(open)
  .into()
}
