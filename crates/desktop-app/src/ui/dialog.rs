//! A confirmation in a modal (confirmations are modals, decided 2026-10-28; `Modal layer` `ZsL7y` in `04d · Bind an
//! MCP server` `z0iFy`, after `05d · Delete connection` `D3c8ht`): the `scrim` over the window under its title bar,
//! and the dialog centred in it, 500 wide (or as wide as its caller says) on `surface_raised` in a `border_strong`
//! stroke, radius 12, with the dialog's shadow. Its body is 20 at its sides and top, its parts 14 apart: a 32 tile in
//! `warning_muted` with the glyph in `warning` (or another [`Tone`]) beside the question in 15/600, the parts the
//! caller gives (a choice under its label, lines of what it does, [`line`]), and for a person-only action the line on
//! `hover` that only the person confirms it. Its footer, under a `border` line, says *Esc cancels* at its start, after
//! what Enter does where Enter confirms it (`Keys` `D0v5t` in `08e · History · Delete confirmation` `I4xzOf`), and
//! ends in *Cancel* (which its caller may give the focus) and the actions; a plain footer (`Footer` `a2EDb` of `03 ·
//! Install now · runs at work` `MGILm`) ends in a ghost *Cancel* with its ×, then the actions. Esc and a press on the
//! scrim cancel. It is lurq's `ui::Dialog` in the theme's recipe (`theme::kit::dialog`, which says how it looks); where
//! Enter confirms, the panel takes the focus as it opens, so Enter confirms and presses no button.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
  ui::{self, DialogFocus},
};

use super::{
  button::{ActionButton, ButtonKind},
  icon::{Icon, icon},
};
use crate::theme::{
  DIALOG_WIDTH, DialogText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, kit::dialog as kit,
};

/// A line of what it does: its glyph is 18 high, as the line's first line is (`Icon box` `EyPBp`).
const LINE_GLYPH_BOX: f32 = 18.0;

/// The colors of the dialog's tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
  /// `warning_muted` with the glyph in `warning`: what the action changes needs a second look.
  #[default]
  Warning,
  /// `info_muted` with the glyph in `info` (`Tile` `gGQD5` of `MGILm`).
  Info,
  /// `danger_muted` with the glyph in `danger`: the action destroys (`Tile` `ZVuxg` of `I4xzOf`).
  Danger,
  /// `hover` with the glyph in `text_secondary`: the action changes nothing, e.g. an export (`Tile` `C907Pu` of
  /// `hysCz`), or a plain change, e.g. a team module's branch (`yhAB1` in `04b` `InYp8`).
  Neutral,
}

impl Tone {
  /// The state the dialog's recipe draws the tile in (`theme::kit::dialog`); the warning's is its own look.
  const fn state(self) -> &'static str {
    match self {
      Self::Warning => "warning",
      Self::Info => kit::INFO,
      Self::Danger => kit::DANGER,
      Self::Neutral => kit::NEUTRAL,
    }
  }
}

/// What a footer says at its start when it is a note's (`Footer` `qERFQ` in T13 `DWfta`, `Key` `e76anZ` in T14
/// `uKVK2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FooterNote {
  /// Why the action cannot be taken yet: a 12 `info` glyph and the words in 12 `text_muted`.
  Unavailable(Arc<str>),
  /// What Esc does, in Geist Mono 11 `text_muted`.
  Keys(Arc<str>),
}

/// What Enter does in the dialog: its words beside the key, and the action.
type Enter = (Arc<str>, Arc<dyn Fn() + Send + Sync>);

pub struct Dialog {
  id: Arc<str>,
  glyph: Icon,
  tone: Tone,
  width: f32,
  title: Arc<str>,
  /// What the dialog is about, under its title.
  what: Option<Arc<str>>,
  parts: Vec<Element>,
  person_only: Option<Arc<str>>,
  cancel: Arc<str>,
  /// The footer without *Esc cancels*, its *Cancel* a ghost button with its ×.
  plain_footer: bool,
  /// The footer's start in place of *Esc cancels*: why the action cannot be taken, or what Esc does in words.
  note: Option<FooterNote>,
  enter: Option<Enter>,
  cancel_ref: Option<ElementRef>,
  actions: Vec<ActionButton>,
  under: Vec<Element>,
}

