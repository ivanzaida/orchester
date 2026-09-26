//! Command entry (`bpmzN`): an allowed command in mono with a trailing icon. With an action it is a
//! form control: a press anywhere on it, or Enter or Space while it has the focus, removes the
//! command (`x`) or adds it back (`plus`, for a command a step leaves out).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Button, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, keyboard_stop},
};
use crate::theme::{ExtraSpacing, ExtraTypography};

/// Entry height (`bpmzN`).
const HEIGHT: f32 = 26.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

enum Action {
  None,
  Remove(Handler),
  Add(Handler),
}

pub struct CommandChip {
  id: String,
  command: Arc<str>,
  action: Action,
}

impl CommandChip {
  pub fn new(id: String, command: Arc<str>) -> Self {
    Self {
      id,
      command,
      action: Action::None,
    }
  }

  /// Pressing the entry removes the command.
  pub fn on_remove(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.action = Action::Remove(Arc::new(handler));
    self
  }

  /// The command is left out; pressing the entry adds it back.
  pub fn on_add(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.action = Action::Add(Arc::new(handler));
    self
  }
}

impl From<CommandChip> for Element {
  fn from(chip: CommandChip) -> Self {
    let (handler, state, glyph, text_color, background) = match chip.action {
      Action::None => {
        return Row::new()
          .id(chip.id)
          .height(Dimension::Px(HEIGHT))
          .padding_horizontal(ExtraSpacing::Inline.role())
          .align_items(Alignment::Center)
          .background(PaletteColor::SurfaceRaised)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .rounded(RadiusSize::Sm)
          .child(command(&chip.command, PaletteColor::TextPrimary))
          .into();
      }
      Action::Remove(handler) => (
        handler,
        "included",
        Icon::X,
        PaletteColor::TextPrimary,
        PaletteColor::SurfaceRaised,
      ),
      Action::Add(handler) => (
        handler,
        "left-out",
        Icon::Plus,
        PaletteColor::TextMuted,
        PaletteColor::SurfacePanel,
      ),
    };
    keyboard_stop(Button::empty().button(), Ring::Outer)
      .id(chip.id)
      .height(Dimension::Px(HEIGHT))
      .padding_left(ExtraSpacing::Inline.role())
      .padding_right(ExtraSpacing::Tight.role())
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(background)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .describe("state", state)
      .on_click(move |_| handler())
      .child(command(&chip.command, text_color))
      .child(icon(glyph, IconSize::Small, PaletteColor::TextMuted))
      .into()
  }
}

fn command(command: &str, color: PaletteColor) -> Text {
  Text::new(command)
    .variant(ExtraTypography::MonoValue.role())
    .color(color)
    .nowrap()
}
