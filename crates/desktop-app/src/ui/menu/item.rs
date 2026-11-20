//! A menu's row (`Menu item` `mAHGP`): 28 high, radius 4, 8 at its sides: a 14 `text_secondary` icon, the label in 12
//! `text_primary` and a shortcut in Geist Mono 11 `text_muted` at its end, 8 apart; a row with a detail (`Menu
//! item/detail` `j6ZUhL`) is 6 over and under its label and the detail in 11 `text_muted` 2 under it, or the detail in
//! 12 `text_muted` after the label on one line (`Item` in 01d `jGqpS`), and a chosen row ends in a 14 check. A row that
//! destroys has its icon and label in `danger` (`Cancel run` `Z1Uvbo` in `JapSp`). Rows take the `hover` fill under
//! the pointer. A row that cannot be chosen is faded, ends in a lock and says why as its detail.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
  ui::{self, Styled},
};

use super::standalone;
use crate::{
  navigation::{
    link::{Open, follow},
    press::OpenIn,
  },
  theme::{ControlHeight, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
    tooltip::Truncated,
  },
};

/// A row's label line, 16 high (`Label` in `Menu item/detail` `j6ZUhL`).
const LABEL_HEIGHT: f32 = 16.0;

enum Choice {
  Action(Press),
  /// It cannot be chosen now, for this reason.
  Unavailable(Arc<str>),
}

pub struct MenuItem {
  id: Arc<str>,
  label: Arc<str>,
  icon: Option<Icon>,
  shortcut: Option<Arc<str>>,
  detail: Option<Arc<str>>,
  /// The detail says what is wrong with the choice.
  warning: bool,
  /// Whether the row is the current choice, in a menu of choices.
  chosen: Option<bool>,
  /// An unavailable row ends in a lock.
  lock: bool,
  /// The label is an identifier, in Geist Mono.
  mono: bool,
  /// The row destroys: its icon and label in `danger`.
  danger: bool,
  /// The detail follows the label on its line.
  inline: bool,
  /// The row is 40 high whatever its detail, its content centered.
  tall: bool,
  /// The row opens a list beside the menu: a 12 `text_muted` chevron at its end.
  submenu: bool,
  /// What the row leads to, in 11 `text_muted` at its end, where a shortcut would be.
  hint: Option<Arc<str>>,
  /// An unavailable row's reason is said elsewhere, in the menu's foot: the row is only faded.
  reason_below: bool,
  /// What the row is, for what opens beside it.
  at: Option<ElementRef>,
  choice: Choice,
}