impl Dialog {
  /// The dialog `id` (its title `<id>-title`, its *Cancel* `<id>-cancel`, its scrim `<id>-veil`, the caps of its
  /// footer's keys `<id>-note_key_cap`, `<id>-note_key_cap-2`) asking `title` beside `glyph`.
  pub fn new(id: &str, glyph: Icon, title: impl Into<Arc<str>>, cancel: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      glyph,
      tone: Tone::default(),
      width: DIALOG_WIDTH,
      title: title.into(),
      what: None,
      parts: Vec::new(),
      person_only: None,
      cancel: cancel.into(),
      plain_footer: false,
      note: None,
      enter: None,
      cancel_ref: None,
      actions: Vec::new(),
      under: Vec::new(),
    }
  }

  /// The tile in `tone`'s colors.
  pub fn tone(mut self, tone: Tone) -> Self {
    self.tone = tone;
    self
  }

  /// `width` wide instead of the dialog's own 500, a structural size its caller names with its design node.
  pub fn width(mut self, width: f32) -> Self {
    self.width = width;
    self
  }

  /// What the dialog is about, in 11.5 `text_muted` 2 under its title (`<id>-description`; `What` `eoJka` of `hysCz`).
  pub fn what(mut self, what: impl Into<Arc<str>>) -> Self {
    self.what = Some(what.into());
    self
  }

  /// The footer without *Esc cancels*: a ghost *Cancel* with its ×, then the actions.
  pub fn plain_footer(mut self) -> Self {
    self.plain_footer = true;
    self
  }

  /// The footer in the plain form, with `note` at its start: a ghost *Cancel* with its ×, then the actions at its end.
  pub fn footer_note(mut self, note: FooterNote) -> Self {
    self.note = Some(note);
    self.plain_footer = true;
    self
  }

  /// Enter, pressed while no text field has the focus, runs `confirm`; the footer says so with `does` before *Esc
  /// cancels*.
  pub fn on_enter(mut self, does: impl Into<Arc<str>>, confirm: impl Fn() + Send + Sync + 'static) -> Self {
    self.enter = Some((does.into(), Arc::new(confirm)));
    self
  }

  /// `cancel` refers to *Cancel*, so its caller can give it the focus as the dialog opens.
  pub fn cancel_ref(mut self, cancel: ElementRef) -> Self {
    self.cancel_ref = Some(cancel);
    self
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

  /// What the footer ends in, after *Cancel* and the actions given before.
  pub fn action(mut self, action: ActionButton) -> Self {
    self.actions.push(action);
    self
  }

  /// What the body says after its parts, e.g. why the action did nothing.
  pub fn under(mut self, under: impl IntoIterator<Item = Element>) -> Self {
    self.under.extend(under);
    self
  }

  /// The dialog over the window while `open` holds; *Cancel*, Esc and the scrim set it to `false`.
  pub fn render(self, ctx: &Ctx, open: &Signal<bool>) -> Element {
    let shut = open.clone();
    let cancel = if self.plain_footer {
      ActionButton::new(format!("{}-cancel", self.id), ButtonKind::Ghost, self.cancel.clone()).icon(Icon::X)
    } else {
      ActionButton::new(format!("{}-cancel", self.id), ButtonKind::Secondary, self.cancel.clone())
    }
    .in_form()
    .on_click(move || shut.set(false));
    let cancel = match self.cancel_ref.clone() {
      Some(element) => cancel.element_ref(element),
      None => cancel,
    };
    let bodiless = self.parts.is_empty() && self.person_only.is_none() && self.under.is_empty();
    let mut dialog = ui::Dialog::new(open.clone())
      .id(self.id.clone())
      .title(&self.title)
      .icon(self.glyph.name())
      .width(self.width)
      .state(self.tone.state(), true)
      .state(kit::BODILESS, bodiless);
    if let Some(what) = &self.what {
      dialog = dialog.description(what);
    }
    dialog = self.note(ctx, dialog);
    if let Some((_, confirm)) = &self.enter {
      let confirm = confirm.clone();
      dialog = dialog
        .initial_focus(DialogFocus::Panel)
        .on_confirm(move || confirm());
    }
    let person = self.person_only.as_ref().map(|words| person_only(&self.id, words));
    let dialog = dialog
      .children(self.parts)
      .children(person)
      .children(self.under)
      .footer(cancel);
    self
      .actions
      .into_iter()
      .fold(dialog, |dialog, action| dialog.footer(action.in_form()))
      .into()
  }

  /// What the footer says at its start: what Enter does and that Esc cancels, in the app's words for the keys, or the
  /// caller's note; nothing in a plain footer without one.
  fn note(&self, ctx: &Ctx, dialog: ui::Dialog) -> ui::Dialog {
    if self.plain_footer {
      return match &self.note {
        Some(note) => dialog.note_child(note_start(&self.id, note)),
        None => dialog,
      };
    }
    let dialog = match &self.enter {
      Some((does, _)) => dialog.note_key_worded(&ctx.t("ui.dialog.enterKey"), does),
      None => dialog,
    };
    dialog.note_key_worded(&ctx.t("ui.dialog.escKey"), &ctx.t("ui.dialog.escCancels"))
  }
}

/// The line on `hover` that only the person confirms it, after the hand.
fn person_only(id: &str, words: &str) -> Row {
  Row::new()
    .id(format!("{id}-person-only"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(ExtraColor::Hover.role())
    .child(icon(Icon::Hand, IconSize::Base, ExtraColor::TextSecondary.role()))
    .child(
      Text::new(words)
        .variant(TypographyStyle::Description)
        .color(ExtraColor::TextSecondary.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// `parts`, the caller's own, in a dialog `width` wide over the window while `open` holds: no header or footer of
/// the dialog's, only its panel, 20 inside it; Esc and a press on the scrim set `open` to `false`.
pub fn over_window(id: &str, width: f32, parts: Vec<Element>, open: &Signal<bool>) -> Element {
  ui::Dialog::new(open.clone())
    .id(id.to_owned())
    .width(width)
    .state(kit::BARE, true)
    .children(parts)
    .into()
}

/// A footer note, as wide as the room before the buttons.
fn note_start(id: &str, note: &FooterNote) -> Element {
  match note {
    FooterNote::Unavailable(reason) => Row::new()
      .id(format!("{id}-unavailable"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .describe("role", "unavailable-reason")
      .child(icon(Icon::Info, IconSize::Sm, PaletteColor::MutedForeground).flex_shrink(0.0))
      .child(
        Text::new(reason)
          .id(format!("{id}-unavailable-text"))
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .into(),
    FooterNote::Keys(words) => Text::new(words)
      .id(format!("{id}-keys"))
      .variant(ExtraTypography::MonoNote.role())
      .nowrap()
      .into(),
  }
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
        .child(icon(glyph, IconSize::Base, PaletteColor::MutedForeground)),
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
