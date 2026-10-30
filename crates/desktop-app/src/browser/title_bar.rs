//! The Browser window's title bar (`Title bar` `Rc27N` in `Window/Browser` `lfxcu`): one 36 row, as the main window's,
//! with the mark, a divider and the project switcher, then the tabs, *+* and the room that drags the window, and the
//! window controls. On macOS the traffic lights come first in the row and the drag room ends it (`M1` `Nahtk`); in
//! full screen the lights hide and the row starts at the left edge (`M2` `nbobh`). The word *Browser* is not in the
//! row (*Open choices* `J6JdR`); the OS window title keeps it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{ChromeTitleBar, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{state::Showing, strip::Strip, switcher::Switcher};
use crate::{brand::mark, theme::ExtraSpacing};

/// The line between the mark and the project switcher (`Divider` `TY2Ik`).
const DIVIDER_HEIGHT: f32 = 16.0;

/// `bar` with `lights` first on macOS outside full screen, then the mark, the switcher and the tabs.
pub fn content(ctx: &mut Ctx, bar: ChromeTitleBar, lights: Option<Element>, showing: &Showing) -> ChromeTitleBar {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  let lights = lights.filter(|_| !ctx.window().info().is_full_screen);
  let leading = Row::new()
    .padding(Padding::new().left(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(lights)
    .child(Row::new().id("browser-mark").child(mark()))
    .child(
      Row::new()
        .width(Dimension::Px(line))
        .height(Dimension::Px(DIVIDER_HEIGHT))
        .background(PaletteColor::Border),
    )
    .child(ctx.mount_keyed::<Switcher>("browser-switcher", showing.clone()));
  bar
    .leading(leading)
    .center(ctx.mount_keyed::<Strip>("browser-strip", showing.clone()))
}
