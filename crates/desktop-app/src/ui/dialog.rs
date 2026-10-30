//! A confirmation in a modal (confirmations are modals, decided 2026-10-28; `Modal layer` `ZsL7y` in `04d · Bind an
//! MCP server` `z0iFy`, after `05d · Delete connection` `D3c8ht`): the `scrim` over the window under its title bar,
//! and the dialog centred in it, 500 wide on `surface_raised` in a `border_strong` stroke, radius 12, with the
//! dialog's shadow. Its body is 20 at its sides and top, its parts 14 apart: a 32 tile in `warning_muted` with the
//! glyph in `warning` beside the question in 15/600, the parts the caller gives (a choice under its label, lines of
//! what it does, [`line`]), and for a person-only action the line on `hover` that only the person confirms it. Its
//! footer, under a `border` line, says *Esc cancels* at its start and ends in *Cancel* and the action. Esc and a press
//! on the scrim cancel.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Modal, ModalTarget, Row, Stack, Text},
  core::Signal,
  layout::{Alignment, StackAlignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  button::{ActionButton, ButtonKind},
  icon::{Icon, icon},
  kbd::key_cap,
  pressable::pressable,
};
use crate::{
  chrome::TITLE_BAR_HEIGHT,
  theme::{DIALOG_TILE, DIALOG_WIDTH, DialogSpacing, DialogText, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
};

/// A line of what it does: its glyph is 18 high, as the line's first line is (`Icon box` `EyPBp`).
const LINE_GLYPH_BOX: f32 = 18.0;

pub struct Dialog {
  id: Arc<str>,
  glyph: Icon,
  title: Arc<str>,
  parts: Vec<Element>,
  person_only: Option<Arc<str>>,
  cancel: Arc<str>,
  action: Option<ActionButton>,
  under: Vec<Element>,
}

impl Dialog {
  /// The dialog `id` (its title `<id>-title`, its *Cancel* `<id>-cancel`) asking `title` beside `glyph`.
  pub fn new(id: &str, glyph: Icon, title: impl Into<Arc<str>>, cancel: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      glyph,
      title: title.into(),
      parts: Vec::new(),
      person_only: None,
      cancel: cancel.into(),
      action: None,
      under: Vec::new(),
    }
  }

  /// A part of its body, in order.
  pub fn part(mut self, part: impl Into<Element>) -> Self {
    self.parts.push(part.into());
    self
  }

  /// Only the person confirms it: the body ends in `words` on `hover`, after the hand.
  pub fn person_only(mut self, words: impl Into<Arc<str>>) -> Self {
    self.person_only = Some(words.into());
    self
  }

  /// What the footer ends in, after *Cancel*.
  pub fn action(mut self, action: ActionButton) -> Self {
    self.action = Some(action);
    self
  }

  /// What the body says after its parts, e.g. why the action did nothing.
  pub fn under(mut self, under: impl IntoIterator<Item = Element>) -> Self {
    self.under.extend(under);
    self
  }

  /// The dialog over the window while `open` holds; *Cancel*, Esc and the scrim set it to `false`.
  pub fn render(mut self, ctx: &Ctx, open: &Signal<bool>) -> Element {
    let id = self.id.clone();
    let footer = footer(ctx, &id, &self.cancel, self.action.take(), open);
    let body = self.body();
    let dialog = Column::new()
      .id(id.to_string())
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Lg)
      .box_shadow(ShadowStyle::Lg)
      .clip()
      .child(body)
      .child(footer);
    let shut = open.clone();
    let scrim = pressable(move || shut.set(false))
      .id(format!("{id}-scrim"))
      .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
      .background(ExtraColor::Scrim.role());
    let layer = Stack::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .stack_align(StackAlignment::Center)
      .child(scrim)
      .child(dialog);
    // The band above the scrim is the title bar's: the pointer passes through to its drag and window controls.
    let window = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::new().top(Dimension::Px(TITLE_BAR_HEIGHT)))
      .hit_test(HitTestBehavior::ContentOnly)
      .child(layer);
    Modal::new(window).target(ModalTarget::Root).open(open.clone()).into()
  }

  fn body(self) -> Column {
    let id = &self.id;
    let tile = Row::new()
      .width(Dimension::Px(DIALOG_TILE))
      .height(Dimension::Px(DIALOG_TILE))
      .flex_shrink(0.0)
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::Tile.role())
      .background(PaletteColor::WarningMuted)
      .child(icon(self.glyph, IconSize::Lg, PaletteColor::Warning));
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(tile)
      .child(
        Text::new(&self.title)
          .id(format!("{id}-title"))
          .variant(DialogText::Title.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      );
    let person = self.person_only.as_ref().map(|words| {
      Row::new()
        .id(format!("{id}-person-only"))
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .rounded(RadiusSize::Md)
        .background(ExtraColor::Hover.role())
        .child(icon(Icon::Hand, IconSize::Base, PaletteColor::TextSecondary))
        .child(
          Text::new(words)
            .variant(lurq::app::theme::TypographyStyle::Description)
            .color(PaletteColor::TextSecondary)
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(DialogSpacing::Side.role())
          .left(DialogSpacing::Side.role())
          .right(DialogSpacing::Side.role())
          .bottom(SpacingSize::Lg),
      )
      .spacing(DialogSpacing::Parts.role())
      .child(head)
      .with_children(self.parts)
      .with_children(person)
      .with_children(self.under)
  }
}

/// *Esc cancels*, *Cancel* and the action.
fn footer(ctx: &Ctx, id: &str, cancel: &str, action: Option<ActionButton>, open: &Signal<bool>) -> Row {
  let shut = open.clone();
  let cancel = ActionButton::new(format!("{id}-cancel"), ButtonKind::Secondary, cancel)
    .in_form()
    .on_click(move || shut.set(false));
  Row::new()
    .id(format!("{id}-footer"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(DialogSpacing::Side.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(key_cap(&format!("{id}-esc"), &ctx.t("ui.dialog.escKey")))
        .child(
          Text::new(&ctx.t("ui.dialog.escCancels"))
            .variant(DialogText::Keys.role())
            .nowrap(),
        ),
    )
    .child(cancel)
    .with_children(action.map(|action| action.in_form()))
}

/// A line of what it does: a 13 `glyph` in `text_muted`, 10 from its words in 12.5 `text_secondary`.
pub fn line(id: impl Into<String>, glyph: Icon, words: &str) -> Row {
  Row::new()
    .id(id.into())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Row::new()
        .height(Dimension::Px(LINE_GLYPH_BOX))
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted)),
    )
    .child(
      Text::new(words)
        .variant(DialogText::Line.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// The lines of what it does, 8 apart (`What it does` `lEByc`).
pub fn lines(id: &str, lines: impl IntoIterator<Item = Row>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(lines)
}

/// The dialog's choice under its `label`, 6 apart (`Choice` `i8uRWi`).
pub fn choice(id: &str, label: &str, control: impl Into<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(label).variant(DialogText::ChoiceLabel.role()))
    .child(control)
}
