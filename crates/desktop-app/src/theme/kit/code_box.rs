//! Code box (`ui::code_box`; `Command` `MOPAN` in `Approval card` `gwOaJ`, `SiCGv`): its value is lurq's
//! `ui::CodeBlock` with no bar and no frame of its own, in the state [`CODE_BOX`]: Geist Mono 12 at 1.5, with no room
//! round its lines, since the kit's box around it pads it.

use lurq::{
  app::theme::Theme,
  ui::{CodeBlock, CodeBlockPart},
};

/// The state a code box's block is in.
pub const CODE_BOX: &str = "code_box";
/// A line of the value: Geist Mono 12 at 1.5.
pub const ROW_HEIGHT: f32 = 18.0;

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<CodeBlock>(|rules| {
    rules.state(CODE_BOX, CodeBlockPart::Code, |style| {
      style.padding_x(0.0).padding_y(0.0)
    })
  });
}
