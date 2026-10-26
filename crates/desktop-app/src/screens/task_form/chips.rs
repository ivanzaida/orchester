//! Property chips (`Property chip` `y0m1CM`, `Property chip/Set` `tJukS`, `Property chip/Open` `Irr86`): one setting
//! of the task in a 28 high chip, radius 6, 8 before and 10 after its 13 icon and words, 6 apart. Empty, it says the
//! setting's name in 12 `text_muted` and its value in 12/500 `text_secondary` inside a `border` stroke; set, only the
//! value in `text_primary` on `surface_raised`, its icon in `text_secondary`; while its popover is open, the stroke is
//! `border_focus` on `surface_raised`. A press opens the setting's popover under it with today's control; a chip whose
//! value a run needs says its name in `warning` once the person tried to run without it. The chips sit in two rows
//! (`Run row` `sgq9u`, `Organize row` `P3Ju6L`) under the criteria; *…* (`More` `LAiJr`) holds what is left.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    popover::{PopoverSide, popover},
    pressable::{inert, pressable},
  },
};

/// Whether a chip's popover is open, and where it opens.
#[derive(Clone)]
pub struct ChipState {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
}

impl ChipState {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  /// Whether the popover is open; reading it subscribes the render.
  pub fn is_open(&self) -> bool {
    self.open.get()
  }

  /// `panel` under the chip while it is open; declared while closed too, so that a press on the chip switches to it
  /// from another open popover.
  pub fn popover(&self, panel: Option<Element>) -> Popup {
    popover(
      self.anchor.clone(),
      self.open.clone(),
      PopoverSide::Below,
      panel.unwrap_or_default(),
    )
  }
}

/// The chips the composer shows, but Sources, whose chip reads the sources it counts (`super::sources`).
#[derive(Clone)]
pub struct Chips {
  pub pipeline: ChipState,
  pub delivers: ChipState,
  pub limits: ChipState,
  pub module: ChipState,
  pub priority: ChipState,
  pub points: ChipState,
  pub more: ChipState,
}

impl Chips {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      pipeline: ChipState::new(ctx),
      delivers: ChipState::new(ctx),
      limits: ChipState::new(ctx),
      module: ChipState::new(ctx),
      priority: ChipState::new(ctx),
      points: ChipState::new(ctx),
      more: ChipState::new(ctx),
    }
  }
}

/// How a chip looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
  /// The setting's name and its value.
  Empty,
  /// Only its value.
  Set,
}

/// One property chip.
pub struct PropertyChip {
  id: Arc<str>,
  icon: Icon,
  label: Arc<str>,
  value: Arc<str>,
  look: Look,
  /// A run needs the value, and the person tried to run without it.
  warning: bool,
  /// It cannot open now: its value is all there is to see while the form is locked.
  locked: bool,
  /// It cannot be changed here, for this reason: a lock at its end, its popover says why.
  unavailable: Option<Arc<str>>,
}

impl PropertyChip {
  pub fn new(id: &str, icon: Icon, (label, value): (Arc<str>, Arc<str>), look: Look) -> Self {
    Self {
      id: id.into(),
      icon,
      label,
      value,
      look,
      warning: false,
      locked: false,
      unavailable: None,
    }
  }

  #[must_use]
  pub const fn warning(mut self, warning: bool) -> Self {
    self.warning = warning;
    self
  }

  #[must_use]
  pub const fn locked(mut self, locked: bool) -> Self {
    self.locked = locked;
    self
  }

  /// The setting cannot be changed here, for `reason`.
  #[must_use]
  pub fn unavailable(mut self, reason: Arc<str>) -> Self {
    self.unavailable = Some(reason);
    self
  }

