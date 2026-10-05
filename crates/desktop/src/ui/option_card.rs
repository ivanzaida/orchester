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
  compact: bool,
  height: Option<Dimension>,
  /// Muted, at the right of the title: e.g. *not saved* or *saved 14:12* in 10.41.
  meta: Option<Arc<str>>,
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
      compact: false,
      height: None,
      meta: None,
    }
  }

  /// The smaller card of a row of four (`cQjjH` in 04.H1 `Byo8K`): 12 padding, a 14px mark, a 12px
  /// title and a 10px detail.
  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  pub fn on_choose(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.state = State::Available(Arc::new(handler));
    self
  }

  /// Execution-mode card (`YZcH2`, `jPX6u`) in the scheduling review.
  pub fn scheduling_review(mut self) -> Self {
    self.height = Some(Dimension::Px(crate::theme::TASK_REVIEW_MODE_HEIGHT));
    self
  }

  /// Says something about the option at the right of its title.
  pub fn meta(mut self, meta: Arc<str>) -> Self {
    self.meta = Some(meta);
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
    let (title, detail, mark_size) = if card.compact {
      (
        ExtraTypography::NoticeHeading.role(),
        ExtraTypography::Hint.role(),
        IconSize::Dismiss,
      )
    } else {
      (TypographyStyle::Label, ExtraTypography::Detail.role(), IconSize::Option)
    };
    let text = Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Indicator.role())
      .child(title_row(&card.title, title, card.meta.as_deref()))
      .child(Text::new(&card.detail).variant(detail));
    let mut button = Button::empty()
      .button()
      .id(card.id)
      .align_items(Alignment::Start)
      .width(Dimension::Pct(100.0))
      .padding(if card.compact {
        ExtraSpacing::Stack.role()
      } else {
        SpacingSize::Md
      })
      .spacing(if card.compact {
        ExtraSpacing::Inline.role()
      } else {
        ExtraSpacing::Stack.role()
      })
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(stroke, stroke_color))
      .rounded(ExtraRadius::Notice.role())
      .describe("selected", if card.selected { "true" } else { "false" })
      .child(icon(mark, mark_size, mark_color))
      .child(text);
    if let Some(height) = card.height {
      button = button.height(height);
    }
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

/// The title, with its meta at the right when it has one.
fn title_row(title: &str, typography: TypographyStyle, meta: Option<&str>) -> Element {
  let title = Text::new(title).variant(typography);
  match meta {
    None => title.into(),
    Some(meta) => Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(title.flex(1.0))
      .child(
        Text::new(meta)
          .variant(ExtraTypography::Hint.role())
          .color(PaletteColor::TextMuted),
      )
      .into(),
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