impl MenuItem {
  /// A row saying `label` that runs `choose` when chosen.
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, choose: impl Fn() + Send + Sync + 'static) -> Self {
    Self::with_choice(id.into(), label.into(), Choice::Action(Press::run(choose)))
  }

  /// A row saying `label` that opens a place with `open` by a browser's rules when chosen (`navigation::link`).
  pub fn opens(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, open: Open) -> Self {
    Self::with_choice(id.into(), label.into(), Choice::Action(Press::Open(open)))
  }

  /// A row that cannot be chosen now, for `reason`: faded, ending in a lock, the reason as its detail.
  pub fn unavailable(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, reason: impl Into<Arc<str>>) -> Self {
    Self::with_choice(id.into(), label.into(), Choice::Unavailable(reason.into()))
  }

  const fn with_choice(id: Arc<str>, label: Arc<str>, choice: Choice) -> Self {
    Self {
      id,
      label,
      icon: None,
      shortcut: None,
      detail: None,
      warning: false,
      chosen: None,
      lock: true,
      mono: false,
      danger: false,
      inline: false,
      tall: false,
      submenu: false,
      hint: None,
      reason_below: false,
      at: None,
      choice,
    }
  }

  /// The row destroys: its icon and label in `danger` (`Cancel run` `Z1Uvbo` in `JapSp`).
  #[must_use]
  pub const fn danger(mut self) -> Self {
    self.danger = true;
    self
  }

  /// The row opens a list beside the menu: a chevron at its end, after its keys (`Item · Move to` in `Card menu`
  /// `vV3BX`).
  #[must_use]
  pub const fn submenu(mut self) -> Self {
    self.submenu = true;
    self
  }

  /// What the row leads to, at its end in 11 `text_muted`, right-aligned (`ready to claim` in `Reopen menu` of T4
  /// `V26A3`).
  #[must_use]
  pub fn hint(mut self, hint: impl Into<Arc<str>>) -> Self {
    self.hint = Some(hint.into());
    self
  }

  /// The row is `at`, for what opens beside it (a card menu's list beside its *Move to*).
  #[must_use]
  pub fn element(mut self, at: ElementRef) -> Self {
    self.at = Some(at);
    self
  }

  /// An unavailable row that is only faded, 28 high, its reason said once in the menu's foot (`Reason` in `Move to
  /// menu` `c88il`); tools still read the reason on the row.
  #[must_use]
  pub const fn reason_below(mut self) -> Self {
    self.reason_below = true;
    self.lock = false;
    self
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// The keys that do the same, at the row's end.
  pub fn shortcut(mut self, keys: impl Into<Arc<str>>) -> Self {
    self.shortcut = Some(keys.into());
    self
  }

  /// What the row is, under its label.
  pub fn detail(mut self, detail: impl Into<Arc<str>>) -> Self {
    self.detail = Some(detail.into());
    self
  }

  /// The detail says what is wrong with the choice: in `warning`.
  pub fn warning_detail(mut self) -> Self {
    self.warning = true;
    self
  }

  /// An unavailable row says why without the lock, where the choice comes back by itself (*By hour* on a
  /// range longer than 7 days, `Group by menu` in `p732q`).
  pub fn without_lock(mut self) -> Self {
    self.lock = false;
    self
  }

  /// The label is an identifier, such as a model id, in Geist Mono.
  #[must_use]
  pub const fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  /// The row is the current choice: a check at its end.
  pub fn chosen(mut self, chosen: bool) -> Self {
    self.chosen = Some(chosen);
    self
  }

  /// The row is 40 high with its label and detail centered, an agent with its model (`Agent · Builder` `lsXUc` in
  /// `Agent menu` `BpZlb`).
  pub fn tall(mut self) -> Self {
    self.tall = true;
    self
  }

  /// The detail follows the label on its line instead of under it: the row stays one line high (`Item` `RMq18` in
  /// 01d).
  pub fn inline_detail(mut self) -> Self {
    self.inline = true;
    self
  }

  /// The label, cut to the row's room with its whole words in a tooltip, and the detail under it or after it.
  fn text(&self, ctx: &mut Ctx, detail: Option<&str>) -> Element {
    let style = if self.mono {
      ExtraTypography::PropertyMono.role()
    } else {
      TypographyStyle::Description
    };
    let label = Truncated::new(format!("{}-label", self.id), self.label.clone(), style)
      .color(self.content_color(PaletteColor::Foreground))
      .render(ctx);
    let detail = detail.map(|detail| {
      let detail = Text::new(detail).id(format!("{}-detail", self.id));
      let detail = if self.inline {
        // On the label's line the detail gives way first: it is cut, the label stays whole.
        detail
          .variant(TypographyStyle::Description)
          .color(PaletteColor::MutedForeground)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
      } else {
        detail.variant(TypographyStyle::Caption)
      };
      if self.warning {
        detail.color(PaletteColor::Warning)
      } else {
        detail
      }
    });
    if self.inline {
      return Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .height(Dimension::Px(LABEL_HEIGHT))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Row::new().flex_shrink(0.0).child(label))
        .with_children(detail)
        .into();
    }
    Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Row::new()
          .height(Dimension::Px(LABEL_HEIGHT))
          .align_items(Alignment::Center)
          .child(label),
      )
      .with_children(detail)
      .into()
  }

  /// The color of the row's icon or label, `color` unless the row destroys.
  fn content_color(&self, color: PaletteColor) -> PaletteColor {
    if self.danger { PaletteColor::Destructive } else { color }
  }

  fn trailing(&self) -> Option<Element> {
    match (&self.choice, self.chosen == Some(true), &self.shortcut) {
      (Choice::Unavailable(_), ..) if !self.lock => None,
      (Choice::Action(_), false, _) if self.hint.is_some() => self.hint.as_deref().map(|hint| {
        Text::new(hint)
          .id(format!("{}-hint", self.id))
          .variant(TypographyStyle::Caption)
          .nowrap()
          .into()
      }),
      (Choice::Unavailable(_), ..) => Some(icon(Icon::Lock, IconSize::Sm, PaletteColor::MutedForeground).into()),
      (_, true, _) => Some(icon(Icon::Check, IconSize::Md, PaletteColor::Foreground).into()),
      (_, false, Some(keys)) => Some(
        Text::new(keys)
          .variant(ExtraTypography::MonoNote.role())
          .nowrap()
          .into(),
      ),
      (_, false, None) => None,
    }
  }

  /// The kit's row as lurq's menu item: what it shows is the row ([`ui::MenuItem::content`]), and the item is the
  /// menu's, so a choice closes the menu before it runs. A row that opens a place opens it in the active tab, or where
  /// a middle press or one with Ctrl (Cmd on macOS) asks (`navigation::link`).
  pub fn render(self, ctx: &mut Ctx) -> ui::MenuItem {
    let detail = match &self.choice {
      Choice::Unavailable(_) if self.reason_below => None,
      Choice::Unavailable(reason) => Some(reason.clone()),
      Choice::Action(_) => self.detail.clone(),
    };
    let row = self.row(ctx, detail.as_deref());
    let item = ui::MenuItem::new(&self.label)
      .id(self.id.clone())
      .content(row)
      .destructive(self.danger)
      .describe("chosen", if self.chosen == Some(true) { "true" } else { "false" });
    let item = match self.chosen {
      Some(chosen) => item.checked(chosen),
      None => item,
    };
    match self.choice {
      Choice::Action(Press::Run(choose)) => item.on_select(move || choose()).describe("state", "enabled"),
      Choice::Action(Press::Open(open)) => {
        let aside = open.clone();
        item
          .on_select(move || open(OpenIn::ActiveTab))
          .on_open_aside(move |event: MouseEvent| follow(&aside, &event))
          .describe("state", "enabled")
      }
      Choice::Unavailable(reason) => item
        .unavailable(&reason)
        .describe("state", "unavailable")
        .describe("reason", reason),
    }
  }

  /// The row in a section of a menu that scrolls ([`standalone`]).
  pub fn render_scrolled(self, ctx: &mut Ctx, open: &Signal<bool>) -> Element {
    standalone(self.render(ctx), open)
  }

  /// What the row shows: its icon, label and detail, and at its end its hint, keys, check, lock or chevron.
  fn row(&self, ctx: &mut Ctx, detail: Option<&str>) -> Row {
    let stacked = detail.is_some() && !self.inline && !self.tall;
    let row = match &self.at {
      Some(at) => Row::new().ref_element(at.clone()),
      None => Row::new(),
    };
    row
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(if self.tall {
        ControlHeight::Agent.value()
      } else {
        ControlHeight::Md.value()
      }))
      .padding(if stacked {
        Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role())
      } else {
        Padding::horizontal(SpacingSize::Sm)
      })
      .spacing(SpacingSize::Sm)
      .align_items(if stacked { Alignment::Start } else { Alignment::Center })
      .with_children(self.icon.map(|glyph| {
        icon(
          glyph,
          IconSize::Md,
          self.content_color(ExtraColor::TextSecondary.role()),
        )
        .id(format!("{}-icon", self.id))
      }))
      .child(self.text(ctx, detail))
      .with_children(self.trailing())
      .with_children(
        self
          .submenu
          .then(|| icon(Icon::ChevronRight, IconSize::Sm, PaletteColor::MutedForeground)),
      )
  }
}
