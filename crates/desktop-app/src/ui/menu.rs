//! Menus: a list of commands that opens anchored to the control that opens it and closes on a choice,
//! a press outside or Esc. Its panel is `surface_raised` inside a `border_strong` stroke, radius 8, 4
//! inside and 2 between rows (`Pipeline menu` `ClKqz`, `Tag menu` `LLIjG`). A row (`Menu item` `mAHGP`)
//! is 28 high, radius 4, 8 at its sides: a 14 `text_secondary` icon, the label in 12 `text_primary` and
//! a shortcut in Geist Mono 11 `text_muted` at its end, 8 apart; a row with a detail (`Menu item/detail`
//! `j6ZUhL`) is 6 over and under its label and the detail in 11 `text_muted` 2 under it, and a chosen
//! row ends in a 14 check. Rows take the `hover` fill under the pointer. A row that cannot be chosen
//! is faded, ends in a lock and says why as its detail. Groups are split by a 1 `border` line with 4
//! over and under it (`Menu separator` `UoDdI`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, Press, inert},
  tooltip::Truncated,
};
use crate::{
  navigation::link::Open,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
};

/// A row's label line, 16 high (`Label` in `Menu item/detail` `j6ZUhL`).
const LABEL_HEIGHT: f32 = 16.0;

/// The panel of a menu with the element id `id`; add its rows as children.
pub fn menu_panel(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .padding(SpacingSize::Xs)
    .spacing(ExtraSpacing::Tight.role())
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Md)
}

/// `panel` opened 6 under `anchor` at its start while `open` holds (`FpAul`); a choice (its rows close
/// it, [`MenuItem::render`]), a press outside or Esc closes it.
pub fn menu(anchor: ElementRef, open: Signal<bool>, panel: impl Into<Element>) -> Popup {
  Popup::new(anchor, panel)
    .open(open)
    .placement(Placement::BottomStart)
    .offset(0.0, ExtraSpacing::Snug.value())
}

/// `panel` opened 6 under `anchor` at its end while `open` holds, for a control at the end of a row (`Version
/// menu` `DqtY6` under its picker in `kadDh`); it closes as [`menu`] does.
pub fn menu_at_end(anchor: ElementRef, open: Signal<bool>, panel: impl Into<Element>) -> Popup {
  Popup::new(anchor, panel)
    .open(open)
    .placement(Placement::BottomEnd)
    .offset(0.0, ExtraSpacing::Snug.value())
}

/// The line between groups of rows.
pub fn separator(ctx: &Ctx) -> Element {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xs))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(line))
        .background(PaletteColor::Border),
    )
    .into()
}

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
  chosen: bool,
  /// An unavailable row ends in a lock.
  lock: bool,
  focus: Focus,
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
      chosen: false,
      lock: true,
      focus: Focus::Pointer,
      choice,
    }
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

  /// A choice within a form's field menu: reachable with Tab and activated with Enter or Space.
  pub fn in_form(mut self) -> Self {
    self.focus = Focus::Form;
    self
  }

  /// The row is the current choice: a check at its end.
  pub fn chosen(mut self, chosen: bool) -> Self {
    self.chosen = chosen;
    self
  }

  /// The label, cut to the row's room with its whole words in a tooltip, and the detail under it.
  fn text(&self, ctx: &mut Ctx, detail: Option<&str>) -> Column {
    let label = Truncated::new(
      format!("{}-label", self.id),
      self.label.clone(),
      TypographyStyle::Description,
    )
    .color(PaletteColor::TextPrimary)
    .render(ctx);
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
      .with_children(detail.map(|detail| {
        let detail = Text::new(detail).variant(TypographyStyle::Caption);
        if self.warning {
          detail.color(PaletteColor::Warning)
        } else {
          detail
        }
      }))
  }

  fn trailing(&self) -> Option<Element> {
    match (&self.choice, self.chosen, &self.shortcut) {
      (Choice::Unavailable(_), ..) if !self.lock => None,
      (Choice::Unavailable(_), ..) => Some(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted).into()),
      (_, true, _) => Some(icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary).into()),
      (_, false, Some(keys)) => Some(
        Text::new(keys)
          .variant(ExtraTypography::MonoNote.role())
          .nowrap()
          .into(),
      ),
      (_, false, None) => None,
    }
  }

  /// The row of the menu that `open` shows: a choice closes it.
  pub fn render(self, ctx: &mut Ctx, open: &Signal<bool>) -> Element {
    let detail = match &self.choice {
      Choice::Unavailable(reason) => Some(reason.clone()),
      Choice::Action(_) => self.detail.clone(),
    };
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(ControlHeight::Md.value()))
      .padding(match detail {
        Some(_) => Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()),
        None => Padding::horizontal(SpacingSize::Sm),
      })
      .spacing(SpacingSize::Sm)
      .align_items(if detail.is_some() {
        Alignment::Start
      } else {
        Alignment::Center
      })
      .with_children(
        self
          .icon
          .map(|glyph| icon(glyph, IconSize::Md, PaletteColor::TextSecondary).id(format!("{}-icon", self.id))),
      )
      .child(self.text(ctx, detail.as_deref()))
      .with_children(self.trailing());
    let surface = |row: Row| {
      Button::empty()
        .id(self.id.clone())
        .describe("a11y_name", self.label.clone())
        .width(Dimension::Pct(100.0))
        .rounded(RadiusSize::Sm)
        .align_items(Alignment::Center)
        .child(row)
    };
    match self.choice {
      Choice::Action(choose) => {
        let open = open.clone();
        let choose = match choose {
          Press::Run(choose) => Press::run(move || {
            open.set(false);
            choose();
          }),
          Press::Open(to) => Press::Open(Arc::new(move |at| {
            open.set(false);
            to(at);
          })),
        };
        self
          .focus
          .apply(choose.button())
          .id(self.id.clone())
          .describe("a11y_name", self.label.clone())
          .width(Dimension::Pct(100.0))
          .rounded(RadiusSize::Sm)
          .describe("chosen", if self.chosen { "true" } else { "false" })
          .child(row)
          .describe("state", "enabled")
          .hovered_style(Style::new().background(ExtraColor::Hover.role()))
          .into()
      }
      Choice::Unavailable(_) => inert(surface(row))
        .describe("state", "unavailable")
        .opacity(DISABLED_OPACITY)
        .into(),
    }
  }
}
