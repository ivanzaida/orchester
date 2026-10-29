//! A status item's card on hover (`Tooltip · Orchester Cloud` `tKtTc` in `ZSB6h`, `Tooltip · Orchester's MCP server`
//! `R2dWD6` in `e2DPHR`, the health dot's and the runner's cards `oagTL`, `ediuF` in `Status bar · redesign`): a
//! popover's panel (`ui::popover`) that shows above its anchor, its end at the anchor's end (or its start at the
//! anchor's start), once the pointer rested on the anchor as long as a tooltip waits, and goes when the pointer leaves.
//! It lets the pointer through. [`card`] lays out what a card of words says: a title, a state with its dot, what it
//! means and a hint; [`facts_card`] a card of facts: a state line, then a label and a value on each line.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  popover::{padded_popover_panel, popover_panel},
  tooltip::TOOLTIP_DELAY,
};
use crate::{
  shell::{
    clock::{Clock, clock},
    status_bar::dot,
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
};

/// A card's width (`tKtTc`).
const CARD_WIDTH: f32 = 380.0;
/// A card of facts' width (`oagTL`).
const FACTS_WIDTH: f32 = 320.0;
/// The width of a fact's label (`l4s6MY` in `oagTL`).
const LABEL_WIDTH: f32 = 96.0;

/// What a card says, by its element id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardWords {
  pub id: Arc<str>,
  pub title: String,
  /// The state line's dot.
  pub dot: PaletteColor,
  pub state: String,
  pub body: String,
  pub hint: String,
  /// A last line under the hint, such as the app's version.
  pub note: Option<String>,
}

/// A card of facts, by its element id: its state line with its dot (`Runner online · since 09:12`), a label and a
/// value on each line, and a note under a line.
#[derive(Clone, Debug, PartialEq)]
pub struct FactsCard {
  pub id: Arc<str>,
  /// The state line's dot, and its words' color.
  pub dot: (PaletteColor, PaletteColor),
  /// The state, and what follows it in `text_muted`; no line without a state.
  pub state: Option<(String, Option<String>)>,
  pub facts: Vec<Fact>,
  pub note: Option<String>,
  /// The card shows from the anchor's start instead of its end.
  pub from_start: bool,
  /// Where the anchor gives way in its line's order (lurq `shrink_order`), down to its content's limit; `None` keeps
  /// it whole.
  pub gives_way: Option<i32>,
}

/// A line of a [`FactsCard`]: `label`, and its value, which may change while it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Fact {
  pub id: &'static str,
  pub label: String,
  pub value: FactValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FactValue {
  Words(String),
  Clock(Clock),
}

/// What a card on hover shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Card {
  Words(CardWords),
  Facts(FactsCard),
}

impl Card {
  fn id(&self) -> &str {
    match self {
      Self::Words(words) => &words.id,
      Self::Facts(facts) => &facts.id,
    }
  }
}

impl From<CardWords> for Card {
  fn from(words: CardWords) -> Self {
    Self::Words(words)
  }
}

impl From<FactsCard> for Card {
  fn from(facts: FactsCard) -> Self {
    Self::Facts(facts)
  }
}

/// The card's panel.
pub fn card(words: &CardWords) -> Element {
  let state = Row::new()
    .id(format!("{}-state", words.id))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(dot(words.dot.clone()))
    .child(Text::new(&words.state).variant(ExtraTypography::Prose.role()));
  popover_panel(&words.id, CARD_WIDTH)
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&words.title).variant(ExtraTypography::FormSection.role()))
        .child(state),
    )
    .child(
      Text::new(&words.body)
        .id(format!("{}-body", words.id))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(&words.hint)
        .id(format!("{}-hint", words.id))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(words.note.as_ref().map(|note| {
      Text::new(note)
        .id(format!("{}-note", words.id))
        .variant(ExtraTypography::Note.role())
    }))
    .into()
}