  /// What a press does: open or close the popover, or nothing while the form is locked.
  fn surface(&self, state: &ChipState) -> Button {
    if self.locked {
      return inert(Button::empty().align_items(Alignment::Center));
    }
    let toggle = state.open.clone();
    pressable(move || toggle.update(|open| *open = !*open))
      .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)))
  }

  /// The chip's words: its name unless it is set, in `warning` while a run needs it, and its value.
  fn words(&self, chip: Button, set: bool) -> Button {
    let label = (!set).then(|| {
      Text::new(&self.label)
        .id(format!("{}-label", self.id))
        .variant(TypographyStyle::Description)
        .color(if self.warning {
          PaletteColor::Warning
        } else {
          PaletteColor::TextMuted
        })
        .nowrap()
    });
    let value = Text::new(&self.value)
      .id(format!("{}-value", self.id))
      .variant(TypographyStyle::Label)
      .color(if set {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      })
      .nowrap();
    chip.with_children(label).child(value)
  }

  /// The chip of `state`, with its popover.
  pub fn render(self, state: &ChipState, panel: Option<Element>) -> Element {
    let open = state.open.get();
    let set = self.look == Look::Set;
    let stroke = if open {
      PaletteColor::BorderFocus
    } else {
      PaletteColor::Border
    };
    let glyph = if set {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextMuted
    };
    let mut chip = self
      .surface(state)
      .id(self.id.clone())
      .ref_element(state.anchor.clone())
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::new().left(SpacingSize::Sm).right(ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Snug.role())
      .rounded(RadiusSize::Md)
      .border(Border::inside(BorderSize::Sm, stroke))
      .describe("a11y_name", self.label.clone())
      .describe("open", if open { "true" } else { "false" })
      .describe("look", if set { "set" } else { "empty" })
      .child(icon(self.icon, IconSize::Base, glyph));
    if set || open {
      chip = chip.background(PaletteColor::SurfaceRaised);
    }
    let mut chip = self.words(chip, set);
    if let Some(reason) = &self.unavailable {
      chip = chip
        .describe("state", "unavailable")
        .describe("reason", reason.clone())
        .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted).id(format!("{}-lock", self.id)));
    }
    Row::new().child(chip).child(state.popover(panel)).into()
  }
}

/// *…* (`More` `LAiJr`): a 28 square with a 16 `ellipsis`, the `hover` fill under the pointer; `on` while what it holds
/// is set, as a set chip is.
pub fn more_chip(ctx: &Ctx, state: &ChipState, on: bool, panel: Option<Element>) -> Element {
  let open = state.open.get();
  let toggle = state.open.clone();
  let mut button = pressable(move || toggle.update(|open| *open = !*open))
    .id("task-form-more")
    .ref_element(state.anchor.clone())
    .size(
      Dimension::Px(ControlHeight::Md.value()),
      Dimension::Px(ControlHeight::Md.value()),
    )
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("a11y_name", ctx.t("task_form.fields.more.label"))
    .describe("open", if open { "true" } else { "false" })
    .describe("on", if on { "true" } else { "false" })
    .child(icon(Icon::Ellipsis, IconSize::Lg, PaletteColor::TextSecondary));
  if on || open {
    button = button.background(PaletteColor::SurfaceRaised).border(Border::inside(
      BorderSize::Sm,
      if open {
        PaletteColor::BorderFocus
      } else {
        PaletteColor::Border
      },
    ));
  }
  Row::new().child(button).child(state.popover(panel)).into()
}

/// A row of chips, 8 apart.
pub fn chip_row(id: &str, chips: Vec<Element>) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(chips)
    .into()
}

/// One choice of a chip's menu that says what it chooses (`Feature delivery v3` `I2BrOT`, `Outside the repository`
/// `yufU5`): a 14 icon, its name in 12.5/500 and its help in 11.5 `text_muted` under it, a check at its end while it is
/// the choice; 8 over and under, 10 at the sides, radius 6, the `hover` fill under the pointer. A press chooses it and
/// closes the menu `open`.
pub struct MenuOption {
  pub id: String,
  pub icon: Icon,
  pub title: Arc<str>,
  pub help: Option<Arc<str>>,
  pub chosen: bool,
}

impl MenuOption {
  pub fn render(self, open: &Signal<bool>, choose: impl Fn() + Send + Sync + 'static) -> Element {
    let open = open.clone();
    let words = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(
        Text::new(&self.title)
          .id(format!("{}-title", self.id))
          .variant(ExtraTypography::OptionTitle.role()),
      )
      .with_children(self.help.map(|help| {
        Text::new(&help)
          .id(format!("{}-help", self.id))
          .width(Dimension::Pct(100.0))
          .variant(ExtraTypography::Note.role())
      }));
    let glyph = if self.chosen {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextSecondary
    };
    pressable(move || {
      open.set(false);
      choose();
    })
    .id(self.id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("a11y_name", self.title.clone())
    .describe("chosen", if self.chosen { "true" } else { "false" })
    .child(icon(self.icon, IconSize::Md, glyph))
    .child(words)
    .with_children(
      self
        .chosen
        .then(|| icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)),
    )
    .into()
  }
}

/// A menu's heading (`Head` `NkT7L`): 11/500 `text_muted`, 6 over, 4 under and 10 at its sides.
pub fn menu_head(text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Snug.role())
        .bottom(SpacingSize::Xs)
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .child(
      Text::new(text)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted),
    )
    .into()
}
