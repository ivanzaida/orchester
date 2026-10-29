//! The Browser window's title bar (`Title bar` `Rc27N` in `Window/Browser` `lfxcu`): one 36 row, as the main window's,
//! with the mark, a divider and the project switcher, then the tabs, *+* and the room that drags the window, and the
//! window controls; on macOS the traffic lights end the row as the main window's do. The word *Browser* is not in the
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

/// `bar` with the mark, the switcher and the tabs, and `lights` at its end on macOS.
pub fn content(ctx: &mut Ctx, bar: ChromeTitleBar, lights: Option<Element>, showing: &Showing) -> ChromeTitleBar {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  let leading = Row::new()
    .padding(Padding::new().left(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Row::new().id("browser-mark").child(mark()))
    .child(
      Row::new()
        .width(Dimension::Px(line))
        .height(Dimension::Px(DIVIDER_HEIGHT))
        .background(PaletteColor::Border),
    )
    .child(ctx.mount_keyed::<Switcher>("browser-switcher", showing.clone()));
  let bar = bar
    .leading(leading)
    .center(ctx.mount_keyed::<Strip>("browser-strip", showing.clone()));
  match lights {
    Some(lights) => bar.trailing(Row::new().align_items(Alignment::Center).child(lights)),
    None => bar,
  }
}