/// The panel of a card of facts (`oagTL`): 12 inside a `border_strong` stroke, its lines 8 apart; the state 12/500 after
/// a 6 dot, a label 11 `text_muted` 96 wide and 12 before its value in `text_secondary`; the note under a `border`
/// line, 8 below it.
pub fn facts_card(ctx: &mut Ctx, card: &FactsCard) -> Element {
  let mut panel = padded_popover_panel(&card.id, FACTS_WIDTH, Padding::all(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .rounded(ExtraRadius::Tile.role());
  if let Some((state, side)) = &card.state {
    let (dot_color, color) = card.dot.clone();
    panel = panel.child(
      Row::new()
        .id(format!("{}-state", card.id))
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(dot(dot_color))
        .child(Text::new(state).variant(TypographyStyle::Label).color(color).nowrap())
        .child(Spacer::new().flex(1.0))
        .with_children(side.as_ref().map(|side| {
          Text::new(side)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextMuted)
            .nowrap()
        })),
    );
  }
  for fact in &card.facts {
    let id = format!("{}-{}", card.id, fact.id);
    let value = match &fact.value {
      FactValue::Words(words) => Text::new(words)
        .id(format!("{id}-value"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextSecondary)
        .into(),
      FactValue::Clock(time) => clock(ctx, &format!("{id}-value"), *time),
    };
    panel = panel.child(
      Row::new()
        .id(id)
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(
          Text::new(&fact.label)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextMuted)
            .width(Dimension::Px(LABEL_WIDTH)),
        )
        .child(Row::new().flex(1.0).child(value)),
    );
  }
  panel
    .with_children(card.note.as_ref().map(|note| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(SpacingSize::Sm))
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(
          Text::new(note)
            .id(format!("{}-note", card.id))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextMuted)
            .width(Dimension::Pct(100.0)),
        )
    }))
    .into()
}

/// `anchor` with `card` while the pointer rests on it.
pub fn hover_card(ctx: &mut Ctx, anchor: impl Into<Element>, card: impl Into<Card>) -> Element {
  let card = card.into();
  let key = format!("hover-card-{}", card.id());
  ctx.mount_keyed_with::<HoverCard>(&key, card, vec![anchor.into()])
}

impl DevtoolsInspectable for Card {}

struct HoverCard {
  shown: Signal<bool>,
  timer: Timeout,
  anchor: ElementRef,
}

impl Component for HoverCard {
  type Props = Card;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let show = shown.clone();
    Self {
      shown,
      timer: ctx.create_timeout(TOOLTIP_DELAY, move || show.set(true)),
      anchor: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let card = ctx.props::<Card>().clone();
    let (rest, leave, hide) = (self.timer.clone(), self.timer.clone(), self.shown.clone());
    let mut anchor = Row::new()
      .ref_element(self.anchor.clone())
      .align_items(Alignment::Center)
      .on_mouse_enter(move || rest.restart())
      .on_mouse_leave(move || {
        leave.cancel();
        hide.set(false);
      })
      .with_children(ctx.children().iter().cloned());
    if let Card::Facts(FactsCard {
      gives_way: Some(order),
      ..
    }) = &card
    {
      anchor = anchor
        .flex_shrink(1.0)
        .shrink_order(*order)
        .shrink_limit(ShrinkLimit::Content);
    }
    if !self.shown.get() {
      return anchor;
    }
    let (panel, placement) = match &card {
      Card::Words(words) => (self::card(words), Placement::TopEnd),
      Card::Facts(facts) => (
        facts_card(ctx, facts),
        if facts.from_start {
          Placement::TopStart
        } else {
          Placement::TopEnd
        },
      ),
    };
    anchor.child(
      Popup::new(self.anchor.clone(), panel)
        .open_when(true)
        .placement(placement)
        // Over its anchor, lurq counts the offset away from it.
        .offset(0.0, ExtraSpacing::Snug.value())
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false),
    )
  }
}
