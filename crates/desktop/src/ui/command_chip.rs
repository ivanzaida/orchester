//! Command entry (`bpmzN`): an allowed command in mono with a trailing icon. With an action it is a
//! form control: a press anywhere on it, or Enter or Space while it has the focus, removes the
//! command (`x`) or adds it back (`plus`, for a command a step leaves out). A filling entry takes
//! the width of its row and ends long text (a folder path) in an ellipsis before its icon.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Button, Row, Text, TextOverflow},
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
  /// Outlined in the text color: the command differs from what it is compared with (a command
  /// added for a run, `IWbGO` in 02.J).
  changed: bool,
  /// Takes the width of its row; long text ends in an ellipsis.
  fill: bool,
}

impl CommandChip {
  pub fn new(id: String, command: Arc<str>) -> Self {
    Self {
      id,
      command,
      action: Action::None,
      changed: false,
      fill: false,
    }
  }

  /// The entry takes the width of its row and shortens long text with an ellipsis.
  pub const fn fill(mut self) -> Self {
    self.fill = true;
    self
  }

  pub const fn changed(mut self, changed: bool) -> Self {
    self.changed = changed;
    self
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
    let stroke = if chip.changed {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::Border
    };
    let (handler, state, glyph, text_color, background) = match chip.action {
      Action::None => {
        return Row::new()
          .id(chip.id)
          .height(Dimension::Px(HEIGHT))
          .padding_horizontal(ExtraSpacing::Inline.role())
          .align_items(Alignment::Center)
          .background(PaletteColor::SurfaceRaised)
          .border(Border::inside(BorderSize::Sm, stroke))
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
    let text = command(&chip.command, text_color);
    let (width, text) = if chip.fill {
      (
        Dimension::Pct(100.0),
        text.text_overflow(TextOverflow::Elipsis).flex(1.0),
      )
    } else {
      (Dimension::Auto, text)
    };
    keyboard_stop(Button::empty().button(), Ring::Outer)
      .id(chip.id)
      .width(width)
      .height(Dimension::Px(HEIGHT))
      .padding_left(ExtraSpacing::Inline.role())
      .padding_right(ExtraSpacing::Tight.role())
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(background)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .describe("state", state)
      .describe("changed", if chip.changed { "true" } else { "false" })
      .on_click(move |_| handler())
      .child(text)
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
