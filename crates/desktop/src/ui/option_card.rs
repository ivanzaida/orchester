//! Option card (`lcwsy`): a radio mark beside a title and what the option does. The chosen card
//! has the 1.5px `$text` stroke. A form control: Tab reaches an available card and Enter or Space
//! chooses it; the chosen card shows its stroke instead of the focus ring (see `ui::pressable`).
//! An option that cannot be chosen is faded, not focusable, and says why under it (04.B `W0Owo`).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Button, Column, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, keyboard_stop, stroke_selected_focused},
};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography, UNAVAILABLE_OPTION_OPACITY};

type Handler = Arc<dyn Fn() + Send + Sync>;

enum State {
  Available(Handler),
  /// Cannot be chosen; the reason, if the title does not already say it.
  Unavailable(Option<Arc<str>>),
}

pub struct OptionCard {
  id: &'static str,
  title: Arc<str>,
  detail: Arc<str>,
  selected: bool,
  state: State,
}

impl OptionCard {
  /// An unavailable card until given [`Self::on_choose`].
  pub fn new(id: &'static str, title: Arc<str>, detail: Arc<str>, selected: bool) -> Self {
    Self {
      id,
      title,
      detail,
      selected,
      state: State::Unavailable(None),
    }
  }

  pub fn on_choose(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.state = State::Available(Arc::new(handler));
    self
  }

  /// Cannot be chosen, for `reason` (lock icon and text under the card).
  pub fn unavailable(mut self, reason: Option<Arc<str>>) -> Self {
    self.state = State::Unavailable(reason);
    self
  }
}

impl From<OptionCard> for Element {
  fn from(card: OptionCard) -> Self {
    let (mark, mark_color, stroke, stroke_color) = if card.selected {
      (
        Icon::CircleDot,
        PaletteColor::TextPrimary,
        BorderSize::Md,
        PaletteColor::TextPrimary,
      )
    } else {
      (
        Icon::Circle,
        PaletteColor::TextSecondary,
        BorderSize::Sm,
        PaletteColor::Border,
      )
    };
    let text = Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Indicator.role())
      .child(Text::new(&card.title).variant(TypographyStyle::Label))
      .child(Text::new(&card.detail).variant(ExtraTypography::Detail.role()));
    let button = Button::empty()
      .button()
      .id(card.id)
      .align_items(Alignment::Start)
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(stroke, stroke_color))
      .rounded(ExtraRadius::Notice.role())
      .describe("selected", if card.selected { "true" } else { "false" })
      .child(icon(mark, IconSize::Option, mark_color))
      .child(text);
    match card.state {
      State::Available(handler) => keyboard_stop(button, Ring::Outer)
        .focused_style(stroke_selected_focused(Ring::Outer, card.selected))
        .cursor(CursorIcon::Pointer)
        .describe("state", "enabled")
        .on_click(move |_| handler())
        .into(),
      State::Unavailable(reason) => Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(
          button
            .focusable(false)
            .cursor(CursorIcon::NotAllowed)
            .describe("state", "unavailable")
            .opacity(UNAVAILABLE_OPTION_OPACITY),
        )
        .with_children(reason.map(|reason| unavailable_reason(&reason)))
        .into(),
    }
  }
}

/// Why an option cannot be chosen (`J5RohL`, `L1B0m`): a lock and the reason, lined up with the
/// option's text.
pub fn unavailable_reason(reason: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_left(ExtraSpacing::ReasonIndent.role())
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Start)
    .describe("role", "unavailable-reason")
    .child(icon(Icon::Lock, IconSize::Reason, PaletteColor::TextMuted))
    .child(
      Text::new(reason)
        .variant(ExtraTypography::Hint.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
    .into()
}
